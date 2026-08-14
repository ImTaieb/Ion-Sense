use std::collections::{HashSet, VecDeque};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serenity::{
    Client, async_trait,
    model::{channel::Message, gateway::GatewayIntents},
    prelude::{Context, EventHandler},
};

use crate::{
    credentials::{SecretKind, credential_account, get_secret},
    dispatcher::EventDispatcher,
    event::{IonSenseEvent, IonSenseEventType, Severity},
    settings::DiscordSettings,
};

struct Handler {
    dispatcher: EventDispatcher,
    allowed_channels: HashSet<u64>,
    recent_messages: Mutex<RecentMessageIds>,
}

struct RecentMessageIds {
    capacity: usize,
    order: VecDeque<u64>,
    ids: HashSet<u64>,
}

impl RecentMessageIds {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::with_capacity(capacity),
            ids: HashSet::with_capacity(capacity),
        }
    }

    fn accept(&mut self, id: u64) -> bool {
        if !self.ids.insert(id) {
            return false;
        }
        self.order.push_back(id);
        while self.order.len() > self.capacity {
            if let Some(expired) = self.order.pop_front() {
                self.ids.remove(&expired);
            }
        }
        true
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _context: Context, message: Message) {
        if message.author.bot || message.webhook_id.is_some() {
            return;
        }

        let is_direct_message = message.guild_id.is_none();
        if !is_direct_message && !self.allowed_channels.contains(&message.channel_id.get()) {
            return;
        }
        if !self
            .recent_messages
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .accept(message.id.get())
        {
            return;
        }

        let body = if message.content.trim().is_empty() {
            "(attachment, embed, or message content unavailable)".into()
        } else {
            truncate(message.content.trim(), 160)
        };
        let event = IonSenseEvent::new(
            IonSenseEventType::FriendMessage,
            format!("{}: {body}", message.author.name),
            Severity::Info,
        );
        if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) =
            self.dispatcher.try_dispatch(event)
        {
            eprintln!("Ion Sense dropped a Discord alert because the event queue is full");
        }
    }
}

pub fn spawn(
    settings: DiscordSettings,
    dispatcher: EventDispatcher,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("ion-discord-detector".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("create Discord detector runtime");
            runtime.block_on(run(settings, dispatcher, stop));
        })
        .expect("failed to spawn Discord detector")
}

async fn run(settings: DiscordSettings, dispatcher: EventDispatcher, stop: Arc<AtomicBool>) {
    let credential = credential_account(SecretKind::DiscordBotToken, "discord-bot");
    let token = match get_secret(SecretKind::DiscordBotToken, &credential) {
        Ok(token) => token,
        Err(error) => {
            eprintln!("Ion Sense Discord detector needs an OS-keychain token: {error:#}");
            return;
        }
    };

    let handler = Handler {
        dispatcher,
        recent_messages: Mutex::new(RecentMessageIds::new(512)),
        allowed_channels: settings
            .allowed_channel_ids
            .into_iter()
            .filter_map(|channel| channel.parse::<u64>().ok())
            .collect(),
    };
    let intents = GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let client_setup = Client::builder(token, intents).event_handler(handler);
    let mut client = match tokio::time::timeout(Duration::from_secs(20), client_setup).await {
        Err(_) => {
            eprintln!("Ion Sense Discord client setup timed out");
            return;
        }
        Ok(Ok(client)) => client,
        Ok(Err(error)) => {
            eprintln!("Ion Sense Discord client setup failed: {error}");
            return;
        }
    };

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        while !stop.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        shard_manager.shutdown_all().await;
    });

    if let Err(error) = client.start().await {
        eprintln!("Ion Sense Discord detector stopped: {error}");
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut result: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        result.push('…');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_never_slices_inside_utf8() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("🙂🙂🙂", 2), "🙂🙂…");
    }

    #[test]
    fn recent_message_ids_reject_replays_and_remain_bounded() {
        let mut recent = RecentMessageIds::new(2);
        assert!(recent.accept(10));
        assert!(!recent.accept(10));
        assert!(recent.accept(11));
        assert!(recent.accept(12));
        assert_eq!(recent.order.len(), 2);
        assert_eq!(recent.ids.len(), 2);
        assert!(recent.accept(10));
    }
}
