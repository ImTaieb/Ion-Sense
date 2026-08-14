use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use tokio::sync::mpsc;

use crate::event::IonSenseEvent;

/// Cloneable input shared by detector modules. Detectors never receive a Tauri
/// handle and therefore cannot bypass this central queue to reach the frontend.
#[derive(Clone, Debug)]
pub struct EventDispatcher {
    sender: mpsc::Sender<IonSenseEvent>,
    pending: Arc<AtomicUsize>,
}

impl EventDispatcher {
    pub fn channel(capacity: usize) -> (Self, mpsc::Receiver<IonSenseEvent>) {
        let (sender, receiver) = mpsc::channel(capacity.max(1));
        (
            Self {
                sender,
                pending: Arc::new(AtomicUsize::new(0)),
            },
            receiver,
        )
    }

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
        self.pending.fetch_add(1, Ordering::AcqRel);
        if let Err(error) = self.sender.try_send(event) {
            self.pending.fetch_sub(1, Ordering::AcqRel);
            return Err(error);
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{IonSenseEventType, Severity};

    fn test_event() -> IonSenseEvent {
        IonSenseEvent::new(IonSenseEventType::BatteryLow, "test", Severity::Warning)
    }

    #[test]
    fn pending_tracks_accepted_events_until_completion() {
        let (dispatcher, mut receiver) = EventDispatcher::channel(1);
        assert_eq!(dispatcher.pending(), 0);
        dispatcher.try_dispatch(test_event()).unwrap();
        assert_eq!(dispatcher.pending(), 1);
        assert!(dispatcher.try_dispatch(test_event()).is_err());
        assert_eq!(dispatcher.pending(), 1);
        assert!(receiver.try_recv().is_ok());
        dispatcher.complete_one();
        assert_eq!(dispatcher.pending(), 0);
    }
}
