use anyhow::{Context, Result};
use keyring::v1::Entry;

const SERVICE_PREFIX: &str = "com.ionsense.desktop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    ImapPassword,
    DiscordBotToken,
}

impl SecretKind {
    fn service(self) -> &'static str {
        match self {
            Self::ImapPassword => "com.ionsense.desktop.imap",
            Self::DiscordBotToken => "com.ionsense.desktop.discord",
        }
    }
}

pub fn get_secret(kind: SecretKind, account: &str) -> Result<String> {
    let entry = Entry::new(kind.service(), account).context("open OS credential entry")?;
    entry.get_password().context("read secret from OS keychain")
}

pub fn set_secret(kind: SecretKind, account: &str, secret: &str) -> Result<()> {
    anyhow::ensure!(!account.trim().is_empty(), "credential account is empty");
    anyhow::ensure!(!secret.is_empty(), "credential secret is empty");
    let entry = Entry::new(kind.service(), account).context("open OS credential entry")?;
    entry
        .set_password(secret)
        .context("write secret to OS keychain")
}

pub fn delete_secret(kind: SecretKind, account: &str) -> Result<()> {
    let entry = Entry::new(kind.service(), account).context("open OS credential entry")?;
    entry
        .delete_credential()
        .context("delete secret from OS keychain")
}

pub fn credential_account(kind: SecretKind, configured_account: &str) -> String {
    match kind {
        SecretKind::ImapPassword => format!("{SERVICE_PREFIX}:{}", configured_account.trim()),
        SecretKind::DiscordBotToken => format!("{SERVICE_PREFIX}:discord-bot"),
    }
}
