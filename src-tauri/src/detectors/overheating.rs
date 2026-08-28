use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::process::Command;

use sysinfo::Components;

use super::wait_until_stopped;
use crate::{
    dispatcher::EventDispatcher,
    event::{IonSenseEvent, IonSenseEventType, Severity},
    settings::TemperatureSettings,
};

pub fn spawn(
    settings: TemperatureSettings,
    dispatcher: EventDispatcher,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("ion-temperature-detector".into())
        .spawn(move || run(settings, dispatcher, stop))
        .expect("failed to spawn temperature detector")
}

fn run(settings: TemperatureSettings, dispatcher: EventDispatcher, stop: Arc<AtomicBool>) {
    // Component discovery and refresh intentionally stay on one OS thread. On
    // Windows the sysinfo backend may use synchronous WMI/ACPI sensor access.
    let mut components = Components::new_with_refreshed_list();
    let threshold = settings.threshold_celsius;
    let rearm_below = threshold - 5.0;
    let mut latched = false;
    let mut consecutive_hot_samples = 0_u8;
    let mut warned_unavailable = false;
    #[cfg(windows)]
    let mut nvidia = NvidiaAvailability::default();

    while !stop.load(Ordering::Acquire) {
        components.refresh(true);
        let hottest = components
            .iter()
            .filter_map(|component| {
                component
                    .temperature()
                    .filter(|temperature| temperature.is_finite())
                    .map(|temperature| (component.label().to_owned(), temperature))
            })
            .max_by(|left, right| left.1.total_cmp(&right.1));
        #[cfg(windows)]
        let hottest = hottest
            .into_iter()
            .chain(nvidia.sample())
            .max_by(|left, right| left.1.total_cmp(&right.1));

        match hottest {
            Some((label, temperature)) => {
                warned_unavailable = false;
                if temperature >= threshold {
                    consecutive_hot_samples = consecutive_hot_samples.saturating_add(1);
                    if !latched && consecutive_hot_samples >= 2 {
                        let sensor = if label.trim().is_empty() {
                            "System sensor"
                        } else {
                            label.trim()
                        };
                        let event = IonSenseEvent::new(
                            IonSenseEventType::Overheating,
                            format!("{sensor} reached {temperature:.0}°C."),
                            Severity::Critical,
                        );
                        if let Err(error) = dispatcher.try_dispatch(event) {
                            match error {
                                tokio::sync::mpsc::error::TrySendError::Closed(_) => return,
                                tokio::sync::mpsc::error::TrySendError::Full(_) => eprintln!(
                                    "Ion Sense dropped a temperature alert because the event queue is full"
                                ),
                            }
                        }
                        latched = true;
                    }
                } else {
                    consecutive_hot_samples = 0;
                    if temperature <= rearm_below {
                        latched = false;
                    }
                }
            }
            None => {
                consecutive_hot_samples = 0;
                if !warned_unavailable {
                    eprintln!(
                        "Ion Sense temperature detector: no readable hardware sensor is exposed"
                    );
                    warned_unavailable = true;
                }
            }
        }

        if wait_until_stopped(&stop, settings.poll_seconds) {
            break;
        }
    }
}

#[cfg(windows)]
fn nvidia_temperature() -> Option<(String, f32)> {
    // Windows does not expose CPU package temperature through a universal API.
    // NVIDIA's driver does expose a reliable hardware temperature, so use it as
    // an additional system-overheat source when ACPI/sysinfo has no reading.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .lines()
        .filter_map(parse_nvidia_sample)
        .max_by(|left, right| left.1.total_cmp(&right.1))
}

/// How long a failed nvidia-smi probe is remembered before trying again.
/// Systems without the NVIDIA driver must not pay a process spawn every poll.
#[cfg(windows)]
const NVIDIA_RETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Remembers whether `nvidia-smi` produced a usable reading so that machines
/// without it pay for a failed process launch only once per retry interval.
#[cfg(windows)]
#[derive(Debug, Default)]
struct NvidiaAvailability {
    usable: Option<bool>,
    last_failed: Option<Instant>,
}

#[cfg(windows)]
impl NvidiaAvailability {
    fn sample(&mut self) -> Option<(String, f32)> {
        if !self.should_probe(Instant::now()) {
            return None;
        }
        let reading = nvidia_temperature();
        self.record(reading.is_some(), Instant::now());
        reading
    }

    fn should_probe(&self, now: Instant) -> bool {
        match self.usable {
            Some(true) | None => true,
            Some(false) => self
                .last_failed
                .is_some_and(|at| now.duration_since(at) >= NVIDIA_RETRY_INTERVAL),
        }
    }

    fn record(&mut self, usable: bool, now: Instant) {
        self.usable = Some(usable);
        self.last_failed = (!usable).then_some(now);
    }
}

#[cfg(windows)]
fn parse_nvidia_sample(line: &str) -> Option<(String, f32)> {
    let (name, temperature) = line.rsplit_once(',')?;
    let temperature = temperature.trim().parse::<f32>().ok()?;
    if !temperature.is_finite() {
        return None;
    }
    let name = name.trim();
    Some((
        if name.is_empty() {
            "NVIDIA GPU".to_owned()
        } else {
            name.to_owned()
        },
        temperature,
    ))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_temperature_rows() {
        assert_eq!(
            parse_nvidia_sample("NVIDIA GeForce RTX 3060, 49"),
            Some(("NVIDIA GeForce RTX 3060".into(), 49.0))
        );
        assert_eq!(parse_nvidia_sample("malformed"), None);
    }

    #[test]
    fn probes_first_then_backs_off_until_the_retry_interval_elapses() {
        let now = Instant::now();
        let mut availability = NvidiaAvailability::default();
        assert!(availability.should_probe(now));

        availability.record(false, now);
        assert!(!availability.should_probe(now + Duration::from_secs(1)));
        assert!(!availability.should_probe(now + NVIDIA_RETRY_INTERVAL - Duration::from_secs(1)));
        assert!(availability.should_probe(now + NVIDIA_RETRY_INTERVAL));

        availability.record(true, now);
        assert!(availability.should_probe(now + Duration::from_secs(1)));
    }

    #[test]
    fn a_successful_probe_is_never_cached_as_a_failure() {
        let now = Instant::now();
        let mut availability = NvidiaAvailability::default();
        availability.record(true, now);
        assert!(availability.usable == Some(true));
        assert!(availability.last_failed.is_none());
    }
}
