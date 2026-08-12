use tokio::sync::mpsc;

use crate::event::IonSenseEvent;

/// Cloneable input shared by detector modules. Detectors never receive a Tauri
/// handle and therefore cannot bypass this central queue to reach the frontend.
#[derive(Clone, Debug)]
pub struct EventDispatcher {
    sender: mpsc::Sender<IonSenseEvent>,
}

impl EventDispatcher {
    pub fn channel(capacity: usize) -> (Self, mpsc::Receiver<IonSenseEvent>) {
        let (sender, receiver) = mpsc::channel(capacity.max(1));
        (Self { sender }, receiver)
    }

    pub async fn dispatch(
        &self,
        event: IonSenseEvent,
    ) -> Result<(), mpsc::error::SendError<IonSenseEvent>> {
        self.sender.send(event).await
    }

    pub fn try_dispatch(
        &self,
        event: IonSenseEvent,
    ) -> Result<(), mpsc::error::TrySendError<IonSenseEvent>> {
        self.sender.try_send(event)
    }
}
