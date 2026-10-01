use super::*;
use ramag_infra_system::{MonitorDescriptor, MonitorKind, RawObservation, SensorKind, Unit};

/// Test data use source timestamps and availability independently of the live OS.
pub(crate) fn host(
    sequence: u64,
    seconds: u64,
    status: Availability,
    value: Option<f64>,
) -> Snapshot {
    Snapshot {
        sequence,
        capture_finished_ns: seconds * 1_000_000_000,
        monitors: vec![MonitorDescriptor {
            id: "cpu:host".into(),
            title: "CPU".into(),
            kind: MonitorKind::Cpu,
            summary_sensor_id: "cpu:host/usage".into(),
        }],
        sensors: vec![SensorDescriptor {
            id: "cpu:host/usage".into(),
            monitor_id: "cpu:host".into(),
            title: "Overall utilization".into(),
            kind: SensorKind::Percentage,
            unit: Unit::Percent,
            source: "test".into(),
            scope: "Host".into(),
            scale: Some(100.0),
        }],
        readings: vec![Reading {
            sensor_id: "cpu:host/usage".into(),
            value,
            total: Some(100.0),
            availability: status,
            reason: None,
            observations: vec![RawObservation {
                source: "test".into(),
                captured_ns: seconds * 1_000_000_000,
                read_started_ns: None,
                integers: Default::default(),
                decimals: Default::default(),
            }],
        }],
        ..Default::default()
    }
}

#[test]
fn unavailable_and_failed_samples_do_not_manufacture_zero() {
    let mut snapshot = MonitorSnapshot::default();
    assert!(snapshot.accept(host(1, 1, Availability::Available, Some(0.0))));
    assert_eq!(
        snapshot
            .latest("cpu:host/usage")
            .and_then(SensorSample::chart_value),
        Some(0.0)
    );
    snapshot.accept(host(2, 2, Availability::WarmingUp, None));
    assert_eq!(
        snapshot
            .latest("cpu:host/usage")
            .map(|sample| sample.status),
        Some(ReadingStatus::WarmingUp)
    );
    snapshot.accept(host(3, 3, Availability::Failed, None));
    assert_eq!(
        snapshot
            .latest("cpu:host/usage")
            .and_then(SensorSample::chart_value),
        None
    );
    snapshot.accept(host(4, 4, Availability::Unavailable, None));
    assert_eq!(
        snapshot
            .latest("cpu:host/usage")
            .and_then(|sample| sample.value),
        None
    );
    snapshot.accept(host(5, 5, Availability::Available, Some(f64::NAN)));
    assert_eq!(
        snapshot
            .latest("cpu:host/usage")
            .map(|sample| sample.status),
        Some(ReadingStatus::Failed)
    );
}

#[test]
fn sequence_and_source_times_keep_history_monotonic_and_bounded() {
    let mut snapshot = MonitorSnapshot::default();
    for sequence in 1..=150 {
        snapshot.accept(host(sequence, sequence, Availability::Available, Some(5.0)));
    }
    let history = snapshot.histories.get("cpu:host/usage");
    assert_eq!(history.map(VecDeque::len), Some(61));
    assert_eq!(
        history.and_then(|h| h.front()).map(|s| s.at_seconds),
        Some(90.0)
    );
    assert!(!snapshot.accept(host(150, 150, Availability::Available, Some(99.0))));
    assert_eq!(
        snapshot.latest("cpu:host/usage").and_then(|s| s.value),
        Some(5.0)
    );
}

#[test]
fn timers_mark_stale_without_appending_or_repeating_measurements() {
    let mut snapshot = MonitorSnapshot::default();
    snapshot.accept(host(1, 1, Availability::Available, Some(25.0)));
    assert!(snapshot.mark_stale(Duration::from_secs(4), RefreshInterval::OneSecond));
    assert_eq!(
        snapshot.histories.get("cpu:host/usage").map(VecDeque::len),
        Some(1)
    );
    assert_eq!(
        snapshot.latest("cpu:host/usage").map(|s| s.status),
        Some(ReadingStatus::Stale)
    );
    assert_eq!(
        snapshot
            .latest("cpu:host/usage")
            .and_then(SensorSample::chart_value),
        None
    );
    assert_eq!(
        snapshot.latest("cpu:host/usage").and_then(|s| s.value),
        Some(25.0)
    );
    assert!(snapshot.collection_stale);
    let reading = host(1, 1, Availability::Available, Some(25.0))
        .readings
        .remove(0);
    assert_eq!(snapshot.process_value(&reading), None);
    snapshot.accept(host(2, 5, Availability::Available, Some(30.0)));
    assert!(!snapshot.collection_stale);
    assert_eq!(snapshot.process_value(&reading), Some(25.0));
}

#[test]
fn process_action_requires_the_confirmation_identity_even_after_pid_reuse() {
    let monitor = SystemMonitor::with_snapshot(MonitorSnapshot::default());
    let identity = StableProcessIdentity {
        pid: 4242,
        start_time_ticks: 10,
    };
    assert_eq!(
        monitor.terminate_process(&identity, "same-name"),
        TerminateResult::Missing { pid: 4242 }
    );
    let self_identity = StableProcessIdentity {
        pid: std::process::id(),
        start_time_ticks: 10,
    };
    assert_eq!(
        monitor.terminate_process(&self_identity, "ramag"),
        TerminateResult::RefusedSelf {
            pid: self_identity.pid
        }
    );
}

/// A cached row may change while confirmation stays open; no OS signal is sent.
#[test]
fn cached_pid_reuse_and_name_changes_are_rejected_before_native_actions() {
    let identity = StableProcessIdentity {
        pid: 4242,
        start_time_ticks: 10,
    };
    let reading = host(1, 1, Availability::Unavailable, None)
        .readings
        .remove(0);
    let mut snapshot = MonitorSnapshot::default();
    snapshot
        .host
        .processes
        .push(ramag_infra_system::ProcessRow {
            identity: StableProcessIdentity {
                pid: 4242,
                start_time_ticks: 20,
            },
            name: "same-name".into(),
            user: None,
            user_reason: None,
            cpu_percent: reading.clone(),
            memory_bytes: reading.clone(),
            read_bytes_per_second: reading.clone(),
            write_bytes_per_second: reading.clone(),
            threads: reading,
        });
    let monitor = SystemMonitor::with_snapshot(snapshot.clone());
    assert_eq!(
        monitor.terminate_process(&identity, "same-name"),
        TerminateResult::ChangedIdentity {
            pid: 4242,
            expected_start_time: 10,
            actual_start_time: 20,
        }
    );
    snapshot.host.processes[0].identity = identity.clone();
    let monitor = SystemMonitor::with_snapshot(snapshot);
    assert_eq!(
        monitor.terminate_process(&identity, "previous-name"),
        TerminateResult::Changed {
            pid: 4242,
            expected_name: "previous-name".into(),
            actual_name: "same-name".into(),
        }
    );
}

#[test]
fn missing_readings_and_invalid_totals_preserve_failure_reasons() {
    let mut snapshot = MonitorSnapshot::default();
    let mut missing = host(1, 1, Availability::Available, Some(1.0));
    missing.readings.clear();
    snapshot.accept(missing);
    let sample = snapshot.latest("cpu:host/usage");
    assert_eq!(
        sample.map(|sample| sample.status),
        Some(ReadingStatus::Unavailable)
    );
    assert!(sample.and_then(|sample| sample.reason.as_deref()).is_some());
    let mut invalid = host(2, 2, Availability::Available, Some(1.0));
    invalid.readings[0].total = Some(f64::INFINITY);
    snapshot.accept(invalid);
    assert_eq!(
        snapshot
            .latest("cpu:host/usage")
            .map(|sample| sample.status),
        Some(ReadingStatus::Failed)
    );
    let mut failed = host(3, 3, Availability::Failed, Some(0.0));
    failed.readings[0].reason = Some("permission denied".into());
    snapshot.accept(failed);
    let sample = snapshot.latest("cpu:host/usage");
    assert_eq!(sample.and_then(SensorSample::chart_value), None);
    assert_eq!(
        sample.and_then(|sample| sample.reason.as_deref()),
        Some("permission denied")
    );
}

#[test]
fn source_time_is_preserved_and_repeated_observations_do_not_extend_history() {
    let mut snapshot = MonitorSnapshot::default();
    let mut first = host(1, 5, Availability::Available, Some(25.0));
    first.readings[0].observations[0].captured_ns = 2_000_000_000;
    snapshot.accept(first.clone());
    assert_eq!(
        snapshot
            .latest("cpu:host/usage")
            .map(|sample| sample.at_seconds),
        Some(2.0)
    );
    first.sequence = 2;
    first.capture_finished_ns = 6_000_000_000;
    snapshot.accept(first);
    assert_eq!(
        snapshot.histories.get("cpu:host/usage").map(VecDeque::len),
        Some(1)
    );
    assert!(snapshot.mark_stale(Duration::ZERO, RefreshInterval::OneSecond));
    snapshot.accept(Snapshot {
        sequence: 3,
        ..Default::default()
    });
    assert!(snapshot.histories.is_empty());
}

/// Reordering an oversized inventory cannot retain additional dormant series.
#[test]
fn inventory_churn_keeps_the_series_count_within_its_hard_limit() {
    let mut snapshot = MonitorSnapshot::default();
    let mut first = host(1, 1, Availability::Available, Some(1.0));
    let descriptor = first.sensors.remove(0);
    first.sensors = (0..MAX_HISTORY_SERIES * 2)
        .map(|index| SensorDescriptor {
            id: format!("sensor-{index}"),
            ..descriptor.clone()
        })
        .collect();
    snapshot.accept(first.clone());
    assert_eq!(snapshot.histories.len(), MAX_HISTORY_SERIES);
    first.sequence = 2;
    first.capture_finished_ns = 2_000_000_000;
    first.sensors.reverse();
    snapshot.accept(first);
    assert_eq!(snapshot.histories.len(), MAX_HISTORY_SERIES);
    assert!(!snapshot.histories.contains_key("sensor-0"));
}
