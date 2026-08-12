use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, ensure};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub version: u32,
    pub launch_at_login: bool,
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
            launch_at_login: false,
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
    #[serde(default, deserialize_with = "deserialize_channel_ids")]
    pub allowed_channel_ids: Vec<String>,
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
        self.discord.allowed_channel_ids = self
            .discord
            .allowed_channel_ids
            .into_iter()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty() && value.parse::<u64>().is_ok())
            .collect();
        self.discord.allowed_channel_ids.sort_unstable();
        self.discord.allowed_channel_ids.dedup();
        self
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.battery.threshold_percent.is_finite(),
            "battery threshold must be a number"
        );
        ensure!(
            self.temperature.threshold_celsius.is_finite(),
            "temperature threshold must be a number"
        );
        if let Some(directory) = self.downloads.directory.as_deref() {
            ensure!(
                Path::new(directory).is_dir(),
                "Downloads override is not an existing folder"
            );
        }
        if self.email.enabled {
            ensure!(!self.email.host.is_empty(), "IMAP host is required");
            ensure!(
                self.email.port != 0,
                "IMAP port must be between 1 and 65535"
            );
            ensure!(!self.email.username.is_empty(), "IMAP username is required");
            ensure!(!self.email.mailbox.is_empty(), "IMAP mailbox is required");
        }
        Ok(())
    }

    pub fn load_or_create(path: &Path) -> Result<Self> {
        let backup = sidecar(path, "bak");
        if !path.exists() && backup.exists() {
            fs::copy(&backup, path)
                .with_context(|| format!("recover settings from backup {}", backup.display()))?;
        }
        if !path.exists() {
            let settings = Self::default().sanitized();
            settings.save(path)?;
            return Ok(settings);
        }

        let raw = fs::read_to_string(path)
            .with_context(|| format!("read settings from {}", path.display()))?;
        match serde_json::from_str::<Self>(&raw) {
            Ok(settings) => Ok(settings.sanitized()),
            Err(primary_error) if backup.exists() => {
                let backup_raw = fs::read_to_string(&backup)
                    .with_context(|| format!("read settings backup {}", backup.display()))?;
                let settings: Self = serde_json::from_str(&backup_raw).with_context(|| {
                    format!(
                        "parse settings from {} (primary error: {primary_error})",
                        backup.display()
                    )
                })?;
                fs::copy(&backup, path)
                    .with_context(|| format!("restore settings backup to {}", path.display()))?;
                Ok(settings.sanitized())
            }
            Err(error) => Err(anyhow!(error))
                .with_context(|| format!("parse settings from {}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create settings directory {}", parent.display()))?;
        }
        let encoded = serde_json::to_string_pretty(self).context("serialize settings")?;
        let pending = sidecar(path, "new");
        let backup = sidecar(path, "bak");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&pending)
            .with_context(|| format!("open pending settings file {}", pending.display()))?;
        file.write_all(format!("{encoded}\n").as_bytes())
            .with_context(|| format!("write pending settings file {}", pending.display()))?;
        file.sync_all()
            .with_context(|| format!("flush pending settings file {}", pending.display()))?;
        drop(file);

        if path.exists() {
            fs::copy(path, &backup)
                .with_context(|| format!("back up settings to {}", backup.display()))?;
            #[cfg(target_os = "windows")]
            fs::remove_file(path)
                .with_context(|| format!("replace settings file {}", path.display()))?;
        }

        if let Err(error) = fs::rename(&pending, path) {
            if !path.exists() && backup.exists() {
                let _ = fs::copy(&backup, path);
            }
            return Err(error).with_context(|| format!("commit settings to {}", path.display()));
        }
        Ok(())
    }
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("settings.json");
    path.with_file_name(format!("{name}.{suffix}"))
}

fn deserialize_channel_ids<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .filter_map(|value| match value {
            serde_json::Value::String(value) => Some(value),
            serde_json::Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_untrusted_numeric_and_list_values() {
        let mut settings = AppSettings::default();
        settings.battery.threshold_percent = -9.0;
        settings.temperature.threshold_celsius = 999.0;
        settings.discord.allowed_channel_ids = vec!["8".into(), "3".into(), "8".into()];

        let settings = settings.sanitized();
        assert_eq!(settings.battery.threshold_percent, 1.0);
        assert_eq!(settings.temperature.threshold_celsius, 120.0);
        assert_eq!(settings.discord.allowed_channel_ids, vec!["3", "8"]);
    }

    #[test]
    fn migrates_numeric_discord_channel_ids() {
        let settings: AppSettings = serde_json::from_value(serde_json::json!({
            "discord": { "allowed_channel_ids": [123, "456"] }
        }))
        .unwrap();
        assert_eq!(settings.discord.allowed_channel_ids, vec!["123", "456"]);
    }

    #[test]
    fn enabled_email_requires_connection_fields() {
        let mut settings = AppSettings::default();
        settings.email.enabled = true;
        assert!(settings.validate().is_err());

        settings.email.host = "imap.example.com".into();
        settings.email.username = "user@example.com".into();
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn corrupt_primary_recovers_the_previous_settings_backup() {
        let unique = format!(
            "ion-sense-settings-test-{}-{}",
            std::process::id(),
            crate::event::IonSenseEvent::new(
                crate::event::IonSenseEventType::NewEmail,
                "test",
                crate::event::Severity::Info,
            )
            .timestamp
        );
        let directory = std::env::temp_dir().join(unique);
        fs::create_dir(&directory).unwrap();
        let path = directory.join("settings.json");

        let original = AppSettings::default();
        original.save(&path).unwrap();
        let mut replacement = original.clone();
        replacement.battery.threshold_percent = 37.0;
        replacement.save(&path).unwrap();
        fs::write(&path, "not json").unwrap();

        let recovered = AppSettings::load_or_create(&path).unwrap();
        assert_eq!(recovered.battery.threshold_percent, 20.0);

        for file in [sidecar(&path, "new"), sidecar(&path, "bak"), path] {
            if file.exists() {
                fs::remove_file(file).unwrap();
            }
        }
        fs::remove_dir(directory).unwrap();
    }
}
