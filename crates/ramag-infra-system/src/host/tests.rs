use super::*;
use crate::*;
use std::{
    cell::RefCell,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf, RefCell<Option<String>>);
impl Fixture {
    fn new() -> std::io::Result<Self> {
        let p = std::env::temp_dir().join(format!(
            "pulse-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&p)?;
        Ok(Self(p, RefCell::new(None)))
    }
    fn put(&self, path: &str, value: &str) {
        let p = self.0.join(path);
        let result = p
            .parent()
            .ok_or_else(|| std::io::Error::other("fixture path has no parent"))
            .and_then(fs::create_dir_all)
            .and_then(|()| fs::write(p, value));
        self.record(result);
    }
    fn record(&self, result: std::io::Result<impl Sized>) {
        if let Err(error) = result {
            self.1.borrow_mut().get_or_insert_with(|| error.to_string());
        }
    }
    fn verify(&self) -> std::io::Result<()> {
        match self.1.borrow().as_ref() {
            Some(error) => Err(std::io::Error::other(error.clone())),
            None => Ok(()),
        }
    }
    fn symlink(&self, source: PathBuf, target: PathBuf) {
        #[cfg(unix)]
        self.record(std::os::unix::fs::symlink(source, target));
        #[cfg(windows)]
        self.record(std::os::windows::fs::symlink_dir(source, target));
    }
    fn rename(&self, source: PathBuf, target: PathBuf) {
        self.record(fs::rename(source, target));
    }
    fn remove_file(&self, path: PathBuf) {
        self.record(fs::remove_file(path));
    }
    fn remove_dir_all(&self, path: PathBuf) {
        self.record(fs::remove_dir_all(path));
    }
    fn create_dir_all(&self, path: PathBuf) {
        self.record(fs::create_dir_all(path));
    }
    fn base(&self) {
        self.put("proc/stat","cpu 100 0 50 800 50 0 0 0 20 0\ncpu0 50 0 25 400 25 0 0 0 10 0\ncpu1 50 0 25 400 25 0 0 0 10 0\n");
        self.put("proc/meminfo","MemTotal: 1000 kB\nMemAvailable: 400 kB\nMemFree: 100 kB\nCached: 200 kB\nBuffers: 50 kB\nSReclaimable: 20 kB\nShmem: 10 kB\nSwapTotal: 500 kB\nSwapFree: 200 kB\n");
        self.put("proc/uptime", "123.5 100\n");
        self.put("proc/loadavg", "1.0 2.0 3.0 1/20 500\n");
        self.put("proc/vmstat", "pgfault 900\npgmajfault 20\n");
        self.put(
            "proc/cpuinfo",
            "processor : 0\ncpu MHz : 3000.0\n\nprocessor : 1\ncpu MHz : 2900.0\n",
        );
        self.put("proc/self/mountinfo", "");
        self.put("etc/passwd", "test:x:1000:1000::/home/test:/bin/sh\n");
    }
    fn process(&self, pid: u32, start: u64, ticks: u64) {
        let mut fields = vec!["0".to_string(); 22];
        fields[0] = "S".into();
        fields[11] = ticks.to_string();
        fields[12] = "0".into();
        fields[17] = "3".into();
        fields[19] = start.to_string();
        fields[21] = "10".into();
        self.put(
            &format!("proc/{pid}/stat"),
            &format!("{pid} (name with ) brackets) {}", fields.join(" ")),
        );
        self.put(
            &format!("proc/{pid}/status"),
            "Name:\tname\nUid:\t1000 1000 1000 1000\nThreads:\t3\nVmRSS:\t40 kB\n",
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn reading<'a>(s: &'a Snapshot, id: &str) -> std::io::Result<&'a Reading> {
    s.readings
        .iter()
        .find(|r| r.sensor_id == id)
        .ok_or_else(|| std::io::Error::other(format!("missing reading {id}")))
}
#[test]
fn network_preference_tracks_route_and_falls_back_to_physical_interface()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    for name in ["docker0", "enp10s0", "wlan0"] {
        f.put(&format!("sys/class/net/{name}/statistics/rx_bytes"), "10");
        f.put(&format!("sys/class/net/{name}/statistics/tx_bytes"), "20");
    }
    f.put("sys/devices/ethernet/marker", "");
    f.symlink(
        f.0.join("sys/devices/ethernet"),
        f.0.join("sys/class/net/enp10s0/device"),
    );
    f.put("sys/class/net/enp10s0/operstate", "up");
    f.put(
        "proc/net/route",
        "enp10s0 00000000 0101A8C0 0003 0 0 100 00000000\n",
    );
    let mut c = HostCollector::rooted(f.0.clone());
    let first = c.collect_at(1);
    let ethernet = first
        .monitors
        .iter()
        .find(|m| m.id.contains("enp10s0"))
        .ok_or_else(|| std::io::Error::other("physical Ethernet monitor must exist"))?
        .id
        .clone();
    assert_eq!(
        first.preferred_network_monitor_id.as_deref(),
        Some(ethernet.as_str())
    );
    f.put(
        "proc/net/route",
        "wlan0 00000000 0101A8C0 0003 0 0 100 00000000\n",
    );
    assert_eq!(
        c.collect_at(2).preferred_network_monitor_id.as_deref(),
        Some("network:name:wlan0")
    );
    f.put("proc/net/route", "");
    assert_eq!(
        c.collect_at(3).preferred_network_monitor_id.as_deref(),
        Some(ethernet.as_str())
    );
    f.verify()?;
    Ok(())
}

#[test]
fn monitor_names_use_host_descriptions_without_changing_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    let physical = "sys/devices/pci0000:00/0000:03:00.0";
    f.put(&format!("{physical}/vendor"), "0x1002");
    f.put(&format!("{physical}/unique_id"), "physical-uuid");
    f.create_dir_all(f.0.join("sys/class/drm/card7"));
    f.symlink(f.0.join(physical), f.0.join("sys/class/drm/card7/device"));
    f.put(
        "run/udev/data/+pci:0000:03:00.0",
        "E:ID_MODEL_FROM_DATABASE=Navi 48 [Radeon AI PRO R9700]\n",
    );
    for (name, alias) in [
        ("veth2986d25", ""),
        ("tap-123abc", ""),
        ("docker0", ""),
        ("eth0", "Office LAN"),
    ] {
        f.put(&format!("sys/class/net/{name}/ifalias"), alias);
        f.put(&format!("sys/class/net/{name}/type"), "1");
    }
    f.put("sys/class/net/tap-123abc/tun_flags", "0x1002");
    f.create_dir_all(f.0.join("sys/class/net/docker0/bridge"));
    let mut c = HostCollector::rooted(f.0.clone());
    let first = c.collect_at(1);
    for (id, expected) in [
        ("amdgpu:physical-uuid", "Radeon AI PRO R9700 (0000:03:00.0)"),
        ("network:name:veth2986d25", "Virtual Ethernet (veth2986d25)"),
        ("network:name:tap-123abc", "Virtual TAP (tap-123abc)"),
        ("network:name:docker0", "Docker bridge (docker0)"),
        ("network:name:eth0", "Office LAN (eth0)"),
    ] {
        assert_eq!(
            first
                .monitors
                .iter()
                .find(|m| m.id == id)
                .ok_or_else(|| std::io::Error::other(format!("missing monitor {id}")))?
                .title,
            expected
        );
    }
    f.put("sys/class/net/eth0/ifalias", "Renamed LAN");
    let second = c.collect_at(2);
    let ethernet = second
        .monitors
        .iter()
        .find(|m| m.id == "network:name:eth0")
        .ok_or_else(|| std::io::Error::other("renamed Ethernet monitor must exist"))?;
    assert_eq!(ethernet.title, "Renamed LAN (eth0)");
    f.remove_file(f.0.join("run/udev/data/+pci:0000:03:00.0"));
    let third = c.collect_at(3);
    assert_eq!(
        third
            .monitors
            .iter()
            .find(|m| m.id == "amdgpu:physical-uuid")
            .ok_or_else(|| std::io::Error::other("physical AMD monitor must remain discoverable"))?
            .title,
        "AMD GPU (0000:03:00.0)"
    );
    f.verify()?;
    Ok(())
}

#[test]
fn intel_pci_device_is_discovered_without_card_index_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new()?;
    f.base();
    let physical = "sys/devices/pci0000:00/0000:00:02.0";
    f.put(&format!("{physical}/vendor"), "0x8086");
    f.put(&format!("{physical}/device"), "0x46a6");
    f.put(
        &format!("{physical}/uevent"),
        "DRIVER=i915\nPCI_SLOT_NAME=0000:00:02.0\n",
    );
    f.create_dir_all(f.0.join("sys/bus/pci/drivers/i915"));
    f.symlink(
        f.0.join("sys/bus/pci/drivers/i915"),
        f.0.join(format!("{physical}/driver")),
    );
    f.create_dir_all(f.0.join("sys/class/drm/card7"));
    f.symlink(f.0.join(physical), f.0.join("sys/class/drm/card7/device"));
    let snapshot = HostCollector::rooted(f.0.clone()).collect_at(1);
    let gpus: Vec<_> = snapshot
        .monitors
        .iter()
        .filter(|m| m.kind == MonitorKind::Gpu)
        .collect();
    assert_eq!(
        gpus.len(),
        1,
        "Intel physical device must produce a monitor"
    );
    assert_eq!(gpus[0].id, "intel-pci:0000:00:02.0");
    f.verify()?;
    Ok(())
}

impl Fixture {
    fn intel(&self, pci: &str, driver: &str, nodes: &[&str]) -> String {
        let physical = format!("sys/devices/pci0000:00/{pci}");
        self.put(&format!("{physical}/vendor"), "0x8086");
        self.put(&format!("{physical}/device"), "0x46a6");
        self.put(
            &format!("{physical}/uevent"),
            &format!("DRIVER={driver}\nPCI_SLOT_NAME={pci}\n"),
        );
        self.create_dir_all(self.0.join(format!("sys/bus/pci/drivers/{driver}")));
        self.symlink(
            self.0.join(format!("sys/bus/pci/drivers/{driver}")),
            self.0.join(format!("{physical}/driver")),
        );
        for node in nodes {
            self.create_dir_all(self.0.join(format!("sys/class/drm/{node}")));
            self.symlink(
                self.0.join(&physical),
                self.0.join(format!("sys/class/drm/{node}/device")),
            );
        }
        physical
    }
}

#[path = "tests/host.rs"]
mod host;
#[path = "tests/intel.rs"]
mod intel;
