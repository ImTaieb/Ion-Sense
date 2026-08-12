use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;

use crate::{dispatcher::EventDispatcher, settings::AppSettings};

mod battery_low;
mod overheating;

pub struct DetectorRuntime {
    stop: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

impl DetectorRuntime {
    pub fn start(settings: &AppSettings, dispatcher: EventDispatcher) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let mut workers = Vec::new();

        if settings.battery.enabled {
            workers.push(battery_low::spawn(
                settings.battery.clone(),
                dispatcher.clone(),
                stop.clone(),
            ));
        }

        if settings.temperature.enabled {
            workers.push(overheating::spawn(
                settings.temperature.clone(),
                dispatcher,
                stop.clone(),
            ));
        }

        Self { stop, workers }
    }
}

impl Drop for DetectorRuntime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

pub(crate) fn wait_until_stopped(stop: &AtomicBool, seconds: u64) -> bool {
    for _ in 0..seconds.max(1) {
        if stop.load(Ordering::Acquire) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    stop.load(Ordering::Acquire)
}
