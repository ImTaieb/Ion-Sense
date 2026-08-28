use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::event::{IonSenseEvent, IonSenseEventType};

/// How long an admitted event suppresses later events with the same key.
/// One window for every detector keeps the behavior predictable: identical
/// system conditions (battery/temperature/download templates) and exact
/// duplicate content messages (email/Discord) are collapsed for 30 seconds.
/// Detectors keep their own latching upstream; this is the central safety net.
pub const SUPPRESS_WINDOW: Duration = Duration::from_secs(30);

/// Upper bound on remembered keys. Real traffic stays far below this; the cap
/// exists so a pathological detector cannot grow the map without limit.
const SUPPRESSION_CACHE_LIMIT: usize = 128;

/// Cloneable input shared by detector modules. Detectors never receive a Tauri
/// handle and therefore cannot bypass this central queue to reach the frontend.
#[derive(Clone, Debug)]
pub struct EventDispatcher {
    sender: mpsc::Sender<IonSenseEvent>,
    pending: Arc<AtomicUsize>,
    suppression: Arc<Mutex<SuppressionState>>,
}

impl EventDispatcher {
    pub fn channel(capacity: usize) -> (Self, mpsc::Receiver<IonSenseEvent>) {
        let (sender, receiver) = mpsc::channel(capacity.max(1));
        (
            Self {
                sender,
                pending: Arc::new(AtomicUsize::new(0)),
                suppression: Arc::new(Mutex::new(SuppressionState::default())),
            },
            receiver,
        )
    }

    /// Debug-only entry point used by the developer test harness. It skips
    /// duplicate suppression so firing the same test event twice always plays.
    #[cfg(debug_assertions)]
    pub async fn dispatch(
        &self,
        event: IonSenseEvent,
    ) -> Result<(), mpsc::error::SendError<IonSenseEvent>> {
        self.pending.fetch_add(1, Ordering::AcqRel);
        if let Err(error) = self.sender.send(event).await {
            self.pending.fetch_sub(1, Ordering::AcqRel);
            return Err(error);
        }
        Ok(())
    }

    pub fn try_dispatch(
        &self,
        event: IonSenseEvent,
    ) -> Result<(), mpsc::error::TrySendError<IonSenseEvent>> {
        if !self.admit(&event, Instant::now()) {
            eprintln!(
                "Ion Sense suppressed a duplicate {:?} event within its cooldown",
                event.event_type
            );
            return Ok(());
        }
        self.pending.fetch_add(1, Ordering::AcqRel);
        if let Err(error) = self.sender.try_send(event) {
            self.pending.fetch_sub(1, Ordering::AcqRel);
            return Err(error);
        }
        Ok(())
    }

    /// Central duplicate gate. Fails open (admits the event) if the state lock
    /// is unavailable: a stuck mutex must never silence real alerts.
    fn admit(&self, event: &IonSenseEvent, now: Instant) -> bool {
        let Ok(mut state) = self.suppression.lock() else {
            return true;
        };
        state.evict_expired(now);
        let key = SuppressionKey::from(event);
        if state.seen.contains_key(&key) {
            return false;
        }
        if state.seen.len() >= SUPPRESSION_CACHE_LIMIT {
            state.seen.clear();
        }
        state.seen.insert(key, now);
        true
    }

    /// Number of accepted events that have not yet completed their HUD cycle.
    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::Acquire)
    }

    pub fn complete_one(&self) {
        let result = self
            .pending
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                pending.checked_sub(1)
            });
        debug_assert!(result.is_ok(), "event dispatcher pending count underflow");
    }
}

#[derive(Debug, Default)]
struct SuppressionState {
    seen: HashMap<SuppressionKey, Instant>,
}

impl SuppressionState {
    fn evict_expired(&mut self, now: Instant) {
        self.seen
            .retain(|_, admitted_at| now.duration_since(*admitted_at) < SUPPRESS_WINDOW);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SuppressionKey {
    event_type: IonSenseEventType,
    context: String,
}

impl From<&IonSenseEvent> for SuppressionKey {
    fn from(event: &IonSenseEvent) -> Self {
        Self {
            event_type: event.event_type,
            context: suppression_context(event),
        }
    }
}

/// The context that decides whether two events of the same type are duplicates.
///
/// System detectors emit template messages where only the readings vary, so the
/// digit-stripped template identifies the condition: "Battery at 12%." and
/// "Battery at 13%." are the same condition within the cooldown window.
///
/// Content detectors forward external text (senders, subjects, chat bodies);
/// only an exact message match counts as a duplicate there, so different
/// senders or different bodies are never collapsed together.
fn suppression_context(event: &IonSenseEvent) -> String {
    match event.event_type {
        IonSenseEventType::BatteryLow
        | IonSenseEventType::Overheating
        | IonSenseEventType::DownloadFinished => normalize_template(&event.message),
        IonSenseEventType::NewEmail
        | IonSenseEventType::FriendMessage
        | IonSenseEventType::PackageDelivered => event.message.clone(),
    }
}

fn normalize_template(message: &str) -> String {
    let mut normalized = String::with_capacity(message.len());
    let mut in_digits = false;
    for character in message.chars() {
        if character.is_ascii_digit() {
            if !in_digits {
                normalized.push('#');
                in_digits = true;
            }
        } else {
            in_digits = false;
            normalized.push(character);
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Severity;

    fn event(event_type: IonSenseEventType, message: &str) -> IonSenseEvent {
        IonSenseEvent {
            event_type,
            message: message.into(),
            severity: Severity::Info,
            timestamp: 0,
        }
    }

    #[test]
    fn pending_tracks_accepted_events_until_completion() {
        let (dispatcher, mut receiver) = EventDispatcher::channel(1);
        assert_eq!(dispatcher.pending(), 0);
        dispatcher
            .try_dispatch(event(IonSenseEventType::BatteryLow, "one"))
            .unwrap();
        assert_eq!(dispatcher.pending(), 1);
        assert!(
            dispatcher
                .try_dispatch(event(IonSenseEventType::BatteryLow, "two"))
                .is_err()
        );
        assert_eq!(dispatcher.pending(), 1);
        assert!(receiver.try_recv().is_ok());
        dispatcher.complete_one();
        assert_eq!(dispatcher.pending(), 0);
    }

    #[test]
    fn suppresses_identical_events_within_the_cooldown() {
        let (dispatcher, _receiver) = EventDispatcher::channel(8);
        let now = Instant::now();
        let duplicate = event(IonSenseEventType::FriendMessage, "Nadia: ready?");
        assert!(dispatcher.admit(&duplicate, now));
        assert!(!dispatcher.admit(&duplicate, now + Duration::from_secs(1)));
        assert!(!dispatcher.admit(&duplicate, now + SUPPRESS_WINDOW - Duration::from_secs(1)));
    }

    #[test]
    fn admits_identical_events_after_the_cooldown() {
        let (dispatcher, _receiver) = EventDispatcher::channel(8);
        let now = Instant::now();
        let duplicate = event(IonSenseEventType::FriendMessage, "Nadia: ready?");
        assert!(dispatcher.admit(&duplicate, now));
        assert!(dispatcher.admit(&duplicate, now + SUPPRESS_WINDOW));
    }

    #[test]
    fn admits_different_event_types_with_equal_messages() {
        let (dispatcher, _receiver) = EventDispatcher::channel(8);
        let now = Instant::now();
        let message = "Something finished.";
        assert!(dispatcher.admit(&event(IonSenseEventType::DownloadFinished, message), now));
        assert!(dispatcher.admit(&event(IonSenseEventType::NewEmail, message), now));
    }

    #[test]
    fn distinguishes_meaningfully_different_content() {
        let (dispatcher, _receiver) = EventDispatcher::channel(8);
        let now = Instant::now();
        assert!(dispatcher.admit(&event(IonSenseEventType::FriendMessage, "Nadia: ok"), now));
        assert!(dispatcher.admit(&event(IonSenseEventType::FriendMessage, "Tarek: ok"), now));
        assert!(dispatcher.admit(&event(IonSenseEventType::NewEmail, "A: Invoice"), now));
        assert!(dispatcher.admit(&event(IonSenseEventType::NewEmail, "A: Receipt"), now));
    }

    #[test]
    fn collapses_system_conditions_that_differ_only_in_readings() {
        let (dispatcher, _receiver) = EventDispatcher::channel(8);
        let now = Instant::now();
        assert!(dispatcher.admit(
            &event(IonSenseEventType::BatteryLow, "Battery at 12%."),
            now
        ));
        assert!(!dispatcher.admit(&event(IonSenseEventType::BatteryLow, "Battery at 9%."), now));
        assert!(dispatcher.admit(
            &event(IonSenseEventType::Overheating, "CPU package reached 94°C."),
            now
        ));
        assert!(!dispatcher.admit(
            &event(IonSenseEventType::Overheating, "CPU package reached 101°C."),
            now
        ));
        // Different sensors are genuinely different conditions and stay separate.
        assert!(dispatcher.admit(
            &event(IonSenseEventType::Overheating, "GPU reached 101°C."),
            now
        ));
    }

    #[test]
    fn suppressed_events_do_not_enter_the_queue_or_pending_count() {
        let (dispatcher, mut receiver) = EventDispatcher::channel(8);
        dispatcher
            .try_dispatch(event(IonSenseEventType::FriendMessage, "Nadia: once"))
            .unwrap();
        dispatcher
            .try_dispatch(event(IonSenseEventType::FriendMessage, "Nadia: once"))
            .unwrap();
        assert_eq!(dispatcher.pending(), 1);
        assert!(receiver.try_recv().is_ok());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn suppression_state_stays_bounded_across_many_conditions() {
        let (dispatcher, _receiver) = EventDispatcher::channel(1);
        let mut now = Instant::now();
        for index in 0..(SUPPRESSION_CACHE_LIMIT * 4) {
            let message = format!("Battery at {index}%.");
            assert!(dispatcher.admit(&event(IonSenseEventType::BatteryLow, &message), now));
            now += SUPPRESS_WINDOW + Duration::from_secs(1);
        }
        let state = dispatcher.suppression.lock().unwrap();
        assert!(state.seen.len() <= SUPPRESSION_CACHE_LIMIT);
    }

    #[test]
    fn digit_runs_collapse_into_a_single_placeholder() {
        assert_eq!(normalize_template("Battery at 12%."), "Battery at #%.");
        assert_eq!(
            normalize_template("CPU 94°C, GPU 101°C"),
            "CPU #°C, GPU #°C"
        );
        assert_eq!(normalize_template("no digits"), "no digits");
    }
}
