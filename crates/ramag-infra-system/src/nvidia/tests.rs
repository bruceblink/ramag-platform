use super::*;
#[test]
fn windows_pci_mapping_is_exact_and_never_falls_back_to_a_name_or_index() {
    let pci = (0, 4, 0, 0);
    assert_eq!(parse_pci_address("00000000:04:00.0"), Some(pci));
    for bad in [
        "",
        "04:00.0",
        "0000:100:00.0",
        "0000:04:20.0",
        "0000:04:00.8",
        "0000:04:00.0;run",
        "0000:04:00:00.0",
    ] {
        assert!(parse_pci_address(bad).is_none(), "{bad}");
    }
    let mut mapped = device("GPU-exact");
    mapped.pci_address = Some(pci);
    let mut other_domain = device("GPU-other-domain");
    other_domain.pci_address = Some((1, 4, 0, 0));
    let mut c = collector(vec![Ok(vec![mapped, other_domain, device("GPU-no-pci")])]);
    let mut s = Snapshot::default();
    c.collect_windows_at_origin(
        &mut s,
        std::time::Instant::now(),
        &BTreeMap::from([(pci, "windows-gpu:PNP-ID".into())]),
    );
    assert_eq!(s.monitors.len(), 1);
    assert_eq!(s.monitors[0].id, "windows-gpu:PNP-ID");
    assert!(
        s.sensors
            .iter()
            .all(|sensor| sensor.monitor_id == "windows-gpu:PNP-ID")
    );
    assert!(
        s.readings
            .iter()
            .any(|r| r.sensor_id == "windows-gpu:PNP-ID/usage" && r.value == Some(0.0))
    );
}
#[test]
fn ambiguous_nvml_pci_samples_do_not_overwrite_each_other() {
    let pci = (0, 4, 0, 0);
    let mut a = device("GPU-a");
    a.pci_address = Some(pci);
    let mut b = device("GPU-b");
    b.pci_address = Some(pci);
    let mut c = collector(vec![Ok(vec![a, b])]);
    let mut s = Snapshot::default();
    c.collect_windows_at_origin(
        &mut s,
        std::time::Instant::now(),
        &BTreeMap::from([(pci, "windows-gpu:PNP-ID".into())]),
    );
    assert!(s.monitors.is_empty());
    assert!(s.sensors.is_empty());
    assert!(
        s.diagnostics
            .iter()
            .any(|d| d.reason.contains("No unambiguous"))
    );
}
struct Fake {
    frames: std::collections::VecDeque<Result<Vec<DeviceSample>, String>>,
}
impl NvmlBackend for Fake {
    fn devices(&mut self, _clock: &CaptureClock) -> Result<Vec<DeviceSample>, String> {
        self.frames
            .pop_front()
            .ok_or_else(|| "NVML test backend exhausted its input frames".to_owned())?
    }
}
fn device(uuid: &str) -> DeviceSample {
    DeviceSample {
        pci_address: None,
        uuid: Ok(uuid.into()),
        name: Ok("GPU".into()),
        values: [
            ("usage", Ok(0)),
            ("temperature", Ok(40)),
            ("power", Ok(123456)),
            ("clock-graphics", Ok(1234)),
            ("clock-memory", Ok(5678)),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect(),
        memory: Ok((1000, 4000)),
        fans: vec![(Ok(110), Ok(1500))],
        windows: Windows::new(),
    }
}
fn collector(frames: Vec<Result<Vec<DeviceSample>, String>>) -> NvidiaCollector {
    NvidiaCollector {
        backend: Box::new(Fake {
            frames: frames.into(),
        }),
    }
}
fn r<'a>(s: &'a Snapshot, id: &str) -> Result<&'a Reading, String> {
    s.readings
        .iter()
        .find(|r| r.sensor_id == id)
        .ok_or_else(|| format!("missing reading {id}"))
}
#[test]
fn conversions_preserve_units_and_valid_zero() -> Result<(), String> {
    let mut c = collector(vec![Ok(vec![device("GPU-one"), device("GPU-two")])]);
    let mut s = Snapshot::default();
    c.collect(&mut s, 5);
    assert_eq!(s.monitors.len(), 2);
    for (suffix, value) in [
        ("usage", 0.0),
        ("vram", 1000.0),
        ("temperature", 40.0),
        ("power", 123.456),
        ("clock-graphics", 1_234_000_000.0),
        ("clock-memory", 5_678_000_000.0),
        ("fan-0-percent", 110.0),
        ("fan-0-rpm", 1500.0),
    ] {
        let r = r(&s, &format!("nvidia:GPU-one/{suffix}"))?;
        assert_eq!(r.value, Some(value));
        assert_eq!(r.observations[0].captured_ns, 5);
    }
    assert_eq!(
        s.sensors
            .iter()
            .find(|s| s.id == "nvidia:GPU-one/fan-0-rpm")
            .ok_or_else(|| "fan RPM descriptor must exist".to_owned())?
            .unit,
        Unit::Rpm
    );
    assert_eq!(
        s.sensors
            .iter()
            .find(|s| s.id == "nvidia:GPU-one/fan-0-percent")
            .ok_or_else(|| "fan percentage descriptor must exist".to_owned())?
            .unit,
        Unit::Percent
    );
    Ok(())
}
#[test]
fn reorder_removal_zero_devices_and_recovery() {
    let mut c = collector(vec![
        Ok(vec![device("GPU-a"), device("GPU-b")]),
        Ok(vec![device("GPU-b"), device("GPU-a")]),
        Ok(vec![]),
        Err("NVML: DriverNotLoaded".into()),
        Ok(vec![device("GPU-a")]),
    ]);
    for ids in [
        vec!["nvidia:GPU-a", "nvidia:GPU-b"],
        vec!["nvidia:GPU-b", "nvidia:GPU-a"],
        vec![],
        vec![],
        vec!["nvidia:GPU-a"],
    ] {
        let mut s = Snapshot::default();
        c.collect(&mut s, 0);
        assert_eq!(
            s.monitors.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ids
        );
        assert!(!s.diagnostics.is_empty());
    }
}
#[test]
fn uuid_failure_has_no_fabricated_index_identity() {
    let mut bad = device("ignored");
    bad.uuid = Err("NVML uuid: NoPermission".into());
    let mut c = collector(vec![Ok(vec![bad, device("GPU-real")])]);
    let mut s = Snapshot::default();
    c.collect(&mut s, 0);
    assert_eq!(s.monitors.len(), 1);
    assert_eq!(s.monitors[0].id, "nvidia:GPU-real");
    assert!(
        s.diagnostics
            .iter()
            .any(|d| d.reason.contains("NoPermission"))
    );
}
#[test]
fn per_field_loss_and_recovery_leave_other_fields_real() -> Result<(), String> {
    let mut bad = device("GPU-a");
    bad.values
        .insert("power".into(), Err("NVML power: GpuLost".into()));
    bad.memory = Err("NVML memory_info v2: FailedToLoadSymbol".into());
    bad.fans[0].1 = Err("NVML RPM: NotSupported".into());
    let mut c = collector(vec![Ok(vec![bad]), Ok(vec![device("GPU-a")])]);
    let mut s = Snapshot::default();
    c.collect(&mut s, 1);
    assert_eq!(r(&s, "nvidia:GPU-a/power")?.value, None);
    assert!(
        r(&s, "nvidia:GPU-a/power")?
            .reason
            .as_ref()
            .ok_or_else(|| "failed power reading must contain a reason".to_owned())?
            .contains("GpuLost")
    );
    assert_eq!(r(&s, "nvidia:GPU-a/temperature")?.value, Some(40.0));
    assert_eq!(r(&s, "nvidia:GPU-a/vram")?.value, None);
    let mut s = Snapshot::default();
    c.collect(&mut s, 2);
    assert_eq!(r(&s, "nvidia:GPU-a/power")?.value, Some(123.456));
    Ok(())
}
#[test]
fn initialization_failure_is_diagnostic_not_fake_gpu() {
    let mut c = collector(vec![Err("NVML init: LibraryNotFound".into())]);
    let mut s = Snapshot::default();
    c.collect(&mut s, 0);
    assert!(s.monitors.is_empty());
    assert!(
        s.diagnostics
            .iter()
            .any(|d| d.reason.contains("LibraryNotFound"))
    );
}

#[test]
fn memory_permission_failure_is_failed() -> Result<(), String> {
    let mut d = device("GPU-a");
    d.memory = Err("NVML memory: NoPermission".into());
    let mut c = collector(vec![Ok(vec![d])]);
    let mut s = Snapshot::default();
    c.collect(&mut s, 0);
    assert_eq!(
        r(&s, "nvidia:GPU-a/vram")?.availability,
        Availability::Failed
    );
    Ok(())
}
struct Session {
    count: Result<u32, String>,
    bad_index: Option<u32>,
}
impl NvmlSession for Session {
    fn count(&mut self) -> Result<u32, String> {
        self.count.clone()
    }
    fn sample(&mut self, index: u32, _clock: &CaptureClock) -> Result<DeviceSample, String> {
        if self.bad_index == Some(index) {
            Err("device_by_index: GpuLost".into())
        } else {
            Ok(device(&format!("GPU-{index}")))
        }
    }
}
#[test]
fn runtime_reuses_session_and_reinitializes_after_count_failure() -> Result<(), String> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut runtime = RuntimeNvml::injected(move || {
        let n = counter.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            Err("init: DriverNotLoaded".into())
        } else {
            Ok(Box::new(Session {
                count: if n == 1 {
                    Err("count: GpuLost".into())
                } else {
                    Ok(2)
                },
                bad_index: Some(0),
            }))
        }
    });
    assert!(runtime.devices(&CaptureClock::new(0)).is_err());
    assert!(runtime.devices(&CaptureClock::new(0)).is_err());
    let devices = runtime.devices(&CaptureClock::new(0))?;
    assert_eq!(devices.len(), 2);
    assert!(devices[0].uuid.is_err());
    assert_eq!(devices[1].uuid.as_deref(), Ok("GPU-1"));
    runtime.devices(&CaptureClock::new(0))?;
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    Ok(())
}

#[test]
fn runtime_reinitializes_after_memory_gpu_loss() -> Result<(), String> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct MemoryLost;
    impl NvmlSession for MemoryLost {
        fn count(&mut self) -> Result<u32, String> {
            Ok(1)
        }
        fn sample(&mut self, _index: u32, _clock: &CaptureClock) -> Result<DeviceSample, String> {
            let mut d = device("GPU-one");
            d.memory = Err("memory: GpuLost".into());
            Ok(d)
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let mut runtime = RuntimeNvml::injected(move || {
        c.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(MemoryLost))
    });
    runtime.devices(&CaptureClock::new(0))?;
    runtime.devices(&CaptureClock::new(0))?;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    Ok(())
}
#[test]
fn successful_runtime_session_is_reused() -> Result<(), String> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let mut runtime = RuntimeNvml::injected(move || {
        c.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Session {
            count: Ok(2),
            bad_index: None,
        }))
    });
    runtime.devices(&CaptureClock::new(0))?;
    runtime.devices(&CaptureClock::new(0))?;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    Ok(())
}
#[test]
fn query_window_is_after_initialization_delay_and_bounds_query() -> Result<(), String> {
    let clock = CaptureClock::new(42);
    std::thread::sleep(std::time::Duration::from_millis(2));
    let mut windows = Windows::new();
    let value = query(&clock, &mut windows, "power", "NVML power", || {
        std::thread::sleep(std::time::Duration::from_millis(2));
        Ok(123u64)
    })?;
    let r = timed(
        nvml_reading("id", "NVML power", Ok(value), 0.001, 42),
        &windows,
        "power",
    );
    let o = &r.observations[0];
    let started = o
        .read_started_ns
        .ok_or_else(|| "NVML query must record its start time".to_owned())?;
    assert!(started >= 2_000_042);
    assert!(o.captured_ns >= started + 2_000_000);
    Ok(())
}
