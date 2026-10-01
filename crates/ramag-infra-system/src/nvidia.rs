//! Runtime-loaded NVML. No linked NVIDIA library and no assumed GPU enumeration index identity.
use crate::{
    counters::*,
    host::{diagnostic, monitor, sensor},
    types::*,
};
use nvml_wrapper::{
    Nvml,
    enum_wrappers::device::{Clock, TemperatureSensor},
};
use std::collections::BTreeMap;
type Value = Result<u64, String>;
type Windows = BTreeMap<String, (u64, u64)>;
pub(crate) type PciAddress = (u32, u32, u32, u32);
struct DeviceSample {
    pci_address: Option<PciAddress>,
    uuid: Result<String, String>,
    name: Result<String, String>,
    values: BTreeMap<String, Value>,
    memory: Result<(u64, u64), String>,
    fans: Vec<(Value, Value)>,
    windows: Windows,
}
trait NvmlBackend: Send {
    fn devices(&mut self, clock: &CaptureClock) -> Result<Vec<DeviceSample>, String>;
}
trait NvmlSession: Send {
    fn count(&mut self) -> Result<u32, String>;
    fn sample(&mut self, index: u32, clock: &CaptureClock) -> Result<DeviceSample, String>;
}
struct CaptureClock {
    base: u64,
    origin: std::time::Instant,
}
impl CaptureClock {
    #[cfg(test)]
    fn new(base: u64) -> Self {
        Self {
            base,
            origin: std::time::Instant::now(),
        }
    }
    fn now(&self) -> u64 {
        self.base
            .saturating_add(self.origin.elapsed().as_nanos().min(u64::MAX as u128) as u64)
    }
}
type Factory = Box<dyn FnMut() -> Result<Box<dyn NvmlSession>, String> + Send>;
struct RuntimeNvml {
    session: Option<Box<dyn NvmlSession>>,
    factory: Factory,
}
pub(crate) struct NvidiaCollector {
    backend: Box<dyn NvmlBackend>,
}
impl RuntimeNvml {
    fn new() -> Self {
        Self {
            session: None,
            factory: Box::new(|| {
                Nvml::init()
                    .map(|nvml| Box::new(nvml) as Box<dyn NvmlSession>)
                    .map_err(|e| format!("NVML init: {e:?}"))
            }),
        }
    }
    #[cfg(test)]
    fn injected(
        factory: impl FnMut() -> Result<Box<dyn NvmlSession>, String> + Send + 'static,
    ) -> Self {
        Self {
            session: None,
            factory: Box::new(factory),
        }
    }
}
impl NvmlBackend for RuntimeNvml {
    fn devices(&mut self, clock: &CaptureClock) -> Result<Vec<DeviceSample>, String> {
        if self.session.is_none() {
            self.session = Some((self.factory)()?);
        }
        let session = self
            .session
            .as_mut()
            .ok_or("NVML initialization yielded no session")?;
        let count = match session.count() {
            Ok(v) => v,
            Err(e) => {
                self.session = None;
                return Err(e);
            }
        };
        let mut lost = false;
        let mut result = Vec::new();
        for index in 0..count {
            match session.sample(index, clock) {
                Ok(sample) => {
                    let lost_error =
                        |e: &String| e.contains("GpuLost") || e.contains("DriverNotLoaded");
                    if sample
                        .values
                        .values()
                        .any(|v| v.as_ref().is_err_and(lost_error))
                        || sample.memory.as_ref().is_err_and(lost_error)
                        || sample.uuid.as_ref().is_err_and(lost_error)
                        || sample.name.as_ref().is_err_and(lost_error)
                        || sample.fans.iter().any(|(p, r)| {
                            p.as_ref().is_err_and(lost_error) || r.as_ref().is_err_and(lost_error)
                        })
                    {
                        lost = true;
                    }
                    result.push(sample);
                }
                Err(e) => {
                    lost |= e.contains("GpuLost") || e.contains("DriverNotLoaded");
                    result.push(DeviceSample {
                        pci_address: None,
                        uuid: Err(e.clone()),
                        name: Err(e.clone()),
                        values: BTreeMap::new(),
                        memory: Err(e),
                        fans: Vec::new(),
                        windows: BTreeMap::new(),
                    });
                }
            }
        }
        if lost {
            self.session = None;
        }
        Ok(result)
    }
}
fn query<T>(
    clock: &CaptureClock,
    windows: &mut Windows,
    key: &str,
    source: &str,
    operation: impl FnOnce() -> Result<T, nvml_wrapper::error::NvmlError>,
) -> Result<T, String> {
    let start = clock.now();
    let value = operation().map_err(|e| format!("{source}: {e:?}"));
    windows.insert(key.into(), (start, clock.now()));
    value
}
impl NvmlSession for Nvml {
    fn count(&mut self) -> Result<u32, String> {
        self.device_count()
            .map_err(|e| format!("NVML device_count: {e:?}"))
    }
    fn sample(&mut self, index: u32, clock: &CaptureClock) -> Result<DeviceSample, String> {
        let device = self
            .device_by_index(index)
            .map_err(|e| format!("NVML device_by_index({index}): {e:?}"))?;
        let mut windows = Windows::new();
        let mut values = BTreeMap::new();
        let uuid = device.uuid().map_err(|e| format!("NVML uuid: {e:?}"));
        let name = device.name().map_err(|e| format!("NVML name: {e:?}"));
        values.insert(
            "usage".into(),
            query(
                clock,
                &mut windows,
                "usage",
                "NVML utilization_rates",
                || device.utilization_rates(),
            )
            .map(|v| v.gpu as u64),
        );
        values.insert(
            "temperature".into(),
            query(
                clock,
                &mut windows,
                "temperature",
                "NVML temperature(Gpu)",
                || device.temperature(TemperatureSensor::Gpu),
            )
            .map(u64::from),
        );
        values.insert(
            "power".into(),
            query(clock, &mut windows, "power", "NVML power_usage", || {
                device.power_usage()
            })
            .map(u64::from),
        );
        values.insert(
            "clock-graphics".into(),
            query(
                clock,
                &mut windows,
                "clock-graphics",
                "NVML clock_info(Graphics)",
                || device.clock_info(Clock::Graphics),
            )
            .map(u64::from),
        );
        values.insert(
            "clock-memory".into(),
            query(
                clock,
                &mut windows,
                "clock-memory",
                "NVML clock_info(Memory)",
                || device.clock_info(Clock::Memory),
            )
            .map(u64::from),
        );
        let memory = query(clock, &mut windows, "vram", "NVML memory_info v2", || {
            device.memory_info()
        })
        .map(|v| (v.used, v.total));
        let fan_count = device.num_fans();
        let count = match fan_count {
            Ok(n) => n,
            Err(e) => {
                values.insert("fan-count".into(), Err(format!("NVML num_fans: {e:?}")));
                1
            }
        };
        let mut fans = Vec::new();
        for fan in 0..count {
            let percent = query(
                clock,
                &mut windows,
                &format!("fan-{fan}-percent"),
                &format!("NVML fan_speed({fan})"),
                || device.fan_speed(fan),
            )
            .map(u64::from);
            let rpm = query(
                clock,
                &mut windows,
                &format!("fan-{fan}-rpm"),
                &format!("NVML fan_speed_rpm({fan})"),
                || device.fan_speed_rpm(fan),
            )
            .map(u64::from);
            fans.push((percent, rpm));
        }
        if count == 0 {
            fans.push((
                Err("NVML num_fans: device exposes no fans".into()),
                Err("NVML num_fans: device exposes no fans".into()),
            ));
        }
        Ok(DeviceSample {
            #[cfg(target_os = "windows")]
            pci_address: device
                .pci_info()
                .ok()
                .and_then(|info| parse_pci_address(&info.bus_id)),
            #[cfg(not(target_os = "windows"))]
            pci_address: None,
            uuid,
            name,
            values,
            memory,
            fans,
            windows,
        })
    }
}
fn timed(mut reading: Reading, windows: &Windows, key: &str) -> Reading {
    if let Some((start, end)) = windows.get(key) {
        for o in &mut reading.observations {
            o.read_started_ns = Some(*start);
            o.captured_ns = *end;
        }
    }
    reading
}
fn error_status(e: &str) -> Availability {
    if e.contains("NotSupported") || e.contains("FailedToLoadSymbol") || e.contains("no fans") {
        Availability::Unavailable
    } else {
        Availability::Failed
    }
}
impl NvidiaCollector {
    pub(crate) fn new() -> Self {
        Self {
            backend: Box::new(RuntimeNvml::new()),
        }
    }
    #[cfg(not(target_os = "windows"))]
    pub(crate) fn collect_at_origin(&mut self, s: &mut Snapshot, origin: std::time::Instant) {
        let clock = CaptureClock { base: 0, origin };
        let ns = clock.now();
        self.collect_clock(s, ns, &clock, None);
    }
    #[cfg(any(test, target_os = "windows"))]
    pub(crate) fn collect_windows_at_origin(
        &mut self,
        s: &mut Snapshot,
        origin: std::time::Instant,
        identities: &BTreeMap<PciAddress, String>,
    ) {
        let clock = CaptureClock { base: 0, origin };
        self.collect_clock(s, clock.now(), &clock, Some(identities));
    }
    #[cfg(test)]
    fn collect(&mut self, s: &mut Snapshot, ns: u64) {
        self.collect_clock(s, ns, &CaptureClock::new(ns), None);
    }
    fn collect_clock(
        &mut self,
        s: &mut Snapshot,
        ns: u64,
        clock: &CaptureClock,
        identities: Option<&BTreeMap<PciAddress, String>>,
    ) {
        let devices = match self.backend.devices(clock) {
            Ok(v) => v,
            Err(e) => {
                diagnostic(s, "nvml", e);
                return;
            }
        };
        s.diagnostics.push(BackendDiagnostic {
            backend: "nvml".into(),
            availability: Availability::Available,
            reason: format!("NVML discovered {} devices", devices.len()),
        });
        let mut pci_counts = BTreeMap::new();
        if identities.is_some() {
            for device in &devices {
                if let Some(pci) = device.pci_address {
                    *pci_counts.entry(pci).or_insert(0usize) += 1;
                }
            }
        }
        for device in devices {
            let uuid = match device.uuid {
                Ok(v) if !v.is_empty() => v,
                Ok(_) => {
                    diagnostic(s, "nvml", "NVML uuid returned an empty identifier".into());
                    continue;
                }
                Err(e) => {
                    diagnostic(s, "nvml", e);
                    continue;
                }
            };
            let id = if let Some(identities) = identities {
                match device
                    .pci_address
                    .filter(|pci| pci_counts.get(pci) == Some(&1))
                    .and_then(|pci| identities.get(&pci))
                {
                    Some(id) => id.clone(),
                    None => {
                        diagnostic(
                            s,
                            "nvml",
                            format!("No unambiguous Windows PnP identity for NVML device {uuid}"),
                        );
                        continue;
                    }
                }
            } else {
                format!("nvidia:{uuid}")
            };
            let name = match device.name {
                Ok(v) => v,
                Err(e) => {
                    diagnostic(s, "nvml", e);
                    "NVIDIA GPU".into()
                }
            };
            monitor(
                s,
                &id,
                &format!("{name} · {}", &uuid[uuid.len().saturating_sub(8)..]),
                MonitorKind::Gpu,
            );
            for (suffix, title, kind, unit, source, factor) in [
                (
                    "usage",
                    "GPU utilization",
                    SensorKind::Percentage,
                    Unit::Percent,
                    "NVML utilization_rates().gpu (%)",
                    1.0,
                ),
                (
                    "temperature",
                    "GPU temperature",
                    SensorKind::Temperature,
                    Unit::Celsius,
                    "NVML temperature(Gpu) (C)",
                    1.0,
                ),
                (
                    "power",
                    "GPU power",
                    SensorKind::Power,
                    Unit::Watts,
                    "NVML power_usage() (mW)",
                    0.001,
                ),
                (
                    "clock-graphics",
                    "Graphics clock",
                    SensorKind::Frequency,
                    Unit::Hertz,
                    "NVML clock_info(Graphics) (MHz)",
                    1e6,
                ),
                (
                    "clock-memory",
                    "Memory clock",
                    SensorKind::Frequency,
                    Unit::Hertz,
                    "NVML clock_info(Memory) (MHz)",
                    1e6,
                ),
            ] {
                let value = device
                    .values
                    .get(suffix)
                    .cloned()
                    .unwrap_or_else(|| Err(format!("{source}: no result")));
                let r = timed(
                    nvml_reading(&format!("{id}/{suffix}"), source, value, factor, ns),
                    &device.windows,
                    suffix,
                );
                sensor(
                    s,
                    (&id, suffix, title),
                    kind,
                    unit,
                    source,
                    "Physical NVIDIA device identified by GPU UUID",
                    r,
                );
            }
            let sid = format!("{id}/vram");
            let source = "NVML memory_info v2 (bytes)";
            let r = match device.memory {
                Ok((used, total)) if used <= total => {
                    let mut r = measured(&sid, used as f64, Some(total as f64));
                    r.observations
                        .push(raw(source, ns, [("used", used), ("total", total)]));
                    r
                }
                Ok(_) => missing(
                    &sid,
                    Availability::Failed,
                    format!("{source}: used exceeds total"),
                ),
                Err(e) => missing(&sid, error_status(&e), e),
            };
            let r = timed(r, &device.windows, "vram");
            sensor(
                s,
                (&id, "vram", "VRAM used"),
                SensorKind::Capacity,
                Unit::Bytes,
                source,
                "Physical device VRAM",
                r,
            );
            if let Some(Err(e)) = device.values.get("fan-count") {
                diagnostic(s, "nvml", e.clone());
            }
            for (index, (percent, rpm)) in device.fans.into_iter().enumerate() {
                for (suffix, title, unit, value, source) in [
                    (
                        format!("fan-{index}-percent"),
                        format!("Fan {index} intended speed"),
                        Unit::Percent,
                        percent,
                        format!("NVML fan_speed({index}) (%)"),
                    ),
                    (
                        format!("fan-{index}-rpm"),
                        format!("Fan {index} intended RPM"),
                        Unit::Rpm,
                        rpm,
                        format!("NVML fan_speed_rpm({index}) (RPM)"),
                    ),
                ] {
                    let r = timed(
                        nvml_reading(&format!("{id}/{suffix}"), &source, value, 1.0, ns),
                        &device.windows,
                        &suffix,
                    );
                    sensor(
                        s,
                        (&id, &suffix, &title),
                        SensorKind::Fan,
                        unit,
                        &source,
                        "Intended device fan speed; NVML does not prove actual rotation; percent may exceed 100 and is never converted to RPM",
                        r,
                    );
                }
            }
        }
    }
}
#[cfg(any(test, target_os = "windows"))]
fn parse_pci_address(value: &str) -> Option<PciAddress> {
    let (address, function) = value.split_once('.')?;
    let mut parts = address.split(':');
    let number = |text: &str| {
        if text.is_empty() || text.len() > 8 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(text, 16).ok()
    };
    let domain = number(parts.next()?)?;
    let bus = number(parts.next()?)?;
    let device = number(parts.next()?)?;
    let function = number(function)?;
    (parts.next().is_none() && bus < 256 && device < 32 && function < 8)
        .then_some((domain, bus, device, function))
}
fn nvml_reading(id: &str, source: &str, value: Value, factor: f64, ns: u64) -> Reading {
    match value {
        Ok(v) => {
            let mut r = measured(id, v as f64 * factor, None);
            r.observations.push(raw(source, ns, [("value", v)]));
            r
        }
        Err(e) => {
            let status = error_status(&e);
            missing(id, status, e)
        }
    }
}
#[cfg(test)]
#[path = "nvidia/tests.rs"]
mod tests;
