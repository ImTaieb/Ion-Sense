use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use async_imap::Session;
use futures::TryStreamExt;
use rustls_platform_verifier::ConfigVerifierExt;
use tokio::net::TcpStream;
use tokio_rustls::{
    TlsConnector,
    client::TlsStream,
    rustls::{ClientConfig, pki_types::ServerName},
};

use crate::{
    credentials::{SecretKind, credential_account, get_secret},
    dispatcher::EventDispatcher,
    event::{IonSenseEvent, IonSenseEventType, Severity},
    settings::EmailSettings,
};

type ImapSession = Session<TlsStream<TcpStream>>;
const IMAP_IO_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Default)]
struct MailboxCursor {
    uid_validity: Option<u32>,
    last_uid: Option<u32>,
}

pub fn spawn(
    settings: EmailSettings,
    dispatcher: EventDispatcher,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("ion-email-detector".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("create email detector runtime");
            runtime.block_on(run(settings, dispatcher, stop));
        })
        .expect("failed to spawn email detector")
}

async fn run(settings: EmailSettings, dispatcher: EventDispatcher, stop: Arc<AtomicBool>) {
    let credential = credential_account(SecretKind::ImapPassword, &settings.username);
    let password = match get_secret(SecretKind::ImapPassword, &credential) {
        Ok(password) => password,
        Err(error) => {
            eprintln!("Ion Sense email detector needs an OS-keychain password: {error:#}");
            return;
        }
    };

    let mut cursor = MailboxCursor::default();
    let mut backoff = 5_u64;
    while !stop.load(Ordering::Acquire) {
        match monitor_connection(&settings, &password, &dispatcher, &stop, &mut cursor).await {
            Ok(()) if stop.load(Ordering::Acquire) => break,
            Ok(()) => backoff = 5,
            Err(error) => {
                eprintln!("Ion Sense email detector disconnected: {error:#}");
                sleep_with_stop(&stop, Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(300);
            }
        }
    }
}

async fn monitor_connection(
    settings: &EmailSettings,
    password: &str,
    dispatcher: &EventDispatcher,
    stop: &AtomicBool,
    cursor: &mut MailboxCursor,
) -> Result<()> {
    let mut session = tokio::time::timeout(IMAP_IO_TIMEOUT, connect(settings, password))
        .await
        .context("IMAP connection timed out")??;
    let mailbox = tokio::time::timeout(IMAP_IO_TIMEOUT, session.examine(&settings.mailbox))
        .await
        .context("opening the IMAP mailbox timed out")?
        .with_context(|| format!("open IMAP mailbox {}", settings.mailbox))?;
    let capabilities = tokio::time::timeout(IMAP_IO_TIMEOUT, session.capabilities())
        .await
        .context("reading IMAP capabilities timed out")?
        .context("read IMAP capabilities")?;
    let supports_idle = capabilities.has_str("IDLE");

    if cursor.uid_validity != mailbox.uid_validity || cursor.last_uid.is_none() {
        cursor.uid_validity = mailbox.uid_validity;
        cursor.last_uid = tokio::time::timeout(IMAP_IO_TIMEOUT, session.uid_search("ALL"))
            .await
            .context("baselining the IMAP cursor timed out")?
            .context("baseline IMAP UID cursor")?
            .into_iter()
            .max();
    }

    while !stop.load(Ordering::Acquire) {
        emit_new_messages(&mut session, cursor, dispatcher, stop).await?;

        if supports_idle {
            let mut idle = session.idle();
            tokio::time::timeout(IMAP_IO_TIMEOUT, idle.init())
                .await
                .context("starting IMAP IDLE timed out")?
                .context("start IMAP IDLE")?;
            {
                let (wait, stop_source) = idle.wait_with_timeout(Duration::from_secs(60));
                tokio::pin!(wait);
                let mut interrupt = Some(stop_source);
                tokio::select! {
                    result = &mut wait => { result.context("wait for IMAP IDLE update")?; }
                    _ = sleep_until_stopped(stop) => {
                        drop(interrupt.take());
                        tokio::time::timeout(Duration::from_secs(5), wait)
                            .await
                            .context("interrupting IMAP IDLE timed out")?
                            .context("interrupt IMAP IDLE")?;
                    }
                }
            }
            session = tokio::time::timeout(IMAP_IO_TIMEOUT, idle.done())
                .await
                .context("finishing IMAP IDLE timed out")?
                .context("finish IMAP IDLE")?;
        } else {
            sleep_with_stop(stop, Duration::from_secs(settings.poll_seconds)).await;
            if !stop.load(Ordering::Acquire) {
                tokio::time::timeout(IMAP_IO_TIMEOUT, session.noop())
                    .await
                    .context("polling the IMAP mailbox timed out")?
                    .context("poll IMAP mailbox")?;
            }
        }
    }

    let _ = tokio::time::timeout(Duration::from_secs(5), session.logout()).await;
    Ok(())
}

async fn connect(settings: &EmailSettings, password: &str) -> Result<ImapSession> {
    let config = ClientConfig::with_platform_verifier().context("load platform TLS verifier")?;
    let tcp = TcpStream::connect((settings.host.as_str(), settings.port))
        .await
        .with_context(|| format!("connect to {}:{}", settings.host, settings.port))?;
    let server_name = ServerName::try_from(settings.host.clone()).context("invalid IMAP host")?;
    let tls = TlsConnector::from(Arc::new(config))
        .connect(server_name, tcp)
        .await
        .context("negotiate IMAP TLS")?;
    let mut client = async_imap::Client::new(tls);
    client
        .read_response()
        .await
        .context("read IMAP server greeting")?
        .ok_or_else(|| anyhow!("IMAP server closed before greeting"))?;
    client
        .login(&settings.username, password)
        .await
        .map_err(|(error, _client)| error)
        .context("authenticate to IMAP")
}

async fn emit_new_messages(
    session: &mut ImapSession,
    cursor: &mut MailboxCursor,
    dispatcher: &EventDispatcher,
    stop: &AtomicBool,
) -> Result<()> {
    let first_uid = cursor.last_uid.unwrap_or(0).saturating_add(1);
    let mut uids: Vec<u32> = tokio::time::timeout(
        IMAP_IO_TIMEOUT,
        session.uid_search(format!("UID {first_uid}:*")),
    )
    .await
    .context("searching for new email timed out")?
    .context("search for new email UIDs")?
    .into_iter()
    .filter(|uid| cursor.last_uid.is_none_or(|known| *uid > known))
    .collect();
    uids.sort_unstable();

    for uid in uids {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut rows = tokio::time::timeout(
            IMAP_IO_TIMEOUT,
            session.uid_fetch(uid.to_string(), "(UID ENVELOPE)"),
        )
        .await
        .with_context(|| format!("fetch email UID {uid} timed out"))?
        .with_context(|| format!("fetch email UID {uid}"))?;
        while let Some(fetch) = tokio::time::timeout(IMAP_IO_TIMEOUT, rows.try_next())
            .await
            .context("reading an email envelope timed out")?
            .context("read email envelope")?
        {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            if let Some(envelope) = fetch.envelope() {
                let subject = envelope
                    .subject
                    .as_ref()
                    .map(|value| String::from_utf8_lossy(value).into_owned())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "(no subject)".into());
                let sender = envelope
                    .from
                    .as_ref()
                    .and_then(|addresses| addresses.first())
                    .and_then(|address| address.name.as_ref().or(address.mailbox.as_ref()))
                    .map(|value| String::from_utf8_lossy(value.as_ref()).into_owned())
                    .unwrap_or_else(|| "A sender".into());
                let event = IonSenseEvent::new(
                    IonSenseEventType::NewEmail,
                    format!("{sender} sent “{}”.", truncate(&subject, 120)),
                    Severity::Info,
                );
                if stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                if let Err(error) = dispatcher.try_dispatch(event) {
                    match error {
                        tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                            return Err(anyhow!("central event dispatcher closed"));
                        }
                        tokio::sync::mpsc::error::TrySendError::Full(_) => eprintln!(
                            "Ion Sense dropped an email alert because the event queue is full"
                        ),
                    }
                }
            }
        }
        drop(rows);
        cursor.last_uid = Some(cursor.last_uid.unwrap_or(0).max(uid));
    }

    Ok(())
}

async fn sleep_until_stopped(stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn sleep_with_stop(stop: &AtomicBool, duration: Duration) {
    let mut remaining = duration.as_secs().max(1);
    while remaining > 0 && !stop.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_secs(1)).await;
        remaining -= 1;
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut result: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        result.push('…');
    }
    result
}
