use super::*;
#[test]
fn cpu_all_cores_exclude_guest_and_iowait() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    let mut c = HostCollector::rooted(f.0.clone());
    let first = c.collect_at(1_000_000_000);
    assert_eq!(
        reading(&first, "cpu:host/usage")?.availability,
        Availability::WarmingUp
    );
    f.put("proc/stat","cpu 140 0 70 830 60 0 0 0 40 0\ncpu0 70 0 35 415 30 0 0 0 20 0\ncpu1 70 0 35 415 30 0 0 0 20 0\n");
    let s = c.collect_at(2_500_000_000);
    assert_eq!(reading(&s, "cpu:host/usage")?.value, Some(60.0));
    assert_eq!(reading(&s, "cpu:host/core-1-usage")?.value, Some(60.0));
    assert_eq!(reading(&s, "cpu:host/core-0-frequency")?.value, Some(3e9));
    assert_eq!(reading(&s, "cpu:host/load-15")?.value, Some(3.0));
    assert_eq!(reading(&s, "cpu:host/uptime")?.value, Some(123.5));
    assert_eq!(reading(&s, "cpu:host/usage")?.observations.len(), 2);
    f.verify()?;
    Ok(())
}
#[test]
fn memory_definition_and_raw_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    let s = HostCollector::rooted(f.0.clone()).collect_at(1);
    let used = reading(&s, "memory:host/used")?;
    assert_eq!(used.value, Some(600.0 * 1024.0));
    assert_eq!(used.total, Some(1000.0 * 1024.0));
    assert!(!used.observations.is_empty());
    assert_eq!(
        reading(&s, "memory:host/cache")?.value,
        Some(210.0 * 1024.0)
    );
    assert_eq!(
        reading(&s, "memory:host/other")?.value,
        Some(640.0 * 1024.0)
    );
    assert_eq!(reading(&s, "memory:host/swap")?.value, Some(300.0 * 1024.0));
    assert_eq!(reading(&s, "memory:host/page-faults")?.value, Some(900.0));
    f.verify()?;
    Ok(())
}
#[test]
fn over_500_processes_denial_and_pid_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    for pid in 1000..1601 {
        f.process(pid, 50, 100);
    }
    let mut c = HostCollector::rooted(f.0.clone());
    assert_eq!(c.collect_at(1_000_000_000).processes.len(), 601);
    f.process(1000, 50, 400);
    f.process(1001, 90, 10000);
    let s = c.collect_at(2_500_000_000);
    let row = s
        .processes
        .iter()
        .find(|r| r.identity.pid == 1000)
        .ok_or_else(|| std::io::Error::other("sampled process 1000 must exist"))?;
    assert_eq!(row.cpu_percent.value, Some(200.0));
    assert_eq!(row.memory_bytes.value, Some(40960.0));
    assert_eq!(row.user.as_deref(), Some("test"));
    assert_eq!(row.read_bytes_per_second.value, None);
    assert!(
        row.read_bytes_per_second
            .reason
            .as_ref()
            .ok_or_else(|| std::io::Error::other("process I/O failure should have a reason"))?
            .contains("io")
    );
    assert_eq!(
        s.processes
            .iter()
            .find(|r| r.identity.pid == 1001)
            .ok_or_else(|| std::io::Error::other("sampled process 1001 must exist"))?
            .cpu_percent
            .availability,
        Availability::WarmingUp
    );
    f.verify()?;
    Ok(())
}
#[test]
fn amd_identity_capabilities_conversions_and_reappearance() -> Result<(), Box<dyn std::error::Error>>
{
    let f = Fixture::new()?;
    f.base();
    let path = "sys/class/drm/card7/device";
    f.put(&format!("{path}/vendor"), "0x1002");
    f.put(&format!("{path}/unique_id"), "physical-uuid");
    f.put(&format!("{path}/gpu_busy_percent"), "37");
    f.put(&format!("{path}/mem_info_vram_used"), "1000");
    f.put(&format!("{path}/mem_info_vram_total"), "4000");
    for (name, value) in [
        ("temp1_input", "-1000"),
        ("temp1_label", "edge"),
        ("temp2_input", "52000"),
        ("temp2_label", "junction"),
        ("temp3_input", "43000"),
        ("temp3_label", "memory"),
        ("power1_average", "25000000"),
        ("freq1_input", "0"),
        ("freq2_input", "1500000000"),
        ("fan1_input", "1200"),
    ] {
        f.put(&format!("{path}/hwmon/hwmon99/{name}"), value);
    }
    let mut c = HostCollector::rooted(f.0.clone());
    let s = c.collect_at(1);
    assert!(s.monitors.iter().any(|m| m.id == "amdgpu:physical-uuid"));
    for (suffix, v) in [
        ("usage", 37.0),
        ("vram", 1000.0),
        ("temp1", -1.0),
        ("temp2", 52.0),
        ("temp3", 43.0),
        ("power-average", 25.0),
        ("clock-graphics", 0.0),
        ("clock-memory", 1.5e9),
        ("fan-rpm", 1200.0),
    ] {
        assert_eq!(
            reading(&s, &format!("amdgpu:physical-uuid/{suffix}"))?.value,
            Some(v)
        );
    }
    assert_eq!(
        reading(&s, "amdgpu:physical-uuid/power-instant")?.availability,
        Availability::Unavailable
    );
    f.rename(
        f.0.join("sys/class/drm/card7"),
        f.0.join("sys/class/drm/card2"),
    );
    assert!(
        c.collect_at(2)
            .monitors
            .iter()
            .any(|m| m.id == "amdgpu:physical-uuid")
    );
    f.remove_dir_all(f.0.join("sys/class/drm/card2"));
    assert!(
        !c.collect_at(3)
            .monitors
            .iter()
            .any(|m| m.id == "amdgpu:physical-uuid")
    );
    f.verify()?;
    Ok(())
}
#[test]
fn network_rates_totals_and_no_invented_connection_counts() -> Result<(), Box<dyn std::error::Error>>
{
    let f = Fixture::new()?;
    f.base();
    for (name, v) in [
        ("address", "aa:bb:cc:dd:ee:ff"),
        ("statistics/rx_bytes", "1000"),
        ("statistics/tx_bytes", "700"),
    ] {
        f.put(&format!("sys/class/net/eth9/{name}"), v);
    }
    let mut c = HostCollector::rooted(f.0.clone());
    c.collect_at(1_000_000_000);
    f.put("sys/class/net/eth9/statistics/rx_bytes", "4000");
    let s = c.collect_at(2_500_000_000);
    assert_eq!(
        reading(&s, "network:mac:aa:bb:cc:dd:ee:ff:name:eth9/rx")?.value,
        Some(2000.0)
    );
    assert_eq!(
        reading(&s, "network:mac:aa:bb:cc:dd:ee:ff:name:eth9/rx-total")?.value,
        Some(4000.0)
    );
    assert_eq!(
        reading(&s, "network:mac:aa:bb:cc:dd:ee:ff:name:eth9/connections")?.value,
        None
    );
    f.verify()?;
    Ok(())
}
#[test]
fn volume_device_rates_sectors_iops_and_latency() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    f.put(
        "proc/self/mountinfo",
        "24 1 8:1 / /mnt/test rw - ext4 /dev/sda1 rw\n",
    );
    f.put("sys/dev/block/8:1/stat", "10 0 100 40 20 0 200 60 0 0 0\n");
    let mut c = HostCollector::rooted(f.0.clone());
    c.collect_at(1_000_000_000);
    f.put("sys/dev/block/8:1/stat", "13 0 106 46 22 0 208 70 0 0 0\n");
    let s = c.collect_at(2_500_000_000);
    let m = s
        .monitors
        .iter()
        .find(|m| m.kind == MonitorKind::Volume)
        .ok_or_else(|| std::io::Error::other("volume monitor must exist"))?;
    assert_eq!(reading(&s, &format!("{}/read", m.id))?.value, Some(2048.0));
    assert_eq!(
        reading(&s, &format!("{}/write", m.id))?.value,
        Some(4096.0 / 1.5)
    );
    assert_eq!(
        reading(&s, &format!("{}/iops", m.id))?.value,
        Some(5.0 / 1.5)
    );
    assert_eq!(
        reading(&s, &format!("{}/latency", m.id))?.value,
        Some(16.0 / 5.0)
    );
    assert!(
        s.sensors
            .iter()
            .find(|d| d.id == format!("{}/read", m.id))
            .ok_or_else(|| std::io::Error::other("read rate sensor must exist"))?
            .scope
            .contains("Shared backing device")
    );
    f.verify()?;
    Ok(())
}

#[test]
fn network_duplicate_hardware_parent_keeps_identity_after_port_removal()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    f.put("sys/devices/pci/test/device", "x");
    for (name, mac) in [("eth0", "00:11:22:33:44:55"), ("eth1", "00:11:22:33:44:66")] {
        f.put(&format!("sys/class/net/{name}/address"), mac);
        f.put(&format!("sys/class/net/{name}/statistics/rx_bytes"), "100");
        f.put(&format!("sys/class/net/{name}/statistics/tx_bytes"), "100");
        f.symlink(
            f.0.join("sys/devices/pci/test"),
            f.0.join(format!("sys/class/net/{name}/device")),
        );
    }
    let mut c = HostCollector::rooted(f.0.clone());
    let s = c.collect_at(1);
    let before = s
        .monitors
        .iter()
        .find(|m| m.title == "Network interface (eth1)")
        .ok_or_else(|| std::io::Error::other("eth1 monitor must exist"))?
        .id
        .clone();
    f.remove_dir_all(f.0.join("sys/class/net/eth0"));
    let s = c.collect_at(2);
    assert_eq!(
        s.monitors
            .iter()
            .find(|m| m.title == "Network interface (eth1)")
            .ok_or_else(|| std::io::Error::other("eth1 monitor must remain after eth0 removal"))?
            .id,
        before
    );
    f.verify()?;
    Ok(())
}
#[test]
fn whole_cpu_failure_and_device_churn_reset_and_prune_baselines()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    let mut c = HostCollector::rooted(f.0.clone());
    c.collect_at(1);
    let initial = c.counters.len();
    assert!(initial >= 3);
    f.remove_file(f.0.join("proc/stat"));
    let s = c.collect_at(2);
    assert_eq!(
        reading(&s, "cpu:host/usage")?.availability,
        Availability::Failed
    );
    assert_eq!(c.counters.len(), 0);
    f.base();
    let s = c.collect_at(3);
    assert_eq!(
        reading(&s, "cpu:host/usage")?.availability,
        Availability::WarmingUp
    );
    assert_eq!(c.counters.len(), initial);
    f.verify()?;
    Ok(())
}
#[test]
fn cpu_temperature_retains_sparse_physical_labels() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    f.put("sys/class/hwmon/hwmon9/name", "coretemp");
    f.put("sys/class/hwmon/hwmon9/temp1_label", "Package id 0");
    f.put("sys/class/hwmon/hwmon9/temp1_input", "51000");
    f.put("sys/class/hwmon/hwmon9/temp6_label", "Core 4");
    f.put("sys/class/hwmon/hwmon9/temp6_input", "49000");
    let s = HostCollector::rooted(f.0.clone()).collect_at(1);
    let sensor = s
        .sensors
        .iter()
        .find(|d| d.title.contains("Core 4"))
        .ok_or_else(|| std::io::Error::other("Core 4 temperature sensor must exist"))?;
    assert_eq!(reading(&s, &sensor.id)?.value, Some(49.0));
    assert!(!sensor.id.contains("hwmon9"));
    f.verify()?;
    Ok(())
}

#[test]
fn derived_source_observations_have_read_windows() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    f.process(1000, 50, 100);
    f.put("proc/1000/io", "read_bytes: 100\nwrite_bytes: 200\n");
    f.put("sys/class/net/eth0/address", "aa:bb:cc:dd:ee:ff");
    f.put("sys/class/net/eth0/statistics/rx_bytes", "100");
    f.put("sys/class/net/eth0/statistics/tx_bytes", "100");
    f.put(
        "proc/self/mountinfo",
        "24 1 8:1 / /mnt/test rw - ext4 /dev/sda1 rw\n",
    );
    f.put("sys/dev/block/8:1/stat", "10 0 100 40 20 0 200 60 0 0 0\n");
    let mut c = HostCollector::rooted(f.0.clone());
    let s = c.collect_at(500);
    for id in [
        "cpu:host/usage",
        "network:mac:aa:bb:cc:dd:ee:ff:name:eth0/rx",
        "volume:source:/dev/sda1:/:/mnt/test/read",
    ] {
        let o = &reading(&s, id)?.observations[0];
        assert_eq!(o.read_started_ns, Some(500), "{id}");
        assert_eq!(o.captured_ns, 500);
    }
    for r in [
        &s.processes[0].cpu_percent,
        &s.processes[0].read_bytes_per_second,
    ] {
        assert_eq!(r.observations[0].read_started_ns, Some(500));
    }
    f.verify()?;
    Ok(())
}

#[test]
fn kernel_thread_count_survives_process_census_races() -> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    f.put("proc/999/stat", "process exited");
    let s = HostCollector::rooted(f.0.clone()).collect_at(1);
    let r = reading(&s, "cpu:host/threads")?;
    assert_eq!(r.value, Some(20.0));
    assert!(r.observations[0].source.contains("loadavg"));
    f.verify()?;
    Ok(())
}

#[test]
fn shared_mac_interfaces_keep_independent_rates_through_discovery_churn()
-> Result<(), Box<dyn std::error::Error>> {
    for hardware_parent in [false, true] {
        let f = Fixture::new()?;
        f.base();
        if hardware_parent {
            f.put(
                "sys/devices/pci/shared/uevent",
                "PCI_SLOT_NAME=0000:01:00.0",
            );
        }
        let put_interface = |name: &str, rx: u64| {
            f.put(
                &format!("sys/class/net/{name}/address"),
                "aa:bb:cc:dd:ee:ff",
            );
            f.put(
                &format!("sys/class/net/{name}/statistics/rx_bytes"),
                &rx.to_string(),
            );
            f.put(&format!("sys/class/net/{name}/statistics/tx_bytes"), "0");
            if hardware_parent {
                let link = f.0.join(format!("sys/class/net/{name}/device"));
                if !link.exists() {
                    f.symlink(f.0.join("sys/devices/pci/shared"), link);
                }
            }
        };
        // First creation order; both VLAN interfaces belong to the same hardware MAC.
        put_interface("eth0.10", 1000);
        put_interface("eth0.20", 9000);
        let mut c = HostCollector::rooted(f.0.clone());
        let first = c.collect_at(1_000_000_000);
        let id10 = first
            .monitors
            .iter()
            .find(|m| m.title == "Network interface (eth0.10)")
            .ok_or_else(|| std::io::Error::other("eth0.10 monitor must exist"))?
            .id
            .clone();
        let id20 = first
            .monitors
            .iter()
            .find(|m| m.title == "Network interface (eth0.20)")
            .ok_or_else(|| std::io::Error::other("eth0.20 monitor must exist"))?
            .id
            .clone();
        assert_ne!(
            id10, id20,
            "two VLAN interfaces sharing one MAC must remain distinct"
        );
        assert_eq!(
            reading(&first, &format!("{id10}/rx"))?.availability,
            Availability::WarmingUp
        );
        assert_eq!(
            reading(&first, &format!("{id20}/rx"))?.availability,
            Availability::WarmingUp
        );
        // Recreate in reverse directory enumeration order without an intervening sample.
        f.remove_dir_all(f.0.join("sys/class/net/eth0.10"));
        f.remove_dir_all(f.0.join("sys/class/net/eth0.20"));
        put_interface("eth0.20", 15000);
        put_interface("eth0.10", 4000);
        let reordered = c.collect_at(2_500_000_000);
        assert_eq!(
            reordered
                .monitors
                .iter()
                .find(|m| m.title == "Network interface (eth0.10)")
                .ok_or_else(|| std::io::Error::other("eth0.10 monitor must retain identity"))?
                .id,
            id10
        );
        assert_eq!(
            reordered
                .monitors
                .iter()
                .find(|m| m.title == "Network interface (eth0.20)")
                .ok_or_else(|| std::io::Error::other("eth0.20 monitor must retain identity"))?
                .id,
            id20
        );
        assert_eq!(
            reading(&reordered, &format!("{id10}/rx"))?.value,
            Some(2000.0)
        );
        assert_eq!(
            reading(&reordered, &format!("{id20}/rx"))?.value,
            Some(4000.0)
        );
        f.remove_dir_all(f.0.join("sys/class/net/eth0.10"));
        put_interface("eth0.20", 17000);
        let removed = c.collect_at(3_500_000_000);
        assert!(!removed.monitors.iter().any(|m| m.id == id10));
        assert_eq!(
            reading(&removed, &format!("{id20}/rx"))?.value,
            Some(2000.0)
        );
        put_interface("eth0.10", 30000);
        put_interface("eth0.20", 19000);
        let restored = c.collect_at(4_500_000_000);
        assert_eq!(
            restored
                .monitors
                .iter()
                .find(|m| m.title == "Network interface (eth0.10)")
                .ok_or_else(|| std::io::Error::other("restored eth0.10 monitor must exist"))?
                .id,
            id10
        );
        assert_eq!(
            reading(&restored, &format!("{id10}/rx"))?.availability,
            Availability::WarmingUp
        );
        assert_eq!(
            reading(&restored, &format!("{id20}/rx"))?.value,
            Some(2000.0)
        );
        f.verify()?;
    }
    Ok(())
}

#[test]
fn linux_snapshot_contains_shared_network_query_evidence() -> Result<(), Box<dyn std::error::Error>>
{
    let f = Fixture::new()?;
    f.base();
    f.put("sys/class/net/lo/address", "00:00:00:00:00:00");
    f.put(
        "proc/net/tcp",
        "sl local_address rem_address st\n0: 0100007F:1234 00000000:0000 0A\n",
    );
    f.put("proc/net/tcp6", "sl local_address rem_address st\n");
    let mut collector = HostCollector::rooted(f.0.clone());
    let snapshot = collector.collect_at(10);
    let evidence = snapshot
        .network_attribution
        .ok_or_else(|| std::io::Error::other("network query evidence must be captured"))?;
    assert_eq!(evidence.tcp_v4.rows.len(), 1);
    assert_eq!(evidence.tcp_v4.rows[0].state_hex.as_deref(), Some("0A"));
    assert_eq!(evidence.tcp_v6.query.source, "/proc/net/tcp6");
    f.remove_file(f.0.join("proc/net/tcp"));
    let failed = collector
        .collect_at(20)
        .network_attribution
        .ok_or_else(|| {
            std::io::Error::other("failed network query evidence must remain available")
        })?;
    assert_eq!(failed.tcp_v4.query.availability, Availability::Failed);
    assert_eq!(failed.tcp_v6.query.availability, Availability::Available);
    assert_eq!(failed.tcp_v6.query.captured_ns, 20);
    f.verify()?;
    Ok(())
}
