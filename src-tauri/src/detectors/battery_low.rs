use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};
use battery::{Manager, State, units::ratio::percent as percent_unit};

use super::wait_until_stopped;
use crate::{
    dispatcher::EventDispatcher,
    event::{IonSenseEvent, IonSenseEventType, Severity},
    settings::BatterySettings,
};

#[derive(Debug, Clone, Copy)]
struct BatterySample {
    percent: f32,
    is_discharging: bool,
}

pub fn spawn(
    settings: BatterySettings,
    dispatcher: EventDispatcher,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("ion-battery-detector".into())
        .spawn(move || run(settings, dispatcher, stop))
        .expect("failed to spawn battery detector")
}

fn run(settings: BatterySettings, dispatcher: EventDispatcher, stop: Arc<AtomicBool>) {
    let manager = match Manager::new().context("initialize battery manager") {
        Ok(manager) => manager,
        Err(error) => {
            eprintln!("Ion Sense battery detector unavailable: {error:#}");
            return;
        }
    };

    let threshold = settings.threshold_percent;
    let rearm_above = (threshold + 3.0).min(100.0);
    let mut previous: Option<BatterySample> = None;
    let mut armed = true;

    while !stop.load(Ordering::Acquire) {
        match sample(&manager) {
            Ok(Some(current)) => {
                if current.percent > rearm_above {
                    armed = true;
                }

                let crossed = previous.is_some_and(|last| {
                    (last.percent > threshold && current.percent <= threshold)
                        || (!last.is_discharging
                            && current.is_discharging
                            && current.percent <= threshold)
                });

                if armed && crossed && current.is_discharging {
                    let event = IonSenseEvent::new(
                        IonSenseEventType::BatteryLow,
                        format!(
                            "Battery at {:.0}%. Connect a power source.",
                            current.percent
                        ),
                        Severity::Warning,
                    );
                    if dispatcher.dispatch_blocking(event).is_err() {
                        return;
                    }
                    armed = false;
                }

                previous = Some(current);
            }
            Ok(None) => {
                // Desktops and machines whose firmware exposes no battery land here.
            }
            Err(error) => eprintln!("Ion Sense battery sample failed: {error:#}"),
        }

        if wait_until_stopped(&stop, settings.poll_seconds) {
            break;
        }
    }
}

fn sample(manager: &Manager) -> Result<Option<BatterySample>> {
    let mut lowest: Option<BatterySample> = None;
    let batteries = manager.batteries().context("enumerate batteries")?;

    for candidate in batteries {
        let battery = candidate.context("read battery")?;
        let percent = battery.state_of_charge().get::<percent_unit>();
        if !percent.is_finite() {
            continue;
        }

        let reading = BatterySample {
            percent: percent.clamp(0.0, 100.0),
            is_discharging: matches!(
                battery.state(),
                State::Discharging | State::Empty | State::Unknown
            ),
        };

        if lowest.is_none_or(|known| reading.percent < known.percent) {
            lowest = Some(reading);
        }
    }

    Ok(lowest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_uses_strict_previous_and_inclusive_current_values() {
        let threshold = 20.0;
        let previous = BatterySample {
            percent: 21.0,
            is_discharging: true,
        };
        let current = BatterySample {
            percent: 20.0,
            is_discharging: true,
        };
        assert!(previous.percent > threshold && current.percent <= threshold);
    }
}
