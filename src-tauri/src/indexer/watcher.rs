use crossbeam_channel::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const DEBOUNCE_MS: u64 = 1500;

/// Pure 1.5 s trailing-edge debouncer state machine.
#[derive(Debug, Clone, Default)]
pub struct Debouncer {
    last_event_ms: Option<u64>,
    dirty: bool,
}

impl Debouncer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a filesystem or shell change event at `now_ms`.
    pub fn record_event(&mut self, now_ms: u64) {
        self.last_event_ms = Some(now_ms);
        self.dirty = true;
    }

    /// Returns `true` when at least `DEBOUNCE_MS` has elapsed since the last event.
    pub fn should_flush(&self, now_ms: u64) -> bool {
        if !self.dirty {
            return false;
        }
        match self.last_event_ms {
            Some(last) => now_ms.saturating_sub(last) >= DEBOUNCE_MS,
            None => false,
        }
    }

    /// Clears the pending dirty flag after a rebuild has been triggered.
    pub fn mark_flushed(&mut self) {
        self.dirty = false;
        self.last_event_ms = None;
    }
}

/// Handle to the background indexer watcher thread.
pub struct WatcherHandle {
    stop_tx: Sender<()>,
    join: Option<JoinHandle<()>>,
}

impl WatcherHandle {
    /// Signals the watcher thread to stop and waits for it to exit.
    pub fn stop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Spawns a background watcher thread (EcoQoS on Windows) that debounces
/// incoming change notifications by 1.5 s before invoking `on_flush`.
pub fn spawn_watcher<F>(event_rx: Receiver<()>, mut on_flush: F) -> WatcherHandle
where
    F: FnMut() + Send + 'static,
{
    let (stop_tx, stop_rx) = crossbeam_channel::bounded::<()>(1);
    let join = thread::Builder::new()
        .name("launchify-indexer-watcher".into())
        .spawn(move || {
            set_current_thread_eco_qos();
            let start = Instant::now();
            let mut debouncer = Debouncer::new();

            loop {
                if debouncer.dirty {
                    let tick = Duration::from_millis(250);
                    crossbeam_channel::select! {
                        recv(stop_rx) -> _ => break,
                        recv(event_rx) -> msg => {
                            if msg.is_err() {
                                break;
                            }
                            let now_ms = start.elapsed().as_millis() as u64;
                            debouncer.record_event(now_ms);
                        }
                        default(tick) => {}
                    }
                } else {
                    crossbeam_channel::select! {
                        recv(stop_rx) -> _ => break,
                        recv(event_rx) -> msg => {
                            if msg.is_err() {
                                break;
                            }
                            let now_ms = start.elapsed().as_millis() as u64;
                            debouncer.record_event(now_ms);
                        }
                    }
                }

                let now_ms = start.elapsed().as_millis() as u64;
                if debouncer.should_flush(now_ms) {
                    debouncer.mark_flushed();
                    on_flush();
                }
            }
        })
        .ok();

    WatcherHandle { stop_tx, join }
}

#[cfg(windows)]
fn set_current_thread_eco_qos() {
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadInformation, ThreadPowerThrottling,
        THREAD_POWER_THROTTLING_CURRENT_VERSION, THREAD_POWER_THROTTLING_EXECUTION_SPEED,
        THREAD_POWER_THROTTLING_STATE,
    };

    unsafe {
        let state = THREAD_POWER_THROTTLING_STATE {
            Version: THREAD_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: THREAD_POWER_THROTTLING_EXECUTION_SPEED,
            StateMask: THREAD_POWER_THROTTLING_EXECUTION_SPEED,
        };
        let _ = SetThreadInformation(
            GetCurrentThread(),
            ThreadPowerThrottling,
            &state as *const _ as *const _,
            std::mem::size_of::<THREAD_POWER_THROTTLING_STATE>() as u32,
        );
    }
}

#[cfg(not(windows))]
fn set_current_thread_eco_qos() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debouncer_waits_1500ms_after_last_event() {
        let mut d = Debouncer::new();
        assert!(!d.should_flush(0));

        d.record_event(100);
        assert!(!d.should_flush(1000));
        assert!(!d.should_flush(1599));
        assert!(d.should_flush(1600));

        // New event resets the 1.5s window.
        d.record_event(1500);
        assert!(!d.should_flush(2999));
        assert!(d.should_flush(3000));

        d.mark_flushed();
        assert!(!d.should_flush(5000));
    }
}
