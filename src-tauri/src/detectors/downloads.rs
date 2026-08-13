use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RenameMode},
};

use crate::{
    dispatcher::EventDispatcher,
    event::{IonSenseEvent, IonSenseEventType, Severity},
    settings::DownloadsSettings,
};

const TEMP_EXTENSIONS: &[&str] = &["crdownload", "part"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    size: u64,
    modified: Option<SystemTime>,
}

pub fn spawn(
    settings: DownloadsSettings,
    dispatcher: EventDispatcher,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("ion-downloads-detector".into())
        .spawn(move || {
            if let Err(error) = run(settings, dispatcher, stop) {
                eprintln!("Ion Sense downloads detector unavailable: {error:#}");
            }
        })
        .expect("failed to spawn downloads detector")
}

fn run(
    settings: DownloadsSettings,
    dispatcher: EventDispatcher,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let downloads_dir = settings
        .directory
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(dirs::download_dir)
        .context("the operating system did not provide a Downloads directory")?;

    let (raw_sender, raw_receiver) = mpsc::sync_channel::<notify::Result<Event>>(256);
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |result| {
        let _ = raw_sender.try_send(result);
    })
    .context("create file-system watcher")?;
    watcher
        .watch(&downloads_dir, RecursiveMode::NonRecursive)
        .with_context(|| format!("watch {}", downloads_dir.display()))?;

    let mut temp_files = HashMap::<PathBuf, Instant>::new();
    let mut emitted = HashMap::<PathBuf, Instant>::new();

    while !stop.load(Ordering::Acquire) {
        let event = match raw_receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(event)) => event,
            Ok(Err(error)) => {
                eprintln!("Ion Sense downloads watcher error: {error}");
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                prune(&mut temp_files, Duration::from_secs(60 * 60));
                prune(&mut emitted, Duration::from_secs(60));
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };

        let candidates = completion_candidates(&event, &mut temp_files);
        for candidate in candidates {
            if emitted.contains_key(&candidate) || !is_stable_file(&candidate, &stop) {
                continue;
            }

            let event = IonSenseEvent::new(
                IonSenseEventType::DownloadFinished,
                "All downloads completed successfully.",
                Severity::Info,
            );
            if let Err(error) = dispatcher.try_dispatch(event) {
                match error {
                    tokio::sync::mpsc::error::TrySendError::Closed(_) => return Ok(()),
                    tokio::sync::mpsc::error::TrySendError::Full(_) => eprintln!(
                        "Ion Sense dropped a download alert because the event queue is full"
                    ),
                }
            }
            emitted.insert(candidate, Instant::now());
        }
    }

    Ok(())
}

fn completion_candidates(
    event: &Event,
    temp_files: &mut HashMap<PathBuf, Instant>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for path in &event.paths {
        if is_temp(path) {
            temp_files.insert(path.clone(), Instant::now());
        }
    }

    match event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() >= 2 => {
            let from = &event.paths[0];
            let to = &event.paths[1];
            if is_temp(from) && !is_temp(to) {
                temp_files.remove(from);
                candidates.push(to.clone());
            }
        }
        EventKind::Remove(_) => {
            for path in &event.paths {
                if is_temp(path) {
                    temp_files.remove(path);
                    candidates.push(without_temp_extension(path));
                }
            }
        }
        EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            for path in &event.paths {
                if is_temp(path) {
                    continue;
                }
                let matched = temp_files
                    .keys()
                    .find(|temp| without_temp_extension(temp) == *path)
                    .cloned();
                if let Some(temp) = matched {
                    temp_files.remove(&temp);
                    candidates.push(path.clone());
                }
            }
        }
        _ => {}
    }

    candidates
}

fn is_temp(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            TEMP_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

fn without_temp_extension(path: &Path) -> PathBuf {
    match path.file_stem() {
        Some(stem) => path.with_file_name(stem),
        None => path.to_path_buf(),
    }
}

fn is_stable_file(path: &Path, stop: &AtomicBool) -> bool {
    let mut previous: Option<FileStamp> = None;
    let mut stable_samples = 0_u8;

    for _ in 0..5 {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        let current = std::fs::metadata(path)
            .ok()
            .filter(|meta| meta.is_file())
            .map(|metadata| FileStamp {
                size: metadata.len(),
                modified: metadata.modified().ok(),
            });

        if current.is_some() && current == previous {
            stable_samples = stable_samples.saturating_add(1);
            if stable_samples >= 2 {
                return true;
            }
        } else {
            stable_samples = 0;
        }
        previous = current;
        thread::sleep(Duration::from_millis(500));
    }

    false
}

fn prune(entries: &mut HashMap<PathBuf, Instant>, max_age: Duration) {
    entries.retain(|_, seen| seen.elapsed() <= max_age);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_supported_browser_temp_extensions() {
        assert!(is_temp(Path::new("archive.zip.crdownload")));
        assert!(is_temp(Path::new("video.mp4.PART")));
        assert!(!is_temp(Path::new("notes.txt")));
        assert!(!is_temp(Path::new("random.tmp")));
    }

    #[test]
    fn strips_one_download_temp_suffix() {
        assert_eq!(
            without_temp_extension(Path::new("C:/Downloads/archive.zip.crdownload")),
            PathBuf::from("C:/Downloads/archive.zip")
        );
    }
}
