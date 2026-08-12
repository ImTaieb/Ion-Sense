use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;

use crate::{dispatcher::EventDispatcher, settings::AppSettings};

mod battery_low;
mod downloads;
mod email;
mod friend_message;
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
                dispatcher.clone(),
                stop.clone(),
            ));
        }

        if settings.downloads.enabled {
            workers.push(downloads::spawn(
                settings.downloads.clone(),
                dispatcher.clone(),
                stop.clone(),
            ));
        }

        if settings.email.enabled && !settings.email.username.trim().is_empty() {
            workers.push(email::spawn(
                settings.email.clone(),
                dispatcher.clone(),
                stop.clone(),
            ));
        }

        if settings.discord.enabled {
            workers.push(friend_message::spawn(
                settings.discord.clone(),
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
        let workers: Vec<_> = self.workers.drain(..).collect();
        if workers.is_empty() {
            return;
        }

        // Network clients can be inside a bounded connect/read timeout when a
        // settings save asks them to stop. Reap them away from Tauri's command
        // path so the settings window and tray never freeze while they exit.
        let _ = std::thread::Builder::new()
            .name("ion-detector-reaper".into())
            .spawn(move || {
                for worker in workers {
                    if worker.join().is_err() {
                        eprintln!("Ion Sense detector worker panicked during shutdown");
                    }
                }
            });
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
