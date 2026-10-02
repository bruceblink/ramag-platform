//! Owned System Pulse samples adapted to the Ramag tool lifecycle.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use ramag_infra_system::{Availability, Reading, SamplingService, SensorDescriptor, Snapshot};

#[path = "monitor/process_order.rs"]
mod process_order;

pub use ramag_infra_system::ProcessIdentity as StableProcessIdentity;

pub const HISTORY_SECONDS: f64 = 60.0;
pub const MAX_VISIBLE_PROCESSES: usize = 120;
const MAX_HISTORY_POINTS: usize = 120;
const MAX_HISTORY_SERIES: usize = 4096;

/// Selects one cached process field without changing collection or process identity.
/// Stable IDs are independent of translated labels; only current finite metrics sort as values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProcessSort {
    #[default]
    Cpu,
    Memory,
    Pid,
    Name,
    User,
    Read,
    Write,
}

impl ProcessSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "内存",
            Self::Pid => "PID",
            Self::Name => "名称",
            Self::User => "用户",
            Self::Read => "读取速率",
            Self::Write => "写入速率",
        }
    }

    pub fn stable_id(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Pid => "pid",
            Self::Name => "name",
            Self::User => "user",
            Self::Read => "read",
            Self::Write => "write",
        }
    }

    /// Stable UI enumeration order shared by sort controls and acceptance checks.
    pub const ALL: [Self; 7] = [
        Self::Cpu,
        Self::Memory,
        Self::Pid,
        Self::Name,
        Self::User,
        Self::Read,
        Self::Write,
    ];
}

/// Defines the ordering of valid values; missing entries remain last in either direction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProcessSortDirection {
    Ascending,
    #[default]
    Descending,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RefreshInterval {
    HalfSecond,
    #[default]
    OneSecond,
    TwoSeconds,
    FiveSeconds,
}

impl RefreshInterval {
    pub fn duration(self) -> Duration {
        match self {
            Self::HalfSecond => Duration::from_millis(500),
            Self::OneSecond => Duration::from_secs(1),
            Self::TwoSeconds => Duration::from_secs(2),
            Self::FiveSeconds => Duration::from_secs(5),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::HalfSecond => "0.5s",
            Self::OneSecond => "1s",
            Self::TwoSeconds => "2s",
            Self::FiveSeconds => "5s",
        }
    }
}

/// Presentation state is independent of the collector's availability enum.
/// Staleness is computed from the monotonic source time; timers cannot invent
/// new measurements or fill missing chart intervals with the last value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadingStatus {
    Current,
    WarmingUp,
    Unavailable,
    Failed,
    Stale,
}

impl ReadingStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Current => "当前",
            Self::WarmingUp => "预热中",
            Self::Unavailable => "不可用",
            Self::Failed => "读取失败",
            Self::Stale => "已过期",
        }
    }
}

/// Each sample retains its source time and physical operands. Only Current
/// samples are charted; unavailable/failed/stale samples create visible gaps.
#[derive(Clone, Debug)]
pub struct SensorSample {
    pub at_seconds: f64,
    pub value: Option<f64>,
    pub total: Option<f64>,
    pub status: ReadingStatus,
    pub reason: Option<String>,
}

impl SensorSample {
    pub fn chart_value(&self) -> Option<f64> {
        (self.status == ReadingStatus::Current)
            .then_some(self.value)
            .flatten()
            .filter(|value| value.is_finite())
    }
}

/// Read-only data cloned for one render. Raw observations and process rows
/// remain owned, and histories are bounded by both time and point/series count.
#[derive(Clone, Debug, Default)]
pub struct MonitorSnapshot {
    pub host: Snapshot,
    pub histories: BTreeMap<String, VecDeque<SensorSample>>,
    pub collection_error: Option<String>,
    /// Ages process metrics and the device inventory as well as chart samples.
    /// The last owned rows stay visible, but their values must not look current.
    pub collection_stale: bool,
}

impl MonitorSnapshot {
    pub fn latest(&self, sensor_id: &str) -> Option<&SensorSample> {
        self.histories
            .get(sensor_id)
            .and_then(|history| history.back())
    }

    /// Accept each sequence once. Retired sensors leave the bounded history
    /// store, and a missing reading appends an unavailable outcome for its id.
    fn accept(&mut self, host: Snapshot) -> bool {
        if host.sequence <= self.host.sequence {
            return false;
        }
        let at = host.capture_finished_ns as f64 / 1_000_000_000.0;
        let sensor_ids = host
            .sensors
            .iter()
            .take(MAX_HISTORY_SERIES)
            .map(|sensor| sensor.id.as_str())
            .collect::<BTreeSet<_>>();
        let readings = host
            .readings
            .iter()
            .map(|reading| (reading.sensor_id.as_str(), reading))
            .collect::<BTreeMap<_, _>>();
        self.histories
            .retain(|id, _| sensor_ids.contains(id.as_str()));
        for descriptor in host.sensors.iter().take(MAX_HISTORY_SERIES) {
            let reading = readings.get(descriptor.id.as_str()).copied();
            let sample = sample_for_reading(reading, at);
            let history = self.histories.entry(descriptor.id.clone()).or_default();
            if history
                .back()
                .is_some_and(|previous| sample.at_seconds <= previous.at_seconds)
            {
                continue;
            }
            history.push_back(sample);
            while history.len() > MAX_HISTORY_POINTS
                || history
                    .front()
                    .is_some_and(|sample| sample.at_seconds < at - HISTORY_SECONDS)
            {
                history.pop_front();
            }
        }
        self.host = host;
        self.collection_error = None;
        self.collection_stale = false;
        true
    }

    fn mark_stale(&mut self, age: Duration, interval: RefreshInterval) -> bool {
        let now = self.host.capture_finished_ns as f64 / 1_000_000_000.0 + age.as_secs_f64();
        let threshold = interval.duration().as_secs_f64() * 3.0;
        let collection_stale = self.host.sequence > 0 && age.as_secs_f64() > threshold;
        let mut changed = self.collection_stale != collection_stale;
        self.collection_stale = collection_stale;
        for sample in self
            .histories
            .values_mut()
            .filter_map(|history| history.back_mut())
        {
            if sample.status == ReadingStatus::Current && now - sample.at_seconds > threshold {
                sample.status = ReadingStatus::Stale;
                changed = true;
            }
        }
        changed
    }

    pub fn descriptor(&self, id: &str) -> Option<&SensorDescriptor> {
        self.host.sensors.iter().find(|sensor| sensor.id == id)
    }

    /// Returns a process metric only while both the snapshot and reading are
    /// current. Kept rows cannot manufacture a zero or hide a stalled worker.
    pub fn process_value(&self, reading: &Reading) -> Option<f64> {
        if self.collection_stale || reading.availability != Availability::Available {
            return None;
        }
        reading.value.filter(|value| value.is_finite())
    }
}

/// Validates finite values at the collector boundary. An available reading
/// without valid operands is a failed source, never a measured zero.
fn sample_for_reading(reading: Option<&Reading>, capture_at: f64) -> SensorSample {
    let Some(reading) = reading else {
        return SensorSample {
            at_seconds: capture_at,
            value: None,
            total: None,
            status: ReadingStatus::Unavailable,
            reason: Some("该传感器本次没有返回数据".into()),
        };
    };
    let valid = reading.value.is_some_and(f64::is_finite)
        && reading
            .total
            .is_none_or(|total| total.is_finite() && total >= 0.0);
    let status = match reading.availability {
        Availability::Available if valid => ReadingStatus::Current,
        Availability::Available | Availability::Failed => ReadingStatus::Failed,
        Availability::WarmingUp => ReadingStatus::WarmingUp,
        Availability::Unavailable => ReadingStatus::Unavailable,
    };
    let at_seconds = if status == ReadingStatus::Current {
        reading
            .observations
            .iter()
            .map(|observation| observation.captured_ns)
            .max()
            .map_or(capture_at, |ns| ns as f64 / 1_000_000_000.0)
    } else {
        capture_at
    };
    SensorSample {
        at_seconds,
        value: (status == ReadingStatus::Current)
            .then_some(reading.value)
            .flatten(),
        total: (status == ReadingStatus::Current)
            .then_some(reading.total)
            .flatten(),
        status,
        reason: if reading.availability == Availability::Available && !valid {
            Some("采集源返回了无效读数".into())
        } else {
            reading.reason.clone()
        },
    }
}

/// The sampling worker owns all OS handles. Runtime teardown transfers the
/// service to a cleanup thread so its final join cannot freeze a GPUI window.
struct Runtime {
    service: Option<SamplingService>,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(service) = self.service.take() {
            std::thread::spawn(move || drop(service));
        }
    }
}

/// The cache lock protects only owned UI data and preferences. `received_at`
/// ages the source timestamp after delivery; redraws never reset freshness.
struct MonitorState {
    snapshot: MonitorSnapshot,
    received_at: Instant,
    interval: RefreshInterval,
    sort: ProcessSort,
    sort_direction: ProcessSortDirection,
}

/// Clones share one worker and one cache. UI methods never perform host reads
/// under this cache lock; sampling and process signals have separate owners.
#[derive(Clone)]
pub struct SystemMonitor {
    runtime: Arc<Runtime>,
    state: Arc<Mutex<MonitorState>>,
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemMonitor {
    /// Starts a worker once; startup errors remain visible in the empty cache.
    pub fn new() -> Self {
        let started = SamplingService::start(RefreshInterval::default().duration());
        let (service, collection_error) = match started {
            Ok(service) => (Some(service), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            runtime: Arc::new(Runtime { service }),
            state: Arc::new(Mutex::new(MonitorState {
                snapshot: MonitorSnapshot {
                    collection_error,
                    ..Default::default()
                },
                received_at: Instant::now(),
                interval: RefreshInterval::default(),
                sort: ProcessSort::default(),
                sort_direction: ProcessSortDirection::default(),
            })),
        }
    }

    pub fn snapshot(&self) -> MonitorSnapshot {
        self.state.lock().snapshot.clone()
    }
    pub fn refresh_interval(&self) -> RefreshInterval {
        self.state.lock().interval
    }
    pub fn process_sort(&self) -> ProcessSort {
        self.state.lock().sort
    }
    pub fn process_sort_direction(&self) -> ProcessSortDirection {
        self.state.lock().sort_direction
    }
    /// Selects a column's initial direction, or reverses the already selected column.
    /// The cache lock changes only presentation preferences; no host read is triggered.
    pub fn set_process_sort(&self, sort: ProcessSort) {
        let mut state = self.state.lock();
        if state.sort == sort {
            state.sort_direction = match state.sort_direction {
                ProcessSortDirection::Ascending => ProcessSortDirection::Descending,
                ProcessSortDirection::Descending => ProcessSortDirection::Ascending,
            };
        } else {
            state.sort = sort;
            state.sort_direction = sort.default_direction();
        }
    }
    /// Changes the worker interval before updating the UI preference; a failed
    /// worker update preserves the previous interval and records its error.
    pub fn set_refresh_interval(&self, interval: RefreshInterval) {
        if let Some(service) = &self.runtime.service
            && let Err(error) = service.set_interval(interval.duration())
        {
            self.state.lock().snapshot.collection_error = Some(error);
            return;
        }
        self.state.lock().interval = interval;
    }

    /// Consume at most the newest worker snapshot and age existing values.
    /// This can run on a GPUI timer because it does no OS collection or waiting.
    pub fn refresh_if_due(&self) -> bool {
        let host = self
            .runtime
            .service
            .as_ref()
            .and_then(SamplingService::take_latest);
        let mut state = self.state.lock();
        let accepted = host.is_some_and(|host| state.snapshot.accept(host));
        if accepted {
            state.received_at = Instant::now();
        }
        let interval = state.interval;
        let age = state.received_at.elapsed();
        state.snapshot.mark_stale(age, interval) || accepted
    }

    pub fn refresh_now(&self) {
        if let Some(service) = &self.runtime.service {
            service.request_refresh();
        }
    }

    #[cfg(target_os = "windows")]
    /// Explicitly controls the optional elevated temperature reader. Call from
    /// the background executor because enabling can start its platform helper.
    pub fn set_cpu_temperatures(&self, enabled: bool) -> Result<(), String> {
        let service = self.runtime.service.as_ref().ok_or("采集服务未启动")?;
        if enabled {
            service.enable_cpu_temperatures()
        } else {
            service.disable_cpu_temperatures();
            Ok(())
        }
    }

    #[cfg(target_os = "windows")]
    pub fn cpu_temperatures_enabled(&self) -> bool {
        self.runtime
            .service
            .as_ref()
            .is_some_and(SamplingService::cpu_temperatures_enabled)
    }

    /// Send to the identity captured when the user opened confirmation. The
    /// collector's native action rechecks identity while pinning the OS handle.
    pub fn terminate_process(
        &self,
        identity: &StableProcessIdentity,
        expected_name: &str,
    ) -> TerminateResult {
        if identity.pid == std::process::id() {
            return TerminateResult::RefusedSelf { pid: identity.pid };
        }
        let snapshot = self.snapshot();
        let Some(row) = snapshot
            .host
            .processes
            .iter()
            .find(|row| row.identity.pid == identity.pid)
        else {
            return TerminateResult::Missing { pid: identity.pid };
        };
        if row.identity != *identity {
            return TerminateResult::ChangedIdentity {
                pid: identity.pid,
                expected_start_time: identity.start_time_ticks,
                actual_start_time: row.identity.start_time_ticks,
            };
        }
        if row.name != expected_name {
            return TerminateResult::Changed {
                pid: identity.pid,
                expected_name: expected_name.into(),
                actual_name: row.name.clone(),
            };
        }
        match ramag_infra_system::process_control::send_signal_checked(
            identity,
            expected_name,
            ramag_infra_system::process_control::ProcessSignal::Kill,
        ) {
            Ok(()) => TerminateResult::Sent {
                pid: identity.pid,
                name: row.name.clone(),
            },
            Err(reason) => TerminateResult::Failed {
                pid: identity.pid,
                name: row.name.clone(),
                reason,
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn with_snapshot(snapshot: MonitorSnapshot) -> Self {
        Self {
            runtime: Arc::new(Runtime { service: None }),
            state: Arc::new(Mutex::new(MonitorState {
                snapshot,
                received_at: Instant::now(),
                interval: RefreshInterval::default(),
                sort: ProcessSort::default(),
                sort_direction: ProcessSortDirection::default(),
            })),
        }
    }
}

/// Reports why the captured confirmation was accepted or refused. A successful
/// signal means the request was sent; it does not promise observed process exit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminateResult {
    RefusedSelf {
        pid: u32,
    },
    Missing {
        pid: u32,
    },
    Changed {
        pid: u32,
        expected_name: String,
        actual_name: String,
    },
    ChangedIdentity {
        pid: u32,
        expected_start_time: u64,
        actual_start_time: u64,
    },
    Sent {
        pid: u32,
        name: String,
    },
    Failed {
        pid: u32,
        name: String,
        reason: String,
    },
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "monitor/process_order_tests.rs"]
mod process_order_tests;

#[cfg(test)]
mod refresh_interval_tests {
    use super::RefreshInterval;
    use std::time::Duration;

    #[test]
    fn sampling_intervals_match_reference_cadences() {
        assert_eq!(
            RefreshInterval::HalfSecond.duration(),
            Duration::from_millis(500)
        );
        assert_eq!(
            RefreshInterval::OneSecond.duration(),
            Duration::from_secs(1)
        );
        assert_eq!(
            RefreshInterval::TwoSeconds.duration(),
            Duration::from_secs(2)
        );
        assert_eq!(
            RefreshInterval::FiveSeconds.duration(),
            Duration::from_secs(5)
        );
        assert_eq!(RefreshInterval::HalfSecond.label(), "0.5s");
    }
}
