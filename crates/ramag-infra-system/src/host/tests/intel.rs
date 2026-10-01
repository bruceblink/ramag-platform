use super::*;
use std::{error::Error, io};
#[test]
fn intel_sparse_gt_clocks_aliases_same_names_and_mixed_amd() -> Result<(), Box<dyn Error>> {
    let f = Fixture::new()?;
    f.base();
    let i915 = f.intel("0000:00:02.0", "i915", &["card7", "renderD128"]);
    let xe = f.intel("0000:03:00.0", "xe", &["card2", "renderD129"]);
    f.put("sys/class/drm/card7/gt/gt3/rps_act_freq_mhz", "500");
    f.put("sys/class/drm/card7/gt/gt3/rps_cur_freq_mhz", "700");
    f.put("sys/class/drm/card7/gt/gt8/rps_act_freq_mhz", "900");
    f.put(&format!("{xe}/tile2/gt5/freq0/act_freq"), "1000");
    f.put(&format!("{xe}/tile2/gt5/freq0/cur_freq"), "1200");
    f.put(&format!("{i915}/product_name"), "same name");
    f.put(&format!("{xe}/product_name"), "same name");
    f.put("sys/class/drm/card0/device/vendor", "0x1002");
    f.put("sys/class/drm/card0/device/unique_id", "amd-preserved");
    f.put("sys/class/drm/card0/device/gpu_busy_percent", "31");
    let mut c = HostCollector::rooted(f.0.clone());
    let s = c.collect_at(10);
    assert_eq!(
        s.monitors
            .iter()
            .filter(|m| m.kind == MonitorKind::Gpu)
            .count(),
        3
    );
    assert_eq!(reading(&s, "amdgpu:amd-preserved/usage")?.value, Some(31.0));
    for (id, value) in [
        ("intel-pci:0000:00:02.0/gt3-frequency-actual", 500e6),
        ("intel-pci:0000:00:02.0/gt3-frequency-requested", 700e6),
        ("intel-pci:0000:00:02.0/gt8-frequency-actual", 900e6),
        ("intel-pci:0000:03:00.0/tile2-gt5-frequency-actual", 1000e6),
        (
            "intel-pci:0000:03:00.0/tile2-gt5-frequency-requested",
            1200e6,
        ),
    ] {
        let r = reading(&s, id)?;
        assert_eq!(r.value, Some(value));
        assert_eq!(r.observations[0].integers["value"], (value / 1e6) as u64);
        assert_eq!(r.observations[0].read_started_ns, Some(10));
    }
    assert_eq!(
        reading(&s, "intel-pci:0000:00:02.0/gt8-frequency-requested")?.availability,
        Availability::Unavailable
    );
    f.rename(
        f.0.join("sys/class/drm/card7"),
        f.0.join("sys/class/drm/card9"),
    );
    assert_eq!(
        reading(
            &c.collect_at(20),
            "intel-pci:0000:00:02.0/gt3-frequency-actual"
        )?
        .value,
        Some(500e6)
    );
    f.verify()?;
    Ok(())
}

#[test]
fn intel_hwmon_scope_units_failure_independence_and_energy_recovery() -> Result<(), Box<dyn Error>>
{
    let f = Fixture::new()?;
    f.base();
    let physical = f.intel("0000:00:02.0", "xe", &["card7"]);
    let hw = format!("{physical}/hwmon/hwmon91");
    for (name, value) in [
        ("name", "xe"),
        ("temp2_input", "51000"),
        ("temp3_input", "-1000"),
        ("temp3_label", "VRAM"),
        ("fan2_input", "1200"),
        ("energy1_input", "1000000"),
        ("energy2_input", "2000000"),
        ("power1_average", "2500000"),
    ] {
        f.put(&format!("{hw}/{name}"), value);
    }
    let mut c = HostCollector::rooted(f.0.clone());
    let s = c.collect_at(1_000_000_000);
    let prefix = "intel-pci:0000:00:02.0/hwmon-xe";
    assert_eq!(
        reading(&s, &format!("{prefix}-temp2_input"))?.value,
        Some(51.0)
    );
    assert!(
        s.sensors
            .iter()
            .find(|d| d.id == format!("{prefix}-temp2_input"))
            .ok_or_else(|| io::Error::other("temperature sensor metadata missing"))?
            .scope
            .contains("Package")
    );
    assert_eq!(
        reading(&s, &format!("{prefix}-temp3_input"))?.value,
        Some(-1.0)
    );
    assert_eq!(
        reading(&s, &format!("{prefix}-fan2_input"))?.value,
        Some(1200.0)
    );
    assert_eq!(
        reading(&s, &format!("{prefix}-power1_average"))?.value,
        Some(2.5)
    );
    assert_eq!(
        reading(&s, &format!("{prefix}-energy1_input-power"))?.availability,
        Availability::WarmingUp
    );
    f.put(&format!("{hw}/energy1_input"), "4000000");
    f.put(&format!("{hw}/temp2_input"), "denied");
    let s = c.collect_at(2_500_000_000);
    assert_eq!(
        reading(&s, &format!("{prefix}-energy1_input-power"))?.value,
        Some(2.0)
    );
    assert_eq!(
        reading(&s, &format!("{prefix}-temp2_input"))?.availability,
        Availability::Failed
    );
    assert_eq!(
        reading(&s, &format!("{prefix}-fan2_input"))?.value,
        Some(1200.0)
    );
    f.put(&format!("{hw}/energy1_input"), "bad");
    c.collect_at(3_000_000_000);
    f.put(&format!("{hw}/energy1_input"), "9000000");
    assert_eq!(
        reading(
            &c.collect_at(4_000_000_000),
            &format!("{prefix}-energy1_input-power")
        )?
        .availability,
        Availability::WarmingUp
    );
    f.put(&format!("{hw}/temp2_input"), "52000");
    f.rename(f.0.join(&hw), f.0.join(format!("{physical}/hwmon/hwmon4")));
    assert_eq!(
        reading(
            &c.collect_at(5_000_000_000),
            &format!("{prefix}-temp2_input")
        )?
        .value,
        Some(52.0)
    );
    f.verify()?;
    Ok(())
}

#[test]
fn intel_hwmon_invalid_identity_never_reuses_old_scope_or_baseline() -> Result<(), Box<dyn Error>> {
    for invalidation in [
        "replacement",
        "missing",
        "malformed",
        "duplicate",
        "unknown-peer",
    ] {
        let f = Fixture::new()?;
        f.base();
        let physical = f.intel("0000:00:02.0", "i915", &["card7"]);
        let hw = format!("{physical}/hwmon/hwmon91");
        f.put(&format!("{hw}/name"), "i915");
        f.put(&format!("{hw}/energy1_input"), "1000000");
        f.put(&format!("{hw}/temp1_input"), "50000");
        let sid = "intel-pci:0000:00:02.0/hwmon-i915-energy1_input-power";
        let mut c = HostCollector::rooted(f.0.clone());
        c.collect_at(1_000_000_000);
        match invalidation {
            "replacement" => f.put(&format!("{hw}/name"), "i915_gt0"),
            "missing" => f.remove_file(f.0.join(format!("{hw}/name"))),
            "malformed" => f.record(fs::write(f.0.join(format!("{hw}/name")), [0xff])),
            "duplicate" => f.put(&format!("{physical}/hwmon/hwmon92/name"), "i915"),
            "unknown-peer" => {
                f.put(&format!("{physical}/hwmon/hwmon92/name"), "i915");
                f.record(fs::write(
                    f.0.join(format!("{physical}/hwmon/hwmon92/name")),
                    [0xff],
                ));
            }
            _ => return Err(io::Error::other("unknown hwmon invalidation case").into()),
        }
        f.put(&format!("{hw}/energy1_input"), "4000000");
        let s = c.collect_at(2_500_000_000);
        let r = reading(&s, sid)?;
        assert_eq!(r.value, None, "{invalidation}: old scope must not publish");
        assert_eq!(
            r.availability,
            if matches!(invalidation, "malformed" | "unknown-peer") {
                Availability::Failed
            } else {
                Availability::Unavailable
            },
            "{invalidation}"
        );
        assert_eq!(
            reading(&s, "intel-pci:0000:00:02.0/hwmon-i915-temp1_input")?.value,
            None
        );
        if invalidation == "replacement" {
            assert_eq!(
                reading(
                    &s,
                    "intel-pci:0000:00:02.0/hwmon-i915_gt0-energy1_input-power"
                )?
                .availability,
                Availability::WarmingUp
            );
        }
        f.put(&format!("{hw}/name"), "i915");
        if matches!(invalidation, "duplicate" | "unknown-peer") {
            f.remove_dir_all(f.0.join(format!("{physical}/hwmon/hwmon92")));
        }
        f.put(&format!("{hw}/energy1_input"), "9000000");
        assert_eq!(
            reading(&c.collect_at(4_000_000_000), sid)?.availability,
            Availability::WarmingUp,
            "{invalidation}: recovery must reset baseline"
        );
        f.verify()?;
    }
    Ok(())
}

#[test]
fn intel_legacy_clocks_reject_overflow_and_keep_requested_semantics() -> Result<(), Box<dyn Error>>
{
    let f = Fixture::new()?;
    f.base();
    f.intel("0000:00:02.0", "i915", &["card7"]);
    f.put(
        "sys/class/drm/card7/gt_act_freq_mhz",
        "18446744073709551615",
    );
    f.put("sys/class/drm/card7/gt_cur_freq_mhz", "1100");
    let s = HostCollector::rooted(f.0.clone()).collect_at(1);
    assert_eq!(
        reading(&s, "intel-pci:0000:00:02.0/gt0-frequency-actual")?.availability,
        Availability::Failed
    );
    assert_eq!(
        reading(&s, "intel-pci:0000:00:02.0/gt0-frequency-requested")?.value,
        Some(1100e6)
    );
    assert!(
        s.sensors
            .iter()
            .find(|d| d.id.ends_with("gt0-frequency-requested"))
            .ok_or_else(|| io::Error::other("requested clock metadata missing"))?
            .title
            .contains("requested")
    );
    f.verify()?;
    Ok(())
}

#[test]
fn intel_temporary_discovery_failure_does_not_reassign_alias_or_drop_physical_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    let physical = f.intel("0000:00:02.0", "i915", &["card7"]);
    f.put("sys/class/drm/card7/gt_act_freq_mhz", "500");
    let mut c = HostCollector::rooted(f.0.clone());
    c.collect_at(1);
    f.remove_file(f.0.join(format!("{physical}/vendor")));
    let s = c.collect_at(2);
    assert!(s.monitors.iter().any(|m| m.id == "intel-pci:0000:00:02.0"));
    // The old card ordinal can now name a different device. Never read its clocks into the saved Intel identity.
    f.remove_file(f.0.join("sys/class/drm/card7/device"));
    f.put("sys/devices/pci0000:00/0000:09:00.0/vendor", "0x1002");
    f.symlink(
        f.0.join("sys/devices/pci0000:00/0000:09:00.0"),
        f.0.join("sys/class/drm/card7/device"),
    );
    f.put("sys/class/drm/card7/gt_act_freq_mhz", "1500");
    let s = c.collect_at(3);
    assert_eq!(
        reading(&s, "intel-pci:0000:00:02.0/gt0-frequency-actual")?.value,
        None
    );
    f.remove_dir_all(f.0.join(physical));
    assert!(
        !c.collect_at(4)
            .monitors
            .iter()
            .any(|m| m.id == "intel-pci:0000:00:02.0")
    );
    f.verify()?;
    Ok(())
}
#[test]
fn intel_integrated_powercap_pp1_is_separate_and_rejects_package_substitution()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    f.intel("0000:00:02.0", "i915", &["card7"]);
    f.put("sys/bus/event_source/devices/i915/type", "17");
    f.put("sys/class/powercap/intel-rapl:0/name", "package-0");
    f.put("sys/class/powercap/intel-rapl:0/energy_uj", "999999999");
    f.put(
        "sys/class/powercap/intel-rapl:0/intel-rapl:0:3/name",
        "uncore",
    );
    f.put(
        "sys/class/powercap/intel-rapl:0/intel-rapl:0:3/energy_uj",
        "1000000",
    );
    let mut c = HostCollector::rooted(f.0.clone());
    c.collect_at(1_000_000_000);
    f.put(
        "sys/class/powercap/intel-rapl:0/intel-rapl:0:3/energy_uj",
        "4000000",
    );
    let s = c.collect_at(2_500_000_000);
    assert_eq!(
        reading(&s, "intel-pci:0000:00:02.0/rapl-package-0-uncore-power")?.value,
        Some(2.0)
    );
    assert!(
        s.sensors
            .iter()
            .find(|d| d.id.ends_with("rapl-package-0-uncore-power"))
            .ok_or_else(|| std::io::Error::other("PP1 sensor metadata missing"))?
            .scope
            .contains("PP1")
    );
    f.put("sys/class/powercap/intel-rapl:1/name", "package-1");
    let s = c.collect_at(3_000_000_000);
    assert!(
        !s.readings
            .iter()
            .any(|r| r.sensor_id.ends_with("rapl-package-0-uncore-power") && r.value.is_some())
    );
    f.verify()?;
    Ok(())
}
#[test]
fn intel_incomplete_powercap_identity_cannot_create_unique_attribution()
-> Result<(), Box<dyn std::error::Error>> {
    for level in ["package", "domain"] {
        let f = Fixture::new()?;
        f.base();
        f.intel("0000:00:02.0", "i915", &["card7"]);
        f.put("sys/bus/event_source/devices/i915/type", "17");
        let package = "sys/class/powercap/intel-rapl:0";
        let domain = format!("{package}/intel-rapl:0:3");
        f.put(&format!("{package}/name"), "package-0");
        f.put(&format!("{package}/energy_uj"), "99999999");
        f.put(&format!("{domain}/name"), "uncore");
        f.put(&format!("{domain}/energy_uj"), "1000000");
        let sid = "intel-pci:0000:00:02.0/rapl-package-0-uncore-power";
        let mut c = HostCollector::rooted(f.0.clone());
        assert_eq!(
            reading(&c.collect_at(1_000_000_000), sid)?.availability,
            Availability::WarmingUp
        );
        let unknown = if level == "package" {
            "sys/class/powercap/intel-rapl:1".to_string()
        } else {
            format!("{package}/intel-rapl:0:4")
        };
        f.put(&format!("{unknown}/name"), "unknown");
        f.record(fs::write(f.0.join(format!("{unknown}/name")), [0xff]));
        f.put(&format!("{domain}/energy_uj"), "4000000");
        let s = c.collect_at(2_500_000_000);
        assert!(
            !s.readings
                .iter()
                .any(|r| r.sensor_id == sid && r.value.is_some()),
            "{level}: unreadable zone must invalidate uniqueness"
        );
        assert!(
            s.diagnostics
                .iter()
                .any(|d| d.availability == Availability::Failed
                    && d.reason.contains(&format!("{unknown}/name")))
        );
        f.remove_dir_all(f.0.join(unknown));
        f.put(&format!("{domain}/energy_uj"), "9000000");
        assert_eq!(
            reading(&c.collect_at(4_000_000_000), sid)?.availability,
            Availability::WarmingUp
        );
        f.verify()?;
    }
    Ok(())
}
#[test]
fn intel_optional_sources_report_absence_and_malformed_discovery_explicitly()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    let physical = f.intel("0000:00:02.0", "xe", &["card7"]);
    let mut c = HostCollector::rooted(f.0.clone());
    let s = c.collect_at(1);
    for suffix in ["temperature", "power", "fan", "frequency-actual"] {
        assert_eq!(
            reading(&s, &format!("intel-pci:0000:00:02.0/{suffix}"))?.availability,
            Availability::Unavailable
        );
    }
    f.put(&format!("{physical}/hwmon"), "not a directory");
    let s = c.collect_at(2);
    assert!(s.diagnostics.iter().any(|d| d.backend == "intel sysfs"
        && d.availability == Availability::Failed
        && d.reason.contains("hwmon")));
    assert!(s.monitors.iter().any(|m| m.id == "intel-pci:0000:00:02.0"));
    f.verify()?;
    Ok(())
}
#[test]
fn intel_collection_preserves_existing_nvidia_and_amd_snapshot_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    f.intel("0000:00:02.0", "i915", &["card7", "renderD137"]);
    let mut s = Snapshot::default();
    for id in ["nvidia:GPU-physical", "amdgpu:physical"] {
        monitor(&mut s, id, "Existing GPU", MonitorKind::Gpu);
        sensor(
            &mut s,
            (id, "usage", "Existing usage"),
            SensorKind::Percentage,
            Unit::Percent,
            "existing vendor",
            "physical device",
            measured(&format!("{id}/usage"), 42.0, None),
        );
    }
    let mut c = HostCollector::rooted(f.0.clone());
    c.intel.collect(&mut s, &f.0, c.origin, Some(10));
    assert_eq!(s.monitors.len(), 3);
    assert_eq!(reading(&s, "nvidia:GPU-physical/usage")?.value, Some(42.0));
    assert_eq!(reading(&s, "amdgpu:physical/usage")?.value, Some(42.0));
    assert_eq!(
        s.sensors
            .iter()
            .find(|d| d.id == "nvidia:GPU-physical/usage")
            .ok_or_else(|| std::io::Error::other("NVIDIA sensor metadata missing"))?
            .source,
        "existing vendor"
    );
    f.verify()?;
    Ok(())
}
#[test]
fn intel_proven_vendor_replacement_is_not_retained_as_old_gpu()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    let physical = f.intel("0000:00:02.0", "i915", &["card7"]);
    let mut c = HostCollector::rooted(f.0.clone());
    assert!(
        c.collect_at(1)
            .monitors
            .iter()
            .any(|m| m.id == "intel-pci:0000:00:02.0")
    );
    f.put(&format!("{physical}/vendor"), "0x1002");
    assert!(
        !c.collect_at(2)
            .monitors
            .iter()
            .any(|m| m.id == "intel-pci:0000:00:02.0")
    );
    f.verify()?;
    Ok(())
}
#[test]
fn intel_incomplete_inventory_cannot_assign_unqualified_pmu_to_another_gpu()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    f.intel("0000:03:00.0", "i915", &["card7"]);
    let hidden = f.intel("0000:00:02.0", "i915", &["card8"]);
    f.remove_file(f.0.join(format!("{hidden}/vendor")));
    f.put("sys/bus/event_source/devices/i915/type", "17");
    let s = HostCollector::rooted(f.0.clone()).collect_at(1);
    assert!(
        !s.readings
            .iter()
            .any(|r| r.sensor_id.starts_with("intel-pci:0000:03:00.0/rapl-"))
    );
    assert!(
        !s.diagnostics
            .iter()
            .any(|d| d.reason.starts_with("Integrated:"))
    );
    f.verify()?;
    Ok(())
}
