//! Owned physical observations. Timestamps share a collector-local monotonic origin.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Owns one complete host sample; callers may move it across threads without
/// retaining native handles. Sensor and monitor IDs identify physical sources,
/// while process identities identify one lifetime of a PID. Capture timestamps
/// share this collector's monotonic origin and sequence increases per capture.
/// Per-field failures stay in readings and diagnostics; absent hardware does
/// not produce an invented device. Consumers bound retained history separately.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub sequence: u64,
    pub clock_anchor: Option<ClockAnchor>,
    pub capture_started_ns: u64,
    pub capture_finished_ns: u64,
    pub monitors: Vec<MonitorDescriptor>,
    pub sensors: Vec<SensorDescriptor>,
    pub readings: Vec<Reading>,
    pub processes: Vec<ProcessRow>,
    pub diagnostics: Vec<BackendDiagnostic>,
    #[serde(default)]
    pub network_attribution: Option<NetworkAttribution>,
    /// Linux default-route interface, falling back to a physical interface.
    #[serde(default)]
    pub preferred_network_monitor_id: Option<String>,
}
/// Bounds the wall-clock observation with two monotonic timestamps. Trends use
/// monotonic time; a wall-clock adjustment cannot change a counter interval.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClockAnchor {
    pub unix_ns: u64,
    pub monotonic_before_ns: u64,
    pub monotonic_after_ns: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorKind {
    Cpu,
    Gpu,
    Memory,
    Volume,
    Network,
}
/// Device inventory entry. Its stable ID groups sensors; summary_sensor_id is
/// a preferred primary reading, not a promise that telemetry is available.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonitorDescriptor {
    pub id: String,
    pub title: String,
    pub kind: MonitorKind,
    pub summary_sensor_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensorKind {
    Percentage,
    Capacity,
    Rate,
    Temperature,
    Counter,
    Frequency,
    Power,
    Fan,
    Duration,
    Scalar,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Percent,
    Bytes,
    BytesPerSecond,
    Celsius,
    Hertz,
    Watts,
    Rpm,
    Count,
    CountPerSecond,
    Milliseconds,
    Seconds,
    Load,
}
/// Describes one stable sensor independently of its latest value. `unit` is
/// the physical unit after conversion; `source` and `scope` explain where and
/// what the backend measured. Optional `scale` is a physical capacity or bound,
/// never a fallback value for unavailable data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorDescriptor {
    pub id: String,
    pub monitor_id: String,
    pub title: String,
    pub kind: SensorKind,
    pub unit: Unit,
    pub source: String,
    pub scope: String,
    pub scale: Option<f64>,
}
/// Available includes real zero; WarmingUp needs a second successful counter
/// observation. Unavailable describes unsupported or missing sources; Failed
/// records a read or validation error. Consumers additionally compute staleness
/// from source time and must not convert any non-available outcome into zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Availability {
    Available,
    WarmingUp,
    Unavailable,
    Failed,
}
/// Owned operands and query timing retained for diagnosis and rate derivation.
/// These maps contain metric operands, not process arguments or environment
/// values. Failed counter observations invalidate the corresponding baseline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawObservation {
    pub source: String,
    /// Time this owned observation was recorded, in collector-relative monotonic nanoseconds.
    pub captured_ns: u64,
    /// Optional start of the source query; when present, bounds the source read.
    pub read_started_ns: Option<u64>,
    pub integers: BTreeMap<String, u64>,
    pub decimals: BTreeMap<String, f64>,
}
/// One field's value, optional physical capacity, state and failure reason.
/// Observations preserve the inputs and measured elapsed time. A missing value
/// is not a measured zero; consumers validate finite operands before plotting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reading {
    pub sensor_id: String,
    pub value: Option<f64>,
    pub total: Option<f64>,
    pub availability: Availability,
    pub reason: Option<String>,
    pub observations: Vec<RawObservation>,
}
/// Identifies a process lifetime without lossy rounding: Linux uses procfs
/// start ticks, Windows the full creation FILETIME, macOS microsecond birth
/// time. Zero means unverified and cannot authorize a process action. Native
/// actions pin/recheck this identity and the displayed name before signaling.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub start_time_ticks: u64,
}
/// Owned task-manager row. Each metric has its own availability, and missing
/// user lookup keeps a reason. Sampling does not collect command arguments,
/// credentials or environment variables; action confirmation captures identity
/// and name separately so later sorting and refresh cannot change the target.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessRow {
    pub identity: ProcessIdentity,
    pub name: String,
    pub user: Option<String>,
    pub user_reason: Option<String>,
    pub cpu_percent: Reading,
    pub memory_bytes: Reading,
    pub read_bytes_per_second: Reading,
    pub write_bytes_per_second: Reading,
    pub threads: Reading,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendDiagnostic {
    pub backend: String,
    pub availability: Availability,
    pub reason: String,
}

/// Shared captured inputs for every per-interface established-TCP connection count.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkAttribution {
    pub interface_addresses: InterfaceAddressObservation,
    pub tcp_v4: TcpTableObservation,
    pub tcp_v6: TcpTableObservation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceQuery {
    pub source: String,
    pub read_started_ns: u64,
    pub captured_ns: u64,
    pub availability: Availability,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterfaceAddressObservation {
    pub query: SourceQuery,
    /// The names and addresses returned by sysinfo, before ownership deduplication.
    pub interfaces: BTreeMap<String, Vec<String>>,
}
#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpVersion {
    Ipv4,
    Ipv6,
}
#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordByteOrder {
    LittleEndian,
    BigEndian,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TcpTableObservation {
    pub query: SourceQuery,
    pub address_family: IpVersion,
    /// Linux prints native-endian 32-bit address words in these procfs files.
    pub word_byte_order: WordByteOrder,
    pub rows: Vec<TcpLocalRow>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpLocalRow {
    pub line_number: u64,
    /// Original address digits; absent when the row structure is missing or malformed.
    pub local_address_hex: Option<String>,
    /// Original two hexadecimal digits; absent when the row structure is missing or malformed.
    pub state_hex: Option<String>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_without_network_attribution_remain_compatible()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut old = serde_json::to_value(Snapshot::default())?;
        let object = old
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("snapshot must serialize as an object"))?;
        object.remove("network_attribution");
        object.remove("preferred_network_monitor_id");
        let restored: Snapshot = serde_json::from_value(old)?;
        assert!(restored.network_attribution.is_none());
        assert!(restored.preferred_network_monitor_id.is_none());
        assert!(Snapshot::default().network_attribution.is_none());
        Ok(())
    }
}
