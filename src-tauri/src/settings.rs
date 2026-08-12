use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub version: u32,
    pub battery: BatterySettings,
    pub temperature: TemperatureSettings,
    pub downloads: DownloadsSettings,
    pub email: EmailSettings,
    pub discord: DiscordSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: 1,
            battery: BatterySettings::default(),
            temperature: TemperatureSettings::default(),
            downloads: DownloadsSettings::default(),
            email: EmailSettings::default(),
            discord: DiscordSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BatterySettings {
    pub enabled: bool,
    pub threshold_percent: f32,
    pub poll_seconds: u64,
}

impl Default for BatterySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_percent: 20.0,
            poll_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TemperatureSettings {
    pub enabled: bool,
    pub threshold_celsius: f32,
    pub poll_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadsSettings {
    pub enabled: bool,
    pub directory: Option<String>,
}

impl Default for DownloadsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmailSettings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub mailbox: String,
    pub poll_seconds: u64,
}

impl Default for EmailSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 993,
            username: String::new(),
            mailbox: "INBOX".into(),
            poll_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordSettings {
    pub enabled: bool,
    pub allowed_channel_ids: Vec<u64>,
}

impl Default for DiscordSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_channel_ids: Vec::new(),
        }
    }
}

impl Default for TemperatureSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_celsius: 85.0,
            poll_seconds: 5,
        }
    }
}

impl AppSettings {
    pub fn sanitized(mut self) -> Self {
        self.version = 1;
        self.battery.threshold_percent = self.battery.threshold_percent.clamp(1.0, 100.0);
        self.battery.poll_seconds = self.battery.poll_seconds.clamp(5, 3_600);
        self.temperature.threshold_celsius = self.temperature.threshold_celsius.clamp(40.0, 120.0);
        self.temperature.poll_seconds = self.temperature.poll_seconds.clamp(2, 300);
        self.email.host = self.email.host.trim().to_owned();
        self.email.username = self.email.username.trim().to_owned();
        self.email.mailbox = self.email.mailbox.trim().to_owned();
        if self.email.mailbox.is_empty() {
            self.email.mailbox = "INBOX".into();
        }
        self.email.poll_seconds = self.email.poll_seconds.clamp(15, 3_600);
        self.discord.allowed_channel_ids.sort_unstable();
        self.discord.allowed_channel_ids.dedup();
        self
    }
}
