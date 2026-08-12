use std::sync::atomic::{AtomicU64, Ordering};
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
    static LAST_TIMESTAMP: AtomicU64 = AtomicU64::new(0);
    let wall_clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);

    let mut previous = LAST_TIMESTAMP.load(Ordering::Relaxed);
    loop {
        let next = wall_clock.max(previous.saturating_add(1));
        match LAST_TIMESTAMP.compare_exchange_weak(
            previous,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return next,
            Err(observed) => previous = observed,
        }
    }
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

    #[test]
    fn generated_timestamps_are_strictly_increasing() {
        let first = IonSenseEvent::new(IonSenseEventType::NewEmail, "one", Severity::Info);
        let second = IonSenseEvent::new(IonSenseEventType::NewEmail, "two", Severity::Info);
        assert!(second.timestamp > first.timestamp);
    }
}
