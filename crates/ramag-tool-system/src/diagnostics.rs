//! Opt-in latest rendered snapshot evidence. One worker owns serialization and I/O.
//! `accepted_unix_ns` and the raw snapshot never change during elapsed stale updates.
//! `render_revision` orders publications, including same-snapshot stale transitions.
//! `rendered_at_collector_ms` is the collector-clock coordinate used to age samples;
//! it describes presentation evaluation time, never a new source capture.
use crate::{meters, workspace::Data};
use ramag_infra_system::Snapshot;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};
use system_pulse_model::Sample;

#[path = "diagnostic_timing.rs"]
pub(crate) mod timing;
use timing::{Clock, History, Timing, bounded_error};

#[derive(Serialize)]
pub(crate) struct Rendered {
    monitor_id: String,
    sensor_id: Option<String>,
    process_identity: Option<ramag_infra_system::ProcessIdentity>,
    element_id: String,
    label: String,
    sample: Option<Sample>,
}
pub(crate) struct Record {
    snapshot: Arc<Snapshot>,
    accepted_unix_ns: u64,
    render_revision: u64,
    rendered_at_collector_ms: u64,
    rendered: Vec<Rendered>,
}
impl Record {
    pub(crate) fn new(
        snapshot: Arc<Snapshot>,
        accepted_unix_ns: u64,
        render_revision: u64,
        rendered_at_collector_ms: u64,
        data: &Data,
    ) -> Self {
        let mut rendered = Vec::new();
        for monitor in &data.catalog {
            rendered.push(Rendered {
                monitor_id: monitor.id.clone(),
                sensor_id: Some(monitor.summary.clone()),
                process_identity: None,
                element_id: format!("{}:summary", monitor.id),
                label: meters::summary(monitor, &data.history),
                sample: meters::summary_sample(monitor, &data.history),
            });
            for sensor in &monitor.sensors {
                let sample = data
                    .history
                    .latest(&monitor.id, &sensor.id)
                    .cloned()
                    .unwrap_or_else(|| {
                        crate::live::missing(
                            sensor.quantity,
                            sensor.unit,
                            "Sensor or device absent",
                            0,
                        )
                    });
                rendered.push(Rendered {
                    monitor_id: monitor.id.clone(),
                    sensor_id: Some(sensor.id.clone()),
                    process_identity: None,
                    element_id: format!("{}:value:{}", monitor.id, sensor.id),
                    label: meters::sensor_label(monitor, sensor, &sample),
                    sample: Some(sample),
                });
            }
        }
        for process in &data.processes {
            for (column, label) in process.cells.iter().enumerate() {
                rendered.push(Rendered {
                    monitor_id: "processes".into(),
                    sensor_id: None,
                    process_identity: Some(process.identity.clone()),
                    element_id: format!(
                        "process:{}:{}:cell:{column}",
                        process.identity.pid, process.identity.start_time_ticks
                    ),
                    label: label.clone(),
                    sample: None,
                });
            }
        }
        Self {
            snapshot,
            accepted_unix_ns,
            render_revision,
            rendered_at_collector_ms,
            rendered,
        }
    }
}
#[derive(Default)]
struct State {
    latest: Option<(Record, Option<Timing>)>,
    stopping: bool,
    error: Option<String>,
    history: Option<History>,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}
pub(crate) struct Writer {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
    clock: Option<Arc<Clock>>,
}
impl Writer {
    pub(crate) fn from_env() -> Result<Option<Self>, String> {
        std::env::var_os("SYSTEM_PULSE_DIAGNOSTICS_PATH")
            .map(|path| {
                if std::env::var_os("SYSTEM_PULSE_DIAGNOSTICS_TRACE").is_some_and(|v| v == "1") {
                    Self::start_with_trace(path.into(), true)
                } else {
                    Self::start(path.into())
                }
            })
            .transpose()
    }
    pub(crate) fn start(path: PathBuf) -> Result<Self, String> {
        Self::start_with_trace(path, false)
    }
    pub(crate) fn start_with_trace(path: PathBuf, trace: bool) -> Result<Self, String> {
        let clock = trace.then(|| Arc::new(Clock::new()));
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                history: trace.then(History::default),
                ..State::default()
            }),
            wake: Condvar::new(),
        });
        let worker_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name("pulse-diagnostics".into())
            .spawn(move || run_worker(worker_shared, path))
            .map_err(|e| format!("Start snapshot diagnostic writer: {e}"))?;
        Ok(Self {
            shared,
            worker: Some(worker),
            clock,
        })
    }
    pub(crate) fn timestamp(&self) -> Option<u64> {
        self.clock.as_ref().map(|clock| clock.now())
    }
    pub(crate) fn submit(&self, record: Record) {
        self.submit_timed(record, None, None);
    }
    pub(crate) fn submit_timed(
        &self,
        record: Record,
        model: Option<(u64, u64)>,
        construction_started: Option<u64>,
    ) {
        let timing = self.clock.as_ref().map(|clock| {
            let mut timing = Timing::new(
                clock.clone(),
                record.snapshot.sequence,
                record.render_revision,
                record.accepted_unix_ns,
            );
            if let Some((started, completed)) = model {
                timing.stages.acceptance_started_ns = Some(started);
                timing.stages.model_completed_ns = Some(completed);
            }
            timing.stages.construction_started_ns = construction_started;
            if construction_started.is_some() {
                timing.stages.construction_completed_ns = Some(clock.now());
            }
            timing.stages.submission_started_ns = Some(clock.now());
            timing
        });
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        let overwritten = state
            .latest
            .as_ref()
            .map(|(record, _)| record.render_revision);
        if let Some(timing) = &timing
            && let Some(history) = &mut state.history
        {
            history.submit(timing.clone(), overwritten);
        }
        state.latest = Some((record, timing));
        // The slot is installed. Stamp before releasing the lock so dequeue
        // cannot precede observed submission completion.
        if let Some((_, Some(timing))) = &mut state.latest {
            timing.stages.submission_completed_ns = Some(timing.clock.now());
            let completed = timing.stages.submission_completed_ns;
            if let Some(history) = &mut state.history
                && let Some(record) = history.records.back_mut()
            {
                record.stages.submission_completed_ns = completed;
            }
        }
        drop(state);
        self.shared.wake.notify_one();
    }
    pub(crate) fn take_error(&self) -> Option<String> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .error
            .take()
    }
}

#[derive(Serialize)]
struct Publication<'a> {
    schema_version: u32,
    application_pid: u32,
    accepted_unix_ns: u64,
    render_revision: u64,
    rendered_at_collector_ms: u64,
    snapshot: &'a Snapshot,
    rendered: &'a [Rendered],
}

fn publish(
    storage: &crate::storage::Storage,
    path: &std::path::Path,
    record: &Record,
    mut timing: Option<&mut Timing>,
) -> Result<(), String> {
    if let Some(t) = &mut timing {
        t.stages.serialization_started_ns = Some(t.clock.now());
    }
    let json = serde_json::to_string(&Publication {
        schema_version: 1,
        application_pid: std::process::id(),
        accepted_unix_ns: record.accepted_unix_ns,
        render_revision: record.render_revision,
        rendered_at_collector_ms: record.rendered_at_collector_ms,
        snapshot: record.snapshot.as_ref(),
        rendered: &record.rendered,
    })
    .map_err(|e| format!("Serialize snapshot diagnostics: {e}"))?;
    if let Some(t) = &mut timing {
        t.stages.serialization_completed_ns = Some(t.clock.now());
        t.bytes = Some(json.len() as u64);
    }
    match timing {
        Some(timing) => storage.write_diagnostic_timed(path, record.render_revision, &json, timing),
        None => storage.write_diagnostic(path, record.render_revision, &json),
    }
}

fn run_worker(shared: Arc<Shared>, path: PathBuf) {
    let storage = crate::storage::Storage::default();
    loop {
        let (record, mut timing) = {
            let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
            while state.latest.is_none() && !state.stopping {
                state = shared.wake.wait(state).unwrap_or_else(|p| p.into_inner());
            }
            let Some((record, mut timing)) = state.latest.take() else {
                break;
            };
            if let Some(t) = &mut timing {
                t.stages.dequeue_ns = Some(t.clock.now());
                t.outcome = "dequeued";
                if let Some(history) = &mut state.history {
                    history.dequeued = history.dequeued.saturating_add(1);
                    history.update(t.clone());
                }
            }
            (record, timing)
        };
        let result = publish(&storage, &path, &record, timing.as_mut());
        if let Err(error) = &result {
            eprintln!("{error}");
            shared.state.lock().unwrap_or_else(|p| p.into_inner()).error = Some(error.clone());
        }
        if let Some(mut timing) = timing {
            timing.outcome = if result.is_err() {
                "failed"
            } else if timing.stages.rename_completed_ns.is_some() {
                "published"
            } else {
                "skipped_older_revision"
            };
            timing.primary_error = result.as_ref().err().map(|e| bounded_error(e));
            let clock = timing.clock.clone();
            let history = {
                let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(history) = state.history.as_mut() {
                    history.update(timing);
                    history.clone()
                } else {
                    Default::default()
                }
            };
            let sidecar = serde_json::to_string(&serde_json::json!({
                "schema_version": 1, "instrumented": true, "capacity": timing::CAPACITY,
                "clock": clock.as_ref(), "history": history,
                "coverage": "History captured after a worker attempt; pending entries and interrupted later stages may be incomplete. Null means unobserved, never zero-duration completion."
            }));
            let result = sidecar.map_err(|e| e.to_string()).and_then(|json| {
                storage.write_diagnostic(
                    &path.with_extension("publication-timing.json"),
                    record.render_revision,
                    &json,
                )
            });
            if let Err(error) = result {
                let error = bounded_error(&error);
                let mut state = shared.state.lock().unwrap_or_else(|p| p.into_inner());
                let first_error = if let Some(history) = state.history.as_mut() {
                    let first_error = (history.sidecar_errors == 0).then(|| error.clone());
                    history.sidecar_errors = history.sidecar_errors.saturating_add(1);
                    history.last_sidecar_error = Some(error);
                    first_error
                } else {
                    None
                };
                drop(state);
                if let Some(error) = first_error {
                    eprintln!("Publication timing sidecar: {error}");
                }
            }
        }
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .stopping = true;
        self.shared.wake.notify_one();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            eprintln!("Snapshot diagnostic worker panicked");
        }
    }
}

#[cfg(test)]
#[path = "diagnostics/tests.rs"]
mod tests;
