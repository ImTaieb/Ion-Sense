mod credentials;
mod detectors;
mod dispatcher;
mod event;
mod settings;

use std::sync::Mutex;

use detectors::DetectorRuntime;
use dispatcher::EventDispatcher;
use settings::AppSettings;
use tauri::Manager;

struct NativeState {
    _detectors: Mutex<DetectorRuntime>,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let settings = AppSettings::default().sanitized();
            let (dispatcher, mut receiver) = EventDispatcher::channel(64);
            let detectors = DetectorRuntime::start(&settings, dispatcher);

            tauri::async_runtime::spawn(async move {
                while let Some(event) = receiver.recv().await {
                    // The final integration commit replaces this diagnostic sink
                    // with the HUD emitter while preserving the same dispatcher.
                    eprintln!("Ion Sense detected event: {event:?}");
                }
            });

            app.manage(NativeState {
                _detectors: Mutex::new(detectors),
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Ion Sense");
}
