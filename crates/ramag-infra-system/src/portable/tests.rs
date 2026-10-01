use super::*;
#[test]
fn filesystem_capacity_uses_fresh_block_operands_and_preserves_query_window() {
    let window = QueryWindow {
        started_ns: 10,
        captured_ns: 20,
    };
    let first = filesystem_capacity_reading("volume/capacity", Ok((100, 40, 4096)), window);
    assert_eq!(first.value, Some(60.0 * 4096.0));
    assert_eq!(first.total, Some(100.0 * 4096.0));
    assert_eq!(first.observations[0].integers["free_blocks"], 40);
    assert_eq!(first.observations[0].read_started_ns, Some(10));
    assert_eq!(first.observations[0].captured_ns, 20);
    let second = filesystem_capacity_reading("volume/capacity", Ok((100, 30, 4096)), window);
    assert_eq!(second.value, Some(70.0 * 4096.0));
}

#[test]
fn filesystem_capacity_failure_and_invalid_counters_never_publish_zero_or_old_values() {
    let window = QueryWindow {
        started_ns: 10,
        captured_ns: 20,
    };
    for result in [
        Err("mount disappeared".into()),
        Ok((10, 11, 4096)),
        Ok((u64::MAX, 0, 4096)),
        Ok((10, 0, 0)),
        Ok((0, 0, 4096)),
    ] {
        let reading = filesystem_capacity_reading("volume/capacity", result, window);
        assert!(reading.value.is_none());
        assert!(reading.total.is_none());
        assert_ne!(reading.availability, Availability::Available);
        assert!(reading.reason.is_some());
    }
    let recovered = filesystem_capacity_reading("volume/capacity", Ok((10, 10, 4096)), window);
    assert_eq!(recovered.value, Some(0.0));
    assert_eq!(recovered.availability, Availability::Available);
}
#[test]
fn common_backend_exposes_host_memory_and_real_process_identity()
-> Result<(), Box<dyn std::error::Error>> {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        return Ok(());
    }
    let mut c = HostCollector::new();
    let mut s = Snapshot::default();
    c.collect_portable(&mut s);
    s = Snapshot::default();
    c.collect_portable(&mut s);
    assert!(
        s.network_attribution.is_none(),
        "the common backend has no Linux TCP table evidence"
    );
    assert!(
        s.readings
            .iter()
            .any(|r| r.sensor_id == "memory:host/used" && r.total.is_some_and(|v| v > 0.0))
    );
    assert!(
        s.processes
            .iter()
            .any(|p| p.identity.pid == std::process::id())
    );
    let finished_ns = c.now();
    for observation in s
        .readings
        .iter()
        .chain(s.processes.iter().flat_map(|p| {
            [
                &p.cpu_percent,
                &p.memory_bytes,
                &p.read_bytes_per_second,
                &p.write_bytes_per_second,
                &p.threads,
            ]
        }))
        .flat_map(|reading| &reading.observations)
    {
        assert!(
            observation
                .read_started_ns
                .is_some_and(|start| start <= observation.captured_ns)
        );
        assert!(observation.captured_ns <= finished_ns);
    }
    let uptime_end = s
        .readings
        .iter()
        .find(|r| r.sensor_id == "cpu:host/uptime")
        .ok_or_else(|| std::io::Error::other("host uptime reading must exist"))?
        .observations[0]
        .captured_ns;
    let latest_disk_observation = s
        .readings
        .iter()
        .flat_map(|r| &r.observations)
        .filter(|o| o.source.starts_with("sysinfo::Disk"))
        .max_by_key(|observation| observation.captured_ns);
    if let Some(disk_observation) = latest_disk_observation {
        assert!(
            disk_observation
                .read_started_ns
                .is_some_and(|started| started >= uptime_end),
            "disk source {} window {:?}-{} precedes uptime end {uptime_end}",
            disk_observation.source,
            disk_observation.read_started_ns,
            disk_observation.captured_ns
        );
    }
    Ok(())
}

#[test]
fn process_count_waits_for_native_identity_warmup_instead_of_reporting_zero() {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        return;
    }
    let mut collector = HostCollector::new();
    let mut snapshot = Snapshot::default();
    collector.collect_portable(&mut snapshot);
    if collector.process_system.processes().is_empty() {
        return;
    }
    let count = snapshot
        .readings
        .iter()
        .find(|reading| reading.sensor_id == "cpu:host/processes");
    assert!(count.is_some());
    let Some(count) = count else { return };
    assert_eq!(count.value, None);
    assert_eq!(count.availability, Availability::WarmingUp);
    assert!(
        count
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("identit"))
    );
}

#[test]
fn delayed_queries_stamp_completion_and_use_measured_disk_rate() {
    use std::cell::Cell;
    let clock = Cell::new(800_000_000u64);
    let now = || clock.get();
    let (first, first_window) = query(now, || {
        clock.set(clock.get() + 200_000_000);
        1000
    });
    assert_eq!(first_window.captured_ns, 1_000_000_000);
    let mut counters = Counters::default();
    let first_reading = counters.derive(
        "disk/read",
        Ok(api_raw(
            "sysinfo::Disk::usage",
            first_window,
            [("bytes", first)],
        )),
        |a, b, e| Ok(delta(a, b, "bytes")? as f64 / e),
    );
    assert_eq!(first_reading.availability, Availability::WarmingUp);
    clock.set(1_500_000_000);
    let (second, second_window) = query(now, || {
        clock.set(clock.get() + 1_000_000_000);
        4000
    });
    let reading = counters.derive(
        "disk/read",
        Ok(api_raw(
            "sysinfo::Disk::usage",
            second_window,
            [("bytes", second)],
        )),
        |a, b, e| Ok(delta(a, b, "bytes")? as f64 / e),
    );
    assert_eq!(reading.value, Some(2000.0));
    assert_eq!(reading.observations[0].read_started_ns, Some(800_000_000));
    assert_eq!(reading.observations[1].read_started_ns, Some(1_500_000_000));
    assert_eq!(reading.observations[1].captured_ns, 2_500_000_000);
}
