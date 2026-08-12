use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub version: u32,
    pub battery: BatterySettings,
    pub temperature: TemperatureSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: 1,
            battery: BatterySettings::default(),
            temperature: TemperatureSettings::default(),
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
        self
    }
}
