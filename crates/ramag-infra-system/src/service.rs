//! One worker owns all OS handles. Delivery has one replaceable snapshot slot.
use crate::{HostCollector, Snapshot};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(1);
pub const SUPPORTED_INTERVALS: [Duration; 4] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
];
/// One bounded delivery slot plus worker controls. Refresh requests coalesce,
/// generation changes interrupt timed waits, and stop prevents another capture.
/// Slow consumers replace older snapshots rather than accumulating a queue.
struct State {
    interval: Duration,
    generation: u64,
    refresh_requested: bool,
    stop: bool,
    latest: Option<Snapshot>,
}
/// The mutex protects controls and owned snapshots only. OS collection happens
/// outside this lock; the condition variable wakes interval/refresh/drop waits.
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}
/// Owns one collector thread and optional Windows temperature session control.
/// The collector is constructed, sampled and destroyed on that thread, allowing
/// native non-Send handles. Drop cancels future sampling and joins the current
/// capture; UI owners must arrange that final wait on a background executor.
pub struct SamplingService {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
    #[cfg(target_os = "windows")]
    thermal: Arc<crate::windows_thermal::Control>,
}
fn valid(interval: Duration) -> Result<(), String> {
    if SUPPORTED_INTERVALS.contains(&interval) {
        Ok(())
    } else {
        Err("Sampling interval must be 500, 1000, 2000, or 5000 milliseconds".into())
    }
}
impl SamplingService {
    /// Starts the worker with a supported cadence. Thread creation or invalid
    /// cadence returns an error; platform field failures appear in snapshots.
    pub fn start(interval: Duration) -> Result<Self, String> {
        valid(interval)?;
        #[cfg(target_os = "windows")]
        let thermal = Arc::new(crate::windows_thermal::Control::default());
        #[cfg(target_os = "windows")]
        let collector_control = thermal.clone();
        let service = Self::spawn(interval, move || {
            let mut host = HostCollector::new();
            #[cfg(target_os = "windows")]
            host.set_temperature_control(collector_control);
            move || host.collect()
        })?;
        #[cfg(target_os = "windows")]
        let service = {
            let mut service = service;
            service.thermal = thermal;
            service
        };
        Ok(service)
    }
    #[cfg(target_os = "windows")]
    pub fn enable_cpu_temperatures(&self) -> Result<(), String> {
        self.thermal.enable()
    }
    #[cfg(target_os = "windows")]
    pub fn disable_cpu_temperatures(&self) {
        self.thermal.disable();
    }
    #[cfg(target_os = "windows")]
    pub fn cpu_temperatures_enabled(&self) -> bool {
        self.thermal.enabled()
    }
    pub fn set_interval(&self, interval: Duration) -> Result<(), String> {
        valid(interval)?;
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.interval = interval;
        state.generation = state.generation.wrapping_add(1);
        self.shared.changed.notify_one();
        Ok(())
    }
    /// Requests one immediate collection; repeated requests collapse into one worker pass.
    pub fn request_refresh(&self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.refresh_requested = true;
        self.shared.changed.notify_one();
    }
    pub fn take_latest(&self) -> Option<Snapshot> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .latest
            .take()
    }
    /// Creates a worker-local collector and samples serially. Missed intervals
    /// are skipped; an immediate request starts at most one extra capture.
    fn spawn<F, C>(interval: Duration, factory: F) -> Result<Self, String>
    where
        F: FnOnce() -> C + Send + 'static,
        C: FnMut() -> Snapshot + 'static,
    {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                interval,
                generation: 0,
                refresh_requested: false,
                stop: false,
                latest: None,
            }),
            changed: Condvar::new(),
        });
        let inner = shared.clone();
        let worker = thread::Builder::new()
            .name("pulse-collector".into())
            .spawn(move || {
                let mut collect = factory();
                loop {
                    if inner.state.lock().unwrap_or_else(|e| e.into_inner()).stop {
                        break;
                    }
                    let started = Instant::now();
                    let snapshot = collect();
                    let mut state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
                    state.latest = Some(snapshot);
                    if state.stop {
                        break;
                    }
                    if state.refresh_requested {
                        state.refresh_requested = false;
                        continue;
                    }
                    let mut generation = state.generation;
                    let mut next = started + state.interval;
                    // A slow backend skips missed sample slots. There is never a catch-up burst.
                    while next <= Instant::now() {
                        next += state.interval;
                    }
                    loop {
                        if state.stop {
                            return;
                        }
                        if state.refresh_requested {
                            state.refresh_requested = false;
                            break;
                        }
                        if state.generation != generation {
                            generation = state.generation;
                            next = started + state.interval;
                        }
                        let now = Instant::now();
                        if now >= next {
                            break;
                        }
                        let (updated, _) = inner
                            .changed
                            .wait_timeout(state, next - now)
                            .unwrap_or_else(|e| e.into_inner());
                        state = updated;
                    }
                }
            })
            .map_err(|e| format!("Could not start collector worker: {e}"))?;
        Ok(Self {
            shared,
            worker: Some(worker),
            #[cfg(target_os = "windows")]
            thermal: Arc::new(crate::windows_thermal::Control::default()),
        })
    }
}
impl Drop for SamplingService {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        self.thermal.disable();
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.stop = true;
        self.shared.changed.notify_one();
        drop(state);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc,
    };
    #[test]
    fn non_send_collector_is_created_used_and_dropped_on_worker()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Local {
            owner: thread::ThreadId,
            events: mpsc::Sender<(&'static str, thread::ThreadId)>,
        }
        impl Drop for Local {
            fn drop(&mut self) {
                assert!(self.events.send(("drop", thread::current().id())).is_ok());
                assert_eq!(self.owner, thread::current().id());
            }
        }
        let caller = thread::current().id();
        let (tx, rx) = mpsc::channel();
        let service = SamplingService::spawn(Duration::from_secs(5), move || {
            let local = std::rc::Rc::new(Local {
                owner: thread::current().id(),
                events: tx,
            });
            assert!(
                local
                    .events
                    .send(("create", thread::current().id()))
                    .is_ok()
            );
            move || {
                assert!(local.events.send(("use", thread::current().id())).is_ok());
                Snapshot::default()
            }
        })?;
        let created = rx.recv_timeout(Duration::from_secs(1))?;
        let used = rx.recv_timeout(Duration::from_secs(1))?;
        drop(service);
        let dropped = rx.recv_timeout(Duration::from_secs(1))?;
        assert_ne!(created.1, caller);
        assert_eq!(created.0, "create");
        assert_eq!(used, ("use", created.1));
        assert_eq!(dropped, ("drop", created.1));
        Ok(())
    }
    #[test]
    fn rejects_other_intervals() {
        assert!(SamplingService::start(Duration::from_millis(42)).is_err());
    }
    #[test]
    fn refresh_request_wakes_the_worker_and_collects_immediately()
    -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();
        let service = SamplingService::spawn(Duration::from_secs(5), move || {
            move || {
                assert!(tx.send(Instant::now()).is_ok());
                Snapshot::default()
            }
        })?;
        rx.recv_timeout(Duration::from_secs(1))?;
        let requested_at = Instant::now();
        service.request_refresh();
        let collected_at = rx.recv_timeout(Duration::from_secs(1))?;
        assert!(collected_at.duration_since(requested_at) < Duration::from_millis(500));
        Ok(())
    }
    #[test]
    fn factory_and_collection_run_on_worker() -> Result<(), Box<dyn std::error::Error>> {
        let caller = std::thread::current().id();
        let (tx, rx) = mpsc::channel();
        let service = SamplingService::spawn(Duration::from_millis(5), move || {
            assert!(tx.send(std::thread::current().id()).is_ok());
            move || Snapshot::default()
        })?;
        assert_ne!(rx.recv_timeout(Duration::from_secs(1))?, caller);
        drop(service);
        Ok(())
    }
    #[test]
    fn slow_backend_never_overlaps_and_delivery_is_latest_only()
    -> Result<(), Box<dyn std::error::Error>> {
        let active = Arc::new(AtomicUsize::new(0));
        let max = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        let count = Arc::new(AtomicU64::new(0));
        let a = active.clone();
        let m = max.clone();
        let c = count.clone();
        let service = SamplingService::spawn(Duration::from_millis(1), move || {
            move || {
                let concurrent = a.fetch_add(1, Ordering::SeqCst) + 1;
                m.fetch_max(concurrent, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(8));
                let sequence = c.fetch_add(1, Ordering::SeqCst) + 1;
                a.fetch_sub(1, Ordering::SeqCst);
                assert!(tx.send(sequence).is_ok());
                Snapshot {
                    sequence,
                    ..Snapshot::default()
                }
            }
        })?;
        for _ in 0..4 {
            rx.recv_timeout(Duration::from_secs(1))?;
        }
        std::thread::sleep(Duration::from_millis(2));
        let latest = service
            .take_latest()
            .ok_or("collector must publish a snapshot")?;
        assert!(latest.sequence >= 4);
        assert!(service.take_latest().is_none());
        assert_eq!(max.load(Ordering::SeqCst), 1);
        drop(service);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        Ok(())
    }
    #[test]
    fn interval_change_interrupts_wait_and_drop_joins() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();
        let service = SamplingService::spawn(Duration::from_secs(5), move || {
            move || {
                assert!(tx.send(()).is_ok());
                Snapshot::default()
            }
        })?;
        rx.recv_timeout(Duration::from_secs(1))?;
        service.set_interval(Duration::from_millis(500))?;
        rx.recv_timeout(Duration::from_secs(2))?;
        let start = std::time::Instant::now();
        drop(service);
        assert!(start.elapsed() < Duration::from_millis(250));
        Ok(())
    }
    #[test]
    fn drop_waits_for_inflight_collection_and_stops_future_work()
    -> Result<(), Box<dyn std::error::Error>> {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let done = Arc::new(AtomicUsize::new(0));
        let d = done.clone();
        let service = SamplingService::spawn(Duration::from_millis(1), move || {
            move || {
                assert!(started_tx.send(()).is_ok());
                assert!(release_rx.recv().is_ok());
                d.fetch_add(1, Ordering::SeqCst);
                Snapshot::default()
            }
        })?;
        started_rx.recv_timeout(Duration::from_secs(1))?;
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let join = std::thread::spawn(move || {
            drop(service);
            assert!(dropped_tx.send(()).is_ok());
        });
        assert!(dropped_rx.recv_timeout(Duration::from_millis(20)).is_err());
        release_tx.send(())?;
        dropped_rx.recv_timeout(Duration::from_secs(1))?;
        assert!(join.join().is_ok());
        assert_eq!(done.load(Ordering::SeqCst), 1);
        Ok(())
    }
}
