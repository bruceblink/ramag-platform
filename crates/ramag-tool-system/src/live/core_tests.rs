use super::*;
#[cfg(test)]
use crate::test_support::TestUnwrapExt;

use ramag_infra_system::{Availability, Reading, SensorDescriptor, SensorKind, Snapshot, Unit};
fn reading(value: Option<f64>, availability: Availability) -> Reading {
    Reading {
        sensor_id: "cpu:host/usage".into(),
        value,
        total: None,
        availability,
        reason: Some("test reason".into()),
        observations: vec![],
    }
}
#[test]
fn failed_and_delayed_values_keep_status_and_reason() {
    let descriptor = SensorDescriptor {
        id: "cpu:host/usage".into(),
        monitor_id: "cpu:host".into(),
        title: "Usage".into(),
        kind: SensorKind::Percentage,
        unit: Unit::Percent,
        source: "test".into(),
        scope: "host".into(),
        scale: None,
    };
    let failed = convert(&descriptor, &reading(None, Availability::Failed), 1000);
    assert_eq!(failed.status, system_pulse_model::ReadingStatus::Failed);
    assert_eq!(failed.reason.as_deref(), Some("test reason"));
    assert!(failed.value.is_none());
    let mut snapshot = Snapshot {
        sequence: 1,
        capture_finished_ns: 1_000_000_000,
        ..Snapshot::default()
    };
    snapshot.sensors.push(descriptor);
    snapshot
        .readings
        .push(reading(Some(34.), Availability::Available));
    let mut state = LiveState::default();
    let mut workspace = Workspace::new(serde_json::json!({}));
    let mut history = HistoryStore::new(10).test_unwrap();
    state
        .accept(&snapshot, &mut workspace, &mut history, 4000, 1000)
        .test_unwrap();
    assert_eq!(
        history
            .latest("cpu:host", "cpu:host/usage")
            .test_unwrap()
            .status,
        ReadingStatus::Stale
    );
    assert!(
        state
            .accept(&snapshot, &mut workspace, &mut history, 4000, 1000)
            .is_err()
    );
    assert_eq!(
        history
            .samples("cpu:host", "cpu:host/usage")
            .test_unwrap()
            .len(),
        1
    );
}
#[test]
fn census_failure_is_not_zero_and_readable_rows_are_separate() {
    let mut snapshot = Snapshot::default();
    snapshot
        .diagnostics
        .push(ramag_infra_system::BackendDiagnostic {
            backend: "linux-processes".into(),
            availability: Availability::Failed,
            reason: "denied".into(),
        });
    let sample = census(&snapshot);
    assert_eq!(sample.status, ReadingStatus::Failed);
    assert!(sample.value.is_none());
    snapshot.readings.push(Reading {
        sensor_id: "cpu:host/processes".into(),
        ..reading(Some(1200.), Availability::Available)
    });
    assert_eq!(census(&snapshot).value, Some(1200.));
}
#[test]
fn selection_survives_reordering_but_not_exit_or_pid_reuse() {
    use ramag_infra_system::ProcessIdentity;
    let first = ProcessIdentity {
        pid: 12,
        start_time_ticks: 1,
    };
    let second = ProcessIdentity {
        pid: 13,
        start_time_ticks: 2,
    };
    let mut selected = Some(first.clone());
    reconcile_selection(&mut selected, &[second.clone(), first.clone()]);
    assert_eq!(selected, Some(first));
    reconcile_selection(
        &mut selected,
        &[
            ProcessIdentity {
                pid: 12,
                start_time_ticks: 3,
            },
            second,
        ],
    );
    assert_eq!(selected, None);
    reconcile_selection(&mut selected, &[]);
    assert_eq!(selected, None);
}
#[test]
fn restored_absent_metadata_and_discovery_preserve_user_choices() {
    let mut workspace = Workspace::new(serde_json::json!({}));
    workspace.panel_mut("amdgpu:stable").collapsed = true;
    let before = catalog(&workspace);
    assert!(
        before
            .iter()
            .any(|m| m.id == "amdgpu:stable" && m.title.contains("amdgpu:stable"))
    );
    let snapshot = Snapshot {
        sequence: 1,
        monitors: vec![ramag_infra_system::MonitorDescriptor {
            id: "amdgpu:stable".into(),
            title: "GPU".into(),
            kind: ramag_infra_system::MonitorKind::Gpu,
            summary_sensor_id: "amdgpu:stable/usage".into(),
        }],
        ..Snapshot::default()
    };
    LiveState::default()
        .accept(
            &snapshot,
            &mut workspace,
            &mut HistoryStore::new(2).test_unwrap(),
            0,
            1000,
        )
        .test_unwrap();
    assert!(workspace.panels["amdgpu:stable"].collapsed);
    assert!(
        catalog(&workspace)
            .iter()
            .any(|m| m.id == "amdgpu:stable" && m.title.contains("GPU"))
    );
}
