use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IonSenseEventType {
    BatteryLow,
    Overheating,
    DownloadFinished,
    NewEmail,
    FriendMessage,
    PackageDelivered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IonSenseEvent {
    #[serde(rename = "type")]
    pub event_type: IonSenseEventType,
    pub message: String,
    pub severity: Severity,
    pub timestamp: u64,
}

impl IonSenseEvent {
    pub fn new(
        event_type: IonSenseEventType,
        message: impl Into<String>,
        severity: Severity,
    ) -> Self {
        Self {
            event_type,
            message: message.into(),
            severity,
            timestamp: unix_timestamp_millis(),
        }
    }
}

fn unix_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_the_phase_one_contract() {
        let event = IonSenseEvent {
            event_type: IonSenseEventType::BatteryLow,
            message: "Battery at 12%.".into(),
            severity: Severity::Warning,
            timestamp: 42,
        };

        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({
                "type": "battery_low",
                "message": "Battery at 12%.",
                "severity": "warning",
                "timestamp": 42
            })
        );
    }
}
