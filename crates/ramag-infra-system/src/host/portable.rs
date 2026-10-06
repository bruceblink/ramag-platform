//! Common sysinfo path. Linux uses direct sources to retain permission and counter detail.
use super::*;
mod processes;
mod readings;
use readings::*;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind};

#[cfg(any(test, not(target_os = "linux")))]
impl HostCollector {
    pub(super) fn collect_portable(&mut self, s: &mut Snapshot) {
        let origin = self.origin;
        let fixed_ns = self.fixed_ns;
        let clock =
            || fixed_ns.unwrap_or_else(|| origin.elapsed().as_nanos().min(u64::MAX as u128) as u64);
        let process_identities = self.prepare_portable_process_refresh();
        let (_, cpu_window) = query(clock, || {
            self.system
                .refresh_cpu_specifics(CpuRefreshKind::everything())
        });
        let (_, memory_window) = query(clock, || {
            self.system
                .refresh_memory_specifics(MemoryRefreshKind::everything())
        });
        let process_window = self.collect_portable_processes(s, process_identities, clock);
        // sysinfo's process refresh may also refresh the global CPU cache. Bound both
        // calls before reading that shared cache, including a slow process refresh.
        let cpu_window = QueryWindow {
            started_ns: cpu_window.started_ns,
            captured_ns: process_window.captured_ns,
        };
        let (_, network_window) = query(clock, || self.networks.refresh(true));
        monitor(s, "cpu:host", "CPU", MonitorKind::Cpu);
        monitor(s, "memory:host", "Memory", MonitorKind::Memory);
        let mut usage = api_scalar(
            "cpu:host/usage",
            "sysinfo::System::global_cpu_usage",
            self.system.global_cpu_usage() as f64,
            cpu_window,
        );
        if self.sequence <= 1 {
            usage.value = None;
            usage.availability = Availability::WarmingUp;
            usage.reason = Some("sysinfo CPU counters need a second refresh".into());
        }
        sensor(
            s,
            ("cpu:host", "usage", "Overall utilization"),
            SensorKind::Percentage,
            Unit::Percent,
            "sysinfo::System::global_cpu_usage",
            "Host CPU usage normalized to all CPUs",
            usage,
        );
        for (index, cpu) in self.system.cpus().iter().enumerate() {
            for (suffix, title, kind, unit, v, source) in [
                (
                    format!("core-{index}-usage"),
                    format!("CPU {index} utilization"),
                    SensorKind::Percentage,
                    Unit::Percent,
                    cpu.cpu_usage() as f64,
                    "sysinfo::Cpu::cpu_usage",
                ),
                (
                    format!("core-{index}-frequency"),
                    format!("CPU {index} frequency"),
                    SensorKind::Frequency,
                    Unit::Hertz,
                    cpu.frequency() as f64 * 1e6,
                    "sysinfo::Cpu::frequency (MHz)",
                ),
            ] {
                let id = format!("cpu:host/{suffix}");
                let mut r = api_scalar(&id, source, v, cpu_window);
                if suffix.ends_with("frequency") {
                    r.observations[0]
                        .integers
                        .insert("MHz".into(), cpu.frequency());
                } else if self.sequence <= 1 {
                    r.value = None;
                    r.availability = Availability::WarmingUp;
                    r.reason = Some("sysinfo CPU counters need a second refresh".into());
                }
                sensor(
                    s,
                    ("cpu:host", &suffix, &title),
                    kind,
                    unit,
                    source,
                    "Logical CPU reported by sysinfo",
                    r,
                );
            }
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        let (components, component_window) =
            query(clock, sysinfo::Components::new_with_refreshed_list);
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        let (components, component_window) = query(clock, || self.temperatures.refresh());
        for component in components.iter() {
            let suffix = format!("temperature:{}", component.label());
            let id = format!("cpu:host/{suffix}");
            let source = "sysinfo::Component::temperature";
            let r = component
                .temperature()
                .map(|v| api_scalar(&id, source, v as f64, component_window))
                .unwrap_or_else(|| {
                    missing(
                        &id,
                        Availability::Unavailable,
                        format!("{source}: no reading"),
                    )
                });
            sensor(
                s,
                ("cpu:host", &suffix, component.label()),
                SensorKind::Temperature,
                Unit::Celsius,
                source,
                "Named hardware component; backend label retained",
                r,
            );
        }
        if components.is_empty() {
            unsupported(
                s,
                ("cpu:host", "temperature", "CPU temperature"),
                SensorKind::Temperature,
                Unit::Celsius,
                "sysinfo::Components",
                "No hardware component temperature exposed",
            );
        }
        let (load, load_window) = query(clock, sysinfo::System::load_average);
        for (suffix, v) in [
            ("load-1", load.one),
            ("load-5", load.five),
            ("load-15", load.fifteen),
        ] {
            if cfg!(target_os = "windows") {
                unsupported(
                    s,
                    ("cpu:host", suffix, "Load average"),
                    SensorKind::Scalar,
                    Unit::Load,
                    "sysinfo::System::load_average",
                    "Load average is not supported by the Windows sysinfo backend",
                );
            } else {
                let r = api_scalar(
                    &format!("cpu:host/{suffix}"),
                    "sysinfo::System::load_average",
                    v,
                    load_window,
                );
                sensor(
                    s,
                    ("cpu:host", suffix, "Load average"),
                    SensorKind::Scalar,
                    Unit::Load,
                    "sysinfo::System::load_average",
                    "Host scheduler load average",
                    r,
                );
            }
        }
        let (uptime, uptime_window) = query(clock, sysinfo::System::uptime);
        let r = api_integer(
            "cpu:host/uptime",
            "sysinfo::System::uptime",
            uptime,
            uptime_window,
        );
        sensor(
            s,
            ("cpu:host", "uptime", "Uptime"),
            SensorKind::Duration,
            Unit::Seconds,
            "sysinfo::System::uptime",
            "Host uptime",
            r,
        );
        for (suffix, title, value, total) in [
            (
                "used",
                "RAM used",
                self.system
                    .total_memory()
                    .checked_sub(self.system.available_memory()),
                Some(self.system.total_memory()),
            ),
            ("total", "RAM total", Some(self.system.total_memory()), None),
            (
                "available",
                "RAM available",
                Some(self.system.available_memory()),
                None,
            ),
            ("free", "RAM free", Some(self.system.free_memory()), None),
            (
                "swap",
                "Swap used",
                Some(self.system.used_swap()),
                Some(self.system.total_swap()),
            ),
            (
                "swap-total",
                "Swap total",
                Some(self.system.total_swap()),
                None,
            ),
        ] {
            let id = format!("memory:host/{suffix}");
            let source = "sysinfo::System memory bytes";
            let mut r = value
                .map(|v| api_integer(&id, source, v, memory_window))
                .unwrap_or_else(|| {
                    missing(
                        &id,
                        Availability::Failed,
                        "Available memory exceeds total".into(),
                    )
                });
            r.total = total.map(|v| v as f64);
            if suffix == "used" {
                r.observations = vec![api_raw(
                    source,
                    memory_window,
                    [
                        ("total", self.system.total_memory()),
                        ("available", self.system.available_memory()),
                    ],
                )];
            }
            sensor(
                s,
                ("memory:host", suffix, title),
                SensorKind::Capacity,
                Unit::Bytes,
                source,
                "RAM used = total - available; backend memory definitions",
                r,
            );
        }
        for suffix in [
            "cache",
            "buffers",
            "other",
            "page-faults",
            "major-page-faults",
        ] {
            unsupported(
                s,
                ("memory:host", suffix, suffix),
                if suffix.contains("faults") {
                    SensorKind::Counter
                } else {
                    SensorKind::Capacity
                },
                if suffix.contains("faults") {
                    Unit::Count
                } else {
                    Unit::Bytes
                },
                "sysinfo::System",
                "sysinfo does not expose this host memory field on this platform",
            );
        }
        #[cfg(target_os = "windows")]
        {
            let preference = super::windows_network::preferred_alias(&self.networks);
            if let Some(alias) = preference.alias
                && let Some(data) = self.networks.get(&alias)
            {
                let mac = data.mac_address().to_string();
                s.preferred_network_monitor_id = Some(network_identity(&alias, Some(&mac), None));
            }
            for error in preference.failures {
                diagnostic(s, "windows-network-route", error);
            }
        }
        for (name, data) in &self.networks {
            let mac = data.mac_address().to_string();
            let id = network_identity(name, Some(&mac), None);
            monitor(s, &id, name, MonitorKind::Network);
            for (suffix, total) in [
                ("rx", data.total_received()),
                ("tx", data.total_transmitted()),
            ] {
                let source = "sysinfo::NetworkData cumulative bytes";
                let sid = format!("{id}/{suffix}");
                let r = self.counters.derive(
                    &sid,
                    Ok(api_raw(source, network_window, [("bytes", total)])),
                    |a, b, e| Ok(delta(a, b, "bytes")? as f64 / e),
                );
                sensor(
                    s,
                    (&id, suffix, suffix),
                    SensorKind::Rate,
                    Unit::BytesPerSecond,
                    source,
                    "Interface bytes per measured second",
                    r,
                );
                let suffix = format!("{suffix}-total");
                let r = api_integer(&format!("{id}/{suffix}"), source, total, network_window);
                sensor(
                    s,
                    (&id, &suffix, &suffix),
                    SensorKind::Counter,
                    Unit::Bytes,
                    source,
                    "Interface cumulative bytes",
                    r,
                );
            }
            unsupported(
                s,
                (&id, "connections", "Established TCP connections"),
                SensorKind::Counter,
                Unit::Count,
                "sysinfo::Networks",
                "sysinfo exposes no address-attributed connection counts",
            );
        }
        #[cfg(not(target_os = "macos"))]
        let (disks, disk_window) = query(clock, sysinfo::Disks::new_with_refreshed_list);
        #[cfg(target_os = "macos")]
        let (disks, disk_window) = query(clock, || {
            sysinfo::Disks::new_with_refreshed_list_specifics(
                sysinfo::DiskRefreshKind::everything().without_storage(),
            )
        });
        for disk in &disks {
            let id = format!(
                "volume:source:{}:{}",
                disk.name().to_string_lossy(),
                disk.mount_point().display()
            );
            monitor(
                s,
                &id,
                &disk.mount_point().display().to_string(),
                MonitorKind::Volume,
            );
            #[cfg(not(target_os = "macos"))]
            let (source, scope, r) = {
                let source = "sysinfo::Disk capacity bytes";
                let mut r = api_integer(
                    &format!("{id}/capacity"),
                    source,
                    disk.total_space().saturating_sub(disk.available_space()),
                    disk_window,
                );
                r.total = Some(disk.total_space() as f64);
                r.observations = vec![api_raw(
                    source,
                    disk_window,
                    [
                        ("total", disk.total_space()),
                        ("available", disk.available_space()),
                    ],
                )];
                (source, "Filesystem total - available bytes", r)
            };
            #[cfg(target_os = "macos")]
            let (source, scope, r) = {
                let (blocks, window) = query(clock, || {
                    rustix::fs::statvfs(disk.mount_point())
                        .map(|v| (v.f_blocks, v.f_bfree, v.f_frsize))
                        .map_err(|error| {
                            format!("statvfs({}): {error}", disk.mount_point().display())
                        })
                });
                (
                    "statvfs filesystem capacity",
                    "(f_blocks - f_bfree) * f_frsize; purgeable files remain used",
                    filesystem_capacity_reading(&format!("{id}/capacity"), blocks, window),
                )
            };
            sensor(
                s,
                (&id, "capacity", "Filesystem used"),
                SensorKind::Capacity,
                Unit::Bytes,
                source,
                scope,
                r,
            );
            for (suffix, total) in [
                ("read", disk.usage().total_read_bytes),
                ("write", disk.usage().total_written_bytes),
            ] {
                let source = "sysinfo::Disk::usage cumulative bytes";
                let sid = format!("{id}/{suffix}");
                let r = self.counters.derive(
                    &sid,
                    Ok(api_raw(source, disk_window, [("bytes", total)])),
                    |a, b, e| Ok(delta(a, b, "bytes")? as f64 / e),
                );
                sensor(
                    s,
                    (&id, suffix, suffix),
                    SensorKind::Rate,
                    Unit::BytesPerSecond,
                    source,
                    "Shared backing device I/O; not per-volume attribution",
                    r,
                );
            }
            for suffix in ["iops", "latency"] {
                unsupported(
                    s,
                    (&id, suffix, suffix),
                    SensorKind::Scalar,
                    if suffix == "iops" {
                        Unit::CountPerSecond
                    } else {
                        Unit::Milliseconds
                    },
                    "sysinfo::Disk::usage",
                    "sysinfo does not expose request counts or request latency",
                );
            }
        }
        s.diagnostics.push(BackendDiagnostic{backend:"sysinfo".into(),availability:Availability::Available,reason:"sysinfo 0.38.4 common backend; some APIs expose no per-field error channel. Native macOS/Windows accuracy is not validated on Linux. Process actions require an exact native creation identity.".into()});
    }
}
#[cfg(any(test, target_os = "macos"))]
#[cfg(test)]
#[path = "../portable/tests.rs"]
mod tests;
