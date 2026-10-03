use super::*;
#[cfg(test)]
use crate::test_support::TestUnwrapExt;

fn empty_record(sequence: u64) -> Record {
    Record {
        snapshot: Arc::new(Snapshot {
            sequence,
            ..Snapshot::default()
        }),
        accepted_unix_ns: 123,
        render_revision: sequence,
        rendered_at_collector_ms: 1,
        rendered: vec![],
    }
}

#[test]
fn direct_publication_preserves_decoded_snapshot_and_rendered_labels() {
    use ramag_infra_system::{Availability, ProcessIdentity, ProcessRow, RawObservation, Reading};
    let reading = Reading {
        sensor_id: "cpu:host/usage".into(),
        value: Some(12.3456789),
        total: None,
        availability: Availability::Available,
        reason: Some("quoted \"value\"\nµ".into()),
        observations: vec![RawObservation {
            source: "counter".into(),
            captured_ns: u64::MAX,
            read_started_ns: None,
            integers: [("ticks".into(), u64::MAX)].into(),
            decimals: [("invalid".into(), f64::NAN)].into(),
        }],
    };
    let identity = ProcessIdentity {
        pid: 42,
        start_time_ticks: u64::MAX,
    };
    let mut record = empty_record(7);
    let snapshot = Arc::make_mut(&mut record.snapshot);
    snapshot.readings.push(reading.clone());
    snapshot.processes.push(ProcessRow {
        identity: identity.clone(),
        name: "editor\n日本語".into(),
        user: None,
        user_reason: Some("unavailable".into()),
        cpu_percent: reading.clone(),
        memory_bytes: reading.clone(),
        read_bytes_per_second: reading.clone(),
        write_bytes_per_second: reading.clone(),
        threads: reading,
    });
    record.rendered.push(Rendered {
        monitor_id: "processes".into(),
        sensor_id: None,
        process_identity: Some(identity),
        element_id: "process:42".into(),
        label: "editor\n日本語".into(),
        sample: None,
    });
    let expected = serde_json::to_value(Publication {
        schema_version: 1,
        application_pid: std::process::id(),
        accepted_unix_ns: record.accepted_unix_ns,
        render_revision: record.render_revision,
        rendered_at_collector_ms: record.rendered_at_collector_ms,
        snapshot: &record.snapshot,
        rendered: &record.rendered,
    })
    .test_unwrap();
    let dir = std::env::temp_dir().join(format!("pulse-direct-json-{}", std::process::id()));
    let path = dir.join("latest.json");
    publish(&crate::storage::Storage::default(), &path, &record, None).test_unwrap();
    let actual: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).test_unwrap()).test_unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        actual["snapshot"]["readings"][0]["observations"][0]["integers"]["ticks"],
        u64::MAX
    );
    assert!(actual["snapshot"]["readings"][0]["observations"][0]["decimals"]["invalid"].is_null());
    std::fs::remove_dir_all(dir).test_unwrap();
}

#[test]
fn timing_history_is_fixed_capacity_and_accounts_for_latest_slot_overwrites() {
    let writer = Writer {
        shared: Arc::new(Shared {
            state: Mutex::new(State {
                history: Some(History::default()),
                ..State::default()
            }),
            wake: Condvar::new(),
        }),
        worker: None,
        clock: Some(Arc::new(Clock::new())),
    };
    for sequence in 1..=70 {
        writer.submit(empty_record(sequence));
    }
    let state = writer.shared.state.lock().test_unwrap();
    let history = state.history.as_ref().test_unwrap();
    assert_eq!(history.records.len(), 64);
    assert_eq!(history.submitted, 70);
    assert_eq!(history.dequeued, 0);
    assert_eq!(history.overwritten_before_dequeue, 69);
    assert_eq!(history.evicted, 6);
    assert_eq!(history.records.front().test_unwrap().sequence, 7);
    assert_eq!(state.latest.as_ref().test_unwrap().0.snapshot.sequence, 70);
    assert!(
        history
            .records
            .iter()
            .take(63)
            .all(|r| r.outcome == "overwritten_before_dequeue" && r.stages.dequeue_ns.is_none())
    );
    assert_eq!(history.records.back().test_unwrap().outcome, "submitted");
    let mut cloned = history.clone();
    cloned.update(Timing::new(
        writer.clock.as_ref().test_unwrap().clone(),
        1,
        1,
        1,
    ));
    assert_eq!(cloned.records.len(), 64);
    assert_eq!(cloned.records.front().test_unwrap().sequence, 7);
}

#[test]
fn trace_errors_are_bounded_utf8_and_explicit_about_truncation() {
    let error = bounded_error(&"💥".repeat(300));
    assert!(error.len() <= 512);
    assert!(error.ends_with(" [truncated]"));
    assert_eq!(bounded_error("short error"), "short error");
}

#[test]
fn sidecar_failure_never_replaces_the_primary_error_or_leaves_temporary_files() {
    let dir = std::env::temp_dir().join(format!("pulse-timing-both-errors-{}", std::process::id()));
    let path = dir.join("latest.json");
    std::fs::create_dir_all(&path).test_unwrap();
    std::fs::create_dir_all(path.with_extension("publication-timing.json")).test_unwrap();
    let writer = Writer::start_with_trace(path.clone(), true).test_unwrap();
    let shared = writer.shared.clone();
    writer.submit(empty_record(1));
    drop(writer);
    let state = shared.state.lock().test_unwrap();
    let primary = state.error.as_ref().test_unwrap();
    assert!(primary.contains("Save") && primary.contains("latest.json"));
    assert!(!primary.contains("publication-timing"));
    let history = state.history.as_ref().test_unwrap();
    assert_eq!(history.sidecar_errors, 1);
    assert!(
        history
            .last_sidecar_error
            .as_ref()
            .test_unwrap()
            .contains("publication-timing")
    );
    assert!(history.records[0].stages.rename_completed_ns.is_none());
    assert_eq!(std::fs::read_dir(&dir).test_unwrap().count(), 2);
    std::fs::remove_dir_all(dir).test_unwrap();
}

#[test]
fn repeated_sidecar_failures_report_once_and_recover() {
    const CHILD: &str = "SYSTEM_PULSE_TIMING_ERROR_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // Capture the real worker's stderr without replacing its reporting path.
        let output = std::process::Command::new(std::env::current_exe().test_unwrap())
            .args([
                "--exact",
                "diagnostics::tests::repeated_sidecar_failures_report_once_and_recover",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .test_unwrap();
        let stderr = String::from_utf8(output.stderr).test_unwrap();
        assert!(output.status.success(), "child regression failed: {stderr}");
        assert_eq!(
            stderr.matches("Publication timing sidecar:").count(),
            1,
            "{stderr}"
        );
        return;
    }

    let dir = std::env::temp_dir().join(format!(
        "pulse-timing-repeated-error-{}",
        std::process::id()
    ));
    let path = dir.join("latest.json");
    let sidecar = path.with_extension("publication-timing.json");
    std::fs::create_dir_all(&sidecar).test_unwrap();
    let writer = Writer::start_with_trace(path.clone(), true).test_unwrap();
    let shared = writer.shared.clone();
    for sequence in 1..=3 {
        writer.submit(empty_record(sequence));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while shared
            .state
            .lock()
            .test_unwrap()
            .history
            .as_ref()
            .test_unwrap()
            .sidecar_errors
            != sequence
        {
            assert!(
                std::time::Instant::now() < deadline,
                "sidecar failure not observed"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            writer.take_error().is_none(),
            "trace error became a primary failure"
        );
        let primary: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).test_unwrap()).test_unwrap();
        assert_eq!(primary["render_revision"], sequence);
    }
    let last_error = shared
        .state
        .lock()
        .test_unwrap()
        .history
        .as_ref()
        .test_unwrap()
        .last_sidecar_error
        .clone()
        .test_unwrap();
    std::fs::remove_dir(&sidecar).test_unwrap();
    writer.submit(empty_record(4));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !sidecar.is_file() {
        assert!(
            std::time::Instant::now() < deadline,
            "sidecar did not recover"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let recovered: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&sidecar).test_unwrap()).test_unwrap();
    assert_eq!(recovered["history"]["sidecar_errors"], 3);
    assert_eq!(recovered["history"]["last_sidecar_error"], last_error);
    assert!(writer.take_error().is_none());

    // A subsequent primary failure must still reach the original error channel.
    std::fs::remove_file(&path).test_unwrap();
    std::fs::create_dir(&path).test_unwrap();
    writer.submit(empty_record(5));
    drop(writer);
    let state = shared.state.lock().test_unwrap();
    let primary = state.error.as_ref().test_unwrap();
    assert!(primary.contains("latest.json") && !primary.contains("publication-timing"));
    let retained: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&sidecar).test_unwrap()).test_unwrap();
    assert_eq!(retained["history"]["sidecar_errors"], 3);
    assert_eq!(retained["history"]["last_sidecar_error"], last_error);
    assert_eq!(retained["history"]["records"][4]["primary_error"], *primary);
    assert_eq!(std::fs::read_dir(&dir).test_unwrap().count(), 2);
    std::fs::remove_dir_all(dir).test_unwrap();
}

#[test]
fn failed_temp_creation_and_revision_skip_do_not_invent_write_completion() {
    let dir = std::env::temp_dir().join(format!("pulse-timing-partial-{}", std::process::id()));
    std::fs::create_dir_all(&dir).test_unwrap();
    let path = dir.join("latest.json");
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::create_dir_all(&temp).test_unwrap();
    let storage = crate::storage::Storage::default();
    let clock = Arc::new(Clock::new());
    let mut timing = Timing::new(clock.clone(), 1, 1, 123);
    assert!(publish(&storage, &path, &empty_record(1), Some(&mut timing)).is_err());
    assert!(timing.stages.temp_write_started_ns.is_some());
    assert!(timing.stages.temp_write_completed_ns.is_none());
    assert!(timing.stages.rename_started_ns.is_none());
    assert!(
        timing
            .trace_error
            .as_ref()
            .test_unwrap()
            .contains("cleanup")
    );
    std::fs::remove_dir(&temp).test_unwrap();
    storage.write_diagnostic(&path, 3, "newer").test_unwrap();
    let mut timing = Timing::new(clock, 2, 2, 123);
    publish(&storage, &path, &empty_record(2), Some(&mut timing)).test_unwrap();
    assert!(timing.stages.temp_write_started_ns.is_none());
    assert!(timing.stages.rename_completed_ns.is_none());
    assert_eq!(std::fs::read_to_string(path).test_unwrap(), "newer");
    std::fs::remove_dir_all(dir).test_unwrap();
}

#[test]
fn traced_publication_orders_worker_stages_and_identifies_the_replaced_inode() {
    let dir = std::env::temp_dir().join(format!("pulse-timing-order-{}", std::process::id()));
    let path = dir.join("latest.json");
    let writer = Writer::start_with_trace(path.clone(), true).test_unwrap();
    writer.submit(empty_record(1));
    drop(writer);
    let sidecar: serde_json::Value = serde_json::from_slice(
        &std::fs::read(path.with_extension("publication-timing.json")).test_unwrap(),
    )
    .test_unwrap();
    let record = &sidecar["history"]["records"][0];
    assert_eq!(record["sequence"], 1);
    assert_eq!(record["render_revision"], 1);
    assert_eq!(record["accepted_unix_ns"], 123);
    assert_eq!(record["outcome"], "published");
    assert_eq!(sidecar["clock"]["application_pid"], std::process::id());
    assert!(sidecar["clock"]["session_id"].as_str().test_unwrap().len() < 100);
    let stages = &record["stages"];
    let ordered = [
        "submission_started_ns",
        "submission_completed_ns",
        "dequeue_ns",
        "serialization_started_ns",
        "serialization_completed_ns",
        "temp_write_started_ns",
        "temp_write_completed_ns",
        "rename_started_ns",
        "rename_completed_ns",
    ];
    let times: Vec<_> = ordered
        .iter()
        .map(|key| stages[key].as_u64().test_unwrap())
        .collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(stages["json_conversion_started_ns"].is_null());
    assert!(stages["json_conversion_completed_ns"].is_null());
    assert!(
        stages["acceptance_started_ns"].is_null(),
        "unobserved model stages stay absent"
    );
    assert_eq!(
        record["bytes"].as_u64().test_unwrap(),
        std::fs::metadata(&path).test_unwrap().len()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(&path).test_unwrap();
        assert_eq!(record["temp_device"], metadata.dev());
        assert_eq!(record["temp_inode"], metadata.ino());
    }
    assert_eq!(std::fs::read_dir(&dir).test_unwrap().count(), 2);
    std::fs::remove_dir_all(dir).test_unwrap();
}

#[test]
fn traced_rename_failure_has_no_invented_completion_and_keeps_primary_error() {
    let dir = std::env::temp_dir().join(format!("pulse-timing-failure-{}", std::process::id()));
    let path = dir.join("latest.json");
    std::fs::create_dir_all(&path).test_unwrap();
    std::fs::write(path.join("original"), "preserve").test_unwrap();
    let writer = Writer::start_with_trace(path.clone(), true).test_unwrap();
    writer.submit(empty_record(1));
    drop(writer);
    let sidecar: serde_json::Value = serde_json::from_slice(
        &std::fs::read(path.with_extension("publication-timing.json")).test_unwrap(),
    )
    .test_unwrap();
    let record = &sidecar["history"]["records"][0];
    assert_eq!(record["outcome"], "failed");
    assert!(
        record["primary_error"]
            .as_str()
            .test_unwrap()
            .contains("Save")
    );
    assert!(record["stages"]["rename_started_ns"].is_u64());
    assert!(record["stages"]["rename_completed_ns"].is_null());
    assert_eq!(
        std::fs::read_to_string(path.join("original")).test_unwrap(),
        "preserve"
    );
    assert_eq!(std::fs::read_dir(&dir).test_unwrap().count(), 2);
    std::fs::remove_dir_all(dir).test_unwrap();
}
#[test]
fn latest_writer_is_atomic_bounded_and_reports_failure() {
    let dir = std::env::temp_dir().join(format!("pulse-diagnostic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).test_unwrap();
    let path = dir.join("latest.json");
    let writer = Writer::start(path.clone()).test_unwrap();
    assert!(writer.timestamp().is_none());
    assert!(writer.shared.state.lock().test_unwrap().history.is_none());
    for sequence in 1..=20 {
        writer.submit(Record {
            snapshot: Arc::new(Snapshot {
                sequence,
                ..Snapshot::default()
            }),
            accepted_unix_ns: sequence,
            render_revision: sequence,
            rendered_at_collector_ms: sequence,
            rendered: vec![],
        });
    }
    drop(writer);
    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).test_unwrap()).test_unwrap();
    assert_eq!(record["snapshot"]["sequence"], 20);
    assert_eq!(record["application_pid"], std::process::id());
    assert_eq!(std::fs::read_dir(&dir).test_unwrap().count(), 1);
    std::fs::remove_file(path).test_unwrap();
    std::fs::remove_dir(dir).test_unwrap();
}
#[test]
fn failed_diagnostic_write_is_reported() {
    let dir = std::env::temp_dir().join(format!("pulse-diagnostic-failure-{}", std::process::id()));
    std::fs::create_dir_all(&dir).test_unwrap();
    let writer = Writer::start(dir.clone()).test_unwrap();
    writer.submit(Record {
        snapshot: Arc::new(Snapshot {
            sequence: 1,
            ..Snapshot::default()
        }),
        accepted_unix_ns: 1,
        render_revision: 1,
        rendered_at_collector_ms: 1,
        rendered: vec![],
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let error = loop {
        if let Some(error) = writer.take_error() {
            break error;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "writer did not report failure"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert!(error.contains("Save"));
    drop(writer);
    std::fs::remove_dir(dir).test_unwrap();
}
#[test]
fn diagnostic_snapshot_size_is_independent_of_configuration_limit() {
    let dir = std::env::temp_dir().join(format!("pulse-large-diagnostic-{}", std::process::id()));
    let path = dir.join("latest.json");
    let writer = Writer::start(path.clone()).test_unwrap();
    let title = "x".repeat(system_pulse_model::MAX_CONFIGURATION_BYTES + 1);
    writer.submit(Record {
        snapshot: Arc::new(Snapshot {
            sequence: 1,
            monitors: vec![ramag_infra_system::MonitorDescriptor {
                id: "cpu:host".into(),
                title,
                kind: ramag_infra_system::MonitorKind::Cpu,
                summary_sensor_id: "cpu:host/usage".into(),
            }],
            ..Snapshot::default()
        }),
        accepted_unix_ns: 1,
        render_revision: 1,
        rendered_at_collector_ms: 1,
        rendered: vec![],
    });
    drop(writer);
    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).test_unwrap()).test_unwrap();
    assert_eq!(
        record["snapshot"]["monitors"][0]["title"]
            .as_str()
            .test_unwrap()
            .len(),
        system_pulse_model::MAX_CONFIGURATION_BYTES + 1
    );
    assert_eq!(std::fs::read_dir(&dir).test_unwrap().count(), 1);
    std::fs::remove_file(path).test_unwrap();
    std::fs::remove_dir(dir).test_unwrap();
}
