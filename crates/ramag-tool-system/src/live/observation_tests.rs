use super::*;

#[test]
fn process_permissions_are_distinct_from_zero_and_other_failures() {
    let zero = collectors::Reading {
        sensor_id: "process:1:1/read".into(),
        value: Some(0.),
        total: None,
        availability: Availability::Available,
        reason: None,
        observations: vec![],
    };
    let denied = collectors::Reading {
        value: None,
        availability: Availability::Failed,
        reason: Some("/proc/1/io: Permission denied (os error 13)".into()),
        ..zero.clone()
    };
    let mut snapshot = Snapshot::default();
    snapshot.processes.push(collectors::ProcessRow {
        identity: ProcessIdentity {
            pid: 1,
            start_time_ticks: 1,
        },
        name: "test".into(),
        user: Some("root".into()),
        user_reason: None,
        cpu_percent: zero.clone(),
        memory_bytes: zero.clone(),
        threads: zero.clone(),
        read_bytes_per_second: denied.clone(),
        write_bytes_per_second: zero,
    });
    let rows = process_views(&snapshot, 0, 2000);
    assert_eq!(
        rows[0].cells[4],
        "No access · /proc/1/io: Permission denied (os error 13)"
    );
    assert_eq!(rows[0].cells[5], "0.0 B/s");
    assert_eq!(rows[0].numeric[2], None);
    assert_eq!(rows[0].numeric[3], Some(0.));
    snapshot.processes[0].read_bytes_per_second.reason =
        Some("/proc/1/io: malformed counter".into());
    assert!(process_views(&snapshot, 0, 2000)[0].cells[4].starts_with("Failed"));
}
#[test]
fn stale_source_in_slow_snapshot_does_not_become_current_at_delivery() {
    let reading = collectors::Reading {
        sensor_id: "cpu:host/usage".into(),
        value: Some(25.),
        total: None,
        availability: Availability::Available,
        reason: None,
        observations: vec![collectors::RawObservation {
            source: "proc".into(),
            captured_ns: 1_000_000_000,
            read_started_ns: Some(900_000_000),
            integers: BTreeMap::new(),
            decimals: BTreeMap::new(),
        }],
    };
    let sample = convert_value(Quantity::Percentage, PhysicalUnit::Percent, &reading, 5000);
    assert_eq!(sample.at_ms, 1000);
}
#[test]
fn process_columns_remain_compact_with_long_denied_reasons() {
    let mut cells = vec![String::new(); 8];
    cells[4] = "Unavailable · Permission denied: ".repeat(12);
    let row = ProcessView {
        identity: ProcessIdentity {
            pid: 1,
            start_time_ticks: 1,
        },
        cells,
        numeric: [None; 5],
    };
    let widths = process_widths(std::slice::from_ref(&row));
    assert!(
        widths.iter().sum::<f32>() <= 1150.,
        "ordinary process columns must fit a desktop window"
    );
    assert!(
        widths[4] <= 160.,
        "an error must not expand the disk column"
    );
}
