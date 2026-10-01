use super::*;
fn sample(ns: u64, busy: u64, total: u64, ticks: bool) -> RawObservation {
    raw_window(
        "test perf engine",
        ns.saturating_sub(10),
        ns,
        [
            ("active", busy),
            ("total", total),
            ("ticks", u64::from(ticks)),
            ("enabled_ns", ns),
            ("running_ns", ns),
            ("capacity", 1),
            ("config_active", 1),
            ("config_total", 2),
        ],
    )
}
#[test]
fn engine_activity_normalizes_busy_ns_or_active_over_total_ticks() -> Result<(), String> {
    assert_eq!(
        utilization(
            &sample(1_000_000_000, 100, 0, false),
            &sample(2_500_000_000, 750_000_100, 0, false),
            1.5
        )?,
        50.0
    );
    assert_eq!(
        utilization(
            &sample(1_000_000_000, 250, 1000, true),
            &sample(2_500_000_000, 1000, 2000, true),
            1.5
        )?,
        75.0
    );
    Ok(())
}
#[test]
fn rapl_fixed_point_joules_use_measured_elapsed_not_percent() -> Result<(), String> {
    let mut a = sample(1_000_000_000, 1u64 << 32, 0, false);
    let mut b = sample(2_500_000_000, 4u64 << 32, 0, false);
    a.integers.insert("energy_denominator".into(), 1u64 << 32);
    b.integers.insert("energy_denominator".into(), 1u64 << 32);
    assert_eq!(utilization(&a, &b, 1.5)?, 2.0);
    Ok(())
}
#[test]
fn first_reset_failed_baseline_irregular_time_and_impossible_activity() {
    let mut c = Counters::default();
    let key = "engine";
    assert_eq!(
        c.derive(key, Ok(sample(1_000_000_000, 100, 1000, true)), utilization)
            .availability,
        Availability::WarmingUp
    );
    let r = c.derive(key, Ok(sample(2_500_000_000, 850, 2000, true)), utilization);
    assert_eq!(r.value, Some(75.0));
    assert_eq!(r.observations.len(), 2);
    assert_eq!(
        c.derive(key, Ok(sample(3_000_000_000, 0, 0, true)), utilization)
            .value,
        None
    );
    assert_eq!(
        c.derive(key, Err("Permission denied".into()), utilization)
            .availability,
        Availability::Failed
    );
    assert_eq!(
        c.derive(key, Ok(sample(4_000_000_000, 100, 200, true)), utilization)
            .availability,
        Availability::WarmingUp
    );
    assert_eq!(
        c.derive(key, Ok(sample(4_000_000_000, 200, 400, true)), utilization)
            .value,
        None
    );
    assert!(utilization(&sample(1, 0, 0, true), &sample(2, 1000, 100, true), 1.0).is_err());
    assert!(
        utilization(
            &sample(1, 0, 0, false),
            &sample(2, 2_000_000_000, 0, false),
            1.0
        )
        .is_err()
    );
    let mut multiplexed = sample(2_000_000_000, 1000, 2000, true);
    multiplexed
        .integers
        .insert("running_ns".into(), 1_500_000_000);
    assert!(utilization(&sample(1_000_000_000, 100, 1000, true), &multiplexed, 1.0).is_err());
}
#[test]
fn pmu_specs_validate_units_formats_and_sparse_xe_engine_selectors()
-> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!("pulse-pmu-specs-{}", std::process::id()));
    std::fs::create_dir_all(path.join("events"))?;
    std::fs::create_dir_all(path.join("format"))?;
    let put = |p: &str, v: &str| std::fs::write(path.join(p), v);
    put("type", "17")?;
    put("events/rcs0-busy", "config=0x0")?;
    put("events/rcs0-busy.unit", "ns")?;
    let mut d = Device {
        pci: "0000:00:02.0".into(),
        driver: "i915".into(),
        path: path.clone(),
        cards: vec![],
    };
    let specs = specifications(&d, &path, &[], 0).map_err(std::io::Error::other)?;
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].active, 0);
    assert_eq!(specs[0].total, None);
    put("events/rcs0-busy.unit", "MHz")?;
    assert!(
        specifications(&d, &path, &[], 0).map_err(std::io::Error::other)?[0]
            .metadata_error
            .is_some()
    );
    d.driver = "xe".into();
    put("events/engine-active-ticks", "event=0x2")?;
    put("events/engine-total-ticks", "event=0x3")?;
    for (key, value) in [
        ("event", "config:0-11"),
        ("engine_class", "config:20-27"),
        ("engine_instance", "config:12-19"),
        ("gt", "config:60-63"),
    ] {
        put(&format!("format/{key}"), value)?;
    }
    let e = Engine {
        gt: 9,
        class: 4,
        instance: 7,
    };
    let specs =
        specifications(&d, &path, std::slice::from_ref(&e), 3).map_err(std::io::Error::other)?;
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].active, (9u64 << 60) | (4 << 20) | (7 << 12) | 2);
    assert_eq!(specs[0].cpu, 3);
    put("format/gt", "config:59-63")?;
    assert!(specifications(&d, &path, std::slice::from_ref(&e), 0).is_err());
    std::fs::remove_dir_all(path)?;
    Ok(())
}
#[test]
fn i915_bad_event_metadata_preserves_peer_sampling_and_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    struct Fake(u64);
    impl Counter for Fake {
        fn read(&mut self) -> io::Result<super::super::native::PerfRead> {
            self.0 += 1;
            Ok(super::super::native::PerfRead {
                active: self.0 * 500_000_000,
                total: 0,
                enabled: self.0 * 1_000_000_000,
                running: self.0 * 1_000_000_000,
            })
        }
    }
    let path = std::env::temp_dir().join(format!("pulse-pmu-peer-{}", std::process::id()));
    std::fs::create_dir_all(path.join("events"))?;
    let put = |p: &str, v: &str| std::fs::write(path.join(p), v);
    put("type", "17")?;
    put("events/rcs0-busy", "config=0x0")?;
    put("events/rcs0-busy.unit", "ns")?;
    put("events/bcs0-busy", "config=0x1000")?;
    put("events/bcs0-busy.unit", "MHz")?;
    let d = Device {
        pci: "0000:00:02.0".into(),
        driver: "i915".into(),
        path: path.clone(),
        cards: vec![],
    };
    let mut p = Pmu::new();
    p.factory = Box::new(|_| Ok(Box::new(Fake(0))));
    let run = |p: &mut Pmu, ns| {
        let mut s = Snapshot::default();
        p.sample_specs(
            &mut s,
            "intel-pci:test",
            &Clock {
                origin: std::time::Instant::now(),
                fixed: Some(ns),
            },
            specifications(&d, &path, &[], 0),
        );
        s
    };
    let first = run(&mut p, 1_000_000_000);
    assert!(
        first
            .readings
            .iter()
            .any(|r| r.sensor_id.ends_with("engine-class0-instance0")
                && r.availability == Availability::WarmingUp),
        "valid peer suppressed: {:?}",
        first.readings
    );
    assert!(
        first
            .readings
            .iter()
            .any(|r| r.availability == Availability::Failed)
    );
    let s = run(&mut p, 2_000_000_000);
    assert!(s.readings.iter().any(|r| r.value == Some(50.0)));
    put("events/bcs0-busy.unit", "ns")?;
    let s = run(&mut p, 3_000_000_000);
    assert!(
        s.readings
            .iter()
            .any(|r| r.sensor_id.ends_with("engine-class1-instance0")
                && r.availability == Availability::WarmingUp)
    );
    put("events/bcs0-busy", "malformed")?;
    let s = run(&mut p, 4_000_000_000);
    assert!(
        s.readings
            .iter()
            .any(|r| r.sensor_id.ends_with("engine-class0-instance0") && r.value == Some(50.0))
    );
    assert!(
        s.readings
            .iter()
            .any(|r| r.sensor_id.ends_with("engine-class1-instance0")
                && r.availability == Availability::Failed)
    );
    put("events/bcs0-busy", "config=0x1000")?;
    let s = run(&mut p, 5_000_000_000);
    assert!(
        s.readings
            .iter()
            .any(|r| r.sensor_id.ends_with("engine-class1-instance0")
                && r.availability == Availability::WarmingUp)
    );
    for (i, denied) in ["bcs0-busy", "bcs0-busy.unit"].iter().enumerate() {
        let read = |path: &Path| {
            if path.file_name().is_some_and(|name| name == *denied) {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            } else {
                sysfs::text(path)
            }
        };
        let specs = ["rcs0-busy", "bcs0-busy"]
            .iter()
            .map(|name| {
                i915_spec(
                    &path,
                    &path.join(format!("events/{name}")),
                    name,
                    17,
                    0,
                    &read,
                )
            })
            .collect();
        let mut s = Snapshot::default();
        p.sample_specs(
            &mut s,
            "intel-pci:test",
            &Clock {
                origin: std::time::Instant::now(),
                fixed: Some((6 + i as u64 * 2) * 1_000_000_000),
            },
            Ok(specs),
        );
        assert!(
            s.readings
                .iter()
                .any(|r| r.sensor_id.ends_with("engine-class0-instance0") && r.value == Some(50.0))
        );
        assert!(
            s.readings
                .iter()
                .any(|r| r.sensor_id.ends_with("engine-class1-instance0")
                    && r.availability == Availability::Unavailable
                    && r.value.is_none())
        );
        let recovered = run(&mut p, (7 + i as u64 * 2) * 1_000_000_000);
        assert!(
            recovered
                .readings
                .iter()
                .any(|r| r.sensor_id.ends_with("engine-class1-instance0")
                    && r.availability == Availability::WarmingUp)
        );
    }
    std::fs::remove_dir_all(path)?;
    Ok(())
}
#[test]
fn initial_malformed_pmu_discovery_is_failed() {
    let mut p = Pmu::new();
    let mut s = Snapshot::default();
    p.sample_specs(
        &mut s,
        "intel-pci:test",
        &Clock {
            origin: std::time::Instant::now(),
            fixed: Some(1),
        },
        Err(invalid("Malformed PMU type")),
    );
    assert_eq!(s.readings[0].availability, Availability::Failed);
}
fn spec(id: &str) -> Spec {
    Spec {
        id: id.into(),
        event_path: None,
        metadata_error: None,
        pmu: "/sys/test".into(),
        kind: 17,
        cpu: 0,
        active: 2,
        total: Some(3),
        energy_denominator: 0,
        scope: "GT5 engine capacity 1".into(),
    }
}
#[test]
fn pmu_retry_failed_read_independence_and_disappearance_release_state()
-> Result<(), Box<dyn std::error::Error>> {
    use std::sync::{Arc, Mutex};
    struct Fake(
        Arc<
            Mutex<
                std::collections::VecDeque<Result<super::super::native::PerfRead, io::ErrorKind>>,
            >,
        >,
    );
    impl Counter for Fake {
        fn read(&mut self) -> io::Result<super::super::native::PerfRead> {
            let result = self
                .0
                .lock()
                .map_err(|error| io::Error::other(error.to_string()))?
                .pop_front()
                .ok_or_else(|| io::Error::other("PMU test counter queue is exhausted"))?;
            result.map_err(io::Error::from)
        }
    }
    let queue = Arc::new(Mutex::new(std::collections::VecDeque::from([
        Ok(super::super::native::PerfRead {
            active: 100,
            total: 1000,
            enabled: 100,
            running: 100,
        }),
        Ok(super::super::native::PerfRead {
            active: 850,
            total: 2000,
            enabled: 200,
            running: 200,
        }),
        Err(io::ErrorKind::PermissionDenied),
        Ok(super::super::native::PerfRead {
            active: 10000,
            total: 20000,
            enabled: 1000,
            running: 1000,
        }),
    ])));
    let mut p = Pmu::new();
    let q = queue.clone();
    let mut opens = 0;
    p.factory = Box::new(move |s| {
        if s.id == "bad" {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        opens += 1;
        if opens == 1 {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        } else {
            Ok(Box::new(Fake(q.clone())))
        }
    });
    let run = |p: &mut Pmu, ns| {
        let mut s = Snapshot::default();
        p.sample_specs(
            &mut s,
            "intel-pci:test",
            &Clock {
                origin: std::time::Instant::now(),
                fixed: Some(ns),
            },
            Ok(vec![spec("good"), spec("bad")]),
        );
        s
    };
    let s = run(&mut p, 1_000_000_000);
    assert_eq!(s.readings.len(), 2);
    assert!(s.readings.iter().all(|r| r.value.is_none()));
    let s = run(&mut p, 2_000_000_000);
    assert_eq!(
        s.readings
            .iter()
            .find(|r| r.sensor_id.ends_with("/good"))
            .ok_or_else(|| std::io::Error::other("good PMU reading must exist"))?
            .availability,
        Availability::WarmingUp
    );
    let s = run(&mut p, 3_500_000_000);
    assert_eq!(
        s.readings
            .iter()
            .find(|r| r.sensor_id.ends_with("/good"))
            .ok_or_else(|| std::io::Error::other("good PMU reading must exist"))?
            .value,
        Some(75.0)
    );
    let s = run(&mut p, 4_000_000_000);
    assert_eq!(
        s.readings
            .iter()
            .find(|r| r.sensor_id.ends_with("/good"))
            .ok_or_else(|| std::io::Error::other("good PMU reading must exist"))?
            .availability,
        Availability::Unavailable
    );
    let s = run(&mut p, 5_000_000_000);
    assert_eq!(
        s.readings
            .iter()
            .find(|r| r.sensor_id.ends_with("/good"))
            .ok_or_else(|| std::io::Error::other("good PMU reading must exist"))?
            .availability,
        Availability::WarmingUp
    );
    p.retain(&[]);
    assert!(p.slots.is_empty());
    assert!(p.counters.is_empty());
    Ok(())
}
