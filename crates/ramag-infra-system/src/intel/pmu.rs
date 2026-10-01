//! Direct Linux performance counters. Percentages retain their engine scope.
use super::{
    Clock,
    native::{Counter, PerfCounter},
};
use super::{Device, drm::Engine, sysfs};
use crate::{counters::*, types::*};
use std::collections::BTreeMap;
use std::{
    io,
    path::{Path, PathBuf},
};
type Factory = Box<dyn FnMut(&Spec) -> io::Result<Box<dyn Counter>> + Send>;
struct Slot {
    spec: Spec,
    handle: Option<Box<dyn Counter>>,
}
pub(super) struct Pmu {
    slots: BTreeMap<String, Slot>,
    counters: Counters,
    factory: Factory,
}
impl Pmu {
    pub(super) fn new() -> Self {
        Self {
            slots: BTreeMap::new(),
            counters: Counters::default(),
            factory: Box::new(PerfCounter::open),
        }
    }
    fn sample_specs(
        &mut self,
        s: &mut Snapshot,
        id: &str,
        clock: &Clock,
        specs: io::Result<Vec<Spec>>,
    ) {
        let prefix = format!("{id}/");
        let error = match specs {
            Ok(mut specs) => {
                // A failed event config cannot establish an engine class. Preserve
                // the previous attributable identity when this event was known.
                for spec in &mut specs {
                    if spec.metadata_error.is_some()
                        && let Some(slot) = self.slots.iter().find_map(|(key, slot)| {
                            (key.starts_with(&prefix)
                                && slot.spec.event_path == spec.event_path
                                && spec.event_path.is_some())
                            .then_some(slot)
                        })
                    {
                        let error = spec.metadata_error.take();
                        *spec = slot.spec.clone();
                        spec.metadata_error = error;
                    }
                }
                self.slots.retain(|key, _| {
                    !key.starts_with(&prefix)
                        || specs.iter().any(|v| key == &format!("{prefix}{}", v.id))
                });
                for spec in specs {
                    let key = format!("{prefix}{}", spec.id);
                    if self.slots.get(&key).is_some_and(|slot| slot.spec != spec) {
                        self.slots.remove(&key);
                        self.counters.derive(
                            &key,
                            Err("PMU configuration changed".into()),
                            utilization,
                        );
                    }
                    self.slots.entry(key).or_insert(Slot { spec, handle: None });
                }
                None
            }
            Err(e) => Some(e),
        };
        let mut first = None;
        let mut count = 0;
        for (key, slot) in self
            .slots
            .iter_mut()
            .filter(|(key, _)| key.starts_with(&prefix))
        {
            count += 1;
            if slot.spec.energy_denominator == 0 {
                first.get_or_insert_with(|| key.clone());
            }
            let spec = &slot.spec;
            let source = format!(
                "{} perf_event_open config={:#x}{} ({})",
                spec.pmu.display(),
                spec.active,
                spec.total.map_or(String::new(), |v| format!(",{v:#x}")),
                if spec.energy_denominator > 0 {
                    "2^-32 Joules"
                } else if spec.total.is_some() {
                    "GuC ticks"
                } else {
                    "busy ns"
                }
            );
            let start = clock.now();
            let result = (|| {
                if let Some(e) = &error {
                    return Err(io::Error::new(e.kind(), e.to_string()));
                }
                if let Some((kind, reason)) = &spec.metadata_error {
                    return Err(io::Error::new(*kind, reason.clone()));
                }
                if slot.handle.is_none() {
                    slot.handle = Some((self.factory)(spec)?);
                }
                slot.handle
                    .as_mut()
                    .ok_or_else(|| io::Error::other("No perf handle"))?
                    .read()
            })();
            let end = clock.now();
            let failure = result
                .as_ref()
                .err()
                .map(|e| (super::drm::error_availability(e), format!("{source}: {e}")));
            let observation = result.map(|v| {
                raw_window(
                    &source,
                    start,
                    end,
                    [
                        ("active", v.active),
                        ("total", v.total),
                        ("ticks", u64::from(spec.total.is_some())),
                        ("enabled_ns", v.enabled),
                        ("running_ns", v.running),
                        ("capacity", 1),
                        ("energy_denominator", spec.energy_denominator),
                        ("config_active", spec.active),
                        ("config_total", spec.total.unwrap_or(0)),
                        ("pmu_type", u64::from(spec.kind)),
                        ("cpu", spec.cpu as u64),
                    ],
                )
            });
            let mut r = self.counters.derive(
                key,
                observation.map_err(|e| format!("{source}: {e}")),
                utilization,
            );
            if let Some((availability, reason)) = failure {
                slot.handle = None;
                r.availability = availability;
                r.reason = Some(reason);
                r.observations.push(raw_window(&source, start, end, []));
            }
            crate::host::sensor(
                s,
                (
                    id,
                    &spec.id,
                    &format!(
                        "{} {}",
                        spec.id,
                        if spec.energy_denominator > 0 {
                            "power"
                        } else {
                            "activity"
                        }
                    ),
                ),
                if spec.energy_denominator > 0 {
                    SensorKind::Power
                } else {
                    SensorKind::Percentage
                },
                if spec.energy_denominator > 0 {
                    Unit::Watts
                } else {
                    Unit::Percent
                },
                &source,
                &spec.scope,
                r,
            );
        }
        if let Some(key) = first {
            if let Some(m) = s.monitors.iter_mut().find(|m| m.id == id) {
                m.summary_sensor_id = key;
            }
        } else if count == 0 {
            let availability = error
                .as_ref()
                .map_or(Availability::Unavailable, super::drm::error_availability);
            let reason =
                error.map_or_else(|| "No engine counters exposed".into(), |e| e.to_string());
            let r = missing(&format!("{id}/usage"), availability, reason);
            crate::host::sensor(
                s,
                (id, "usage", "GPU engine activity"),
                SensorKind::Percentage,
                Unit::Percent,
                "Linux Intel device PMU",
                "No whole-device activity inferred from clients or clocks",
                r,
            );
        }
        let keys: Vec<_> = self.slots.keys().map(String::as_str).collect();
        self.counters.retain(&keys);
    }
    pub(super) fn sample_power(
        &mut self,
        s: &mut Snapshot,
        id: &str,
        clock: &Clock,
        spec: io::Result<Spec>,
    ) {
        match spec {
            Ok(spec) => self.sample_specs(s, id, clock, Ok(vec![spec])),
            Err(e)
                if self
                    .slots
                    .keys()
                    .any(|key| key.starts_with(&format!("{id}/"))) =>
            {
                self.sample_specs(s, id, clock, Err(e))
            }
            Err(e) => {
                let r = missing(
                    &format!("{id}/rapl-graphics-power"),
                    super::drm::error_availability(&e),
                    e.to_string(),
                );
                crate::host::sensor(
                    s,
                    (id, "rapl-graphics-power", "RAPL graphics-domain power"),
                    SensorKind::Power,
                    Unit::Watts,
                    "Intel RAPL energy-gpu",
                    "Attribution requires one evidenced integrated GPU and one RAPL die",
                    r,
                );
            }
        }
    }
    pub(super) fn retain(&mut self, ids: &[String]) {
        self.slots
            .retain(|key, _| ids.iter().any(|id| key.starts_with(&format!("{id}/"))));
        let keys: Vec<_> = self.slots.keys().map(String::as_str).collect();
        self.counters.retain(&keys);
    }
    pub(super) fn collect(
        &mut self,
        s: &mut Snapshot,
        device: &Device,
        root: &Path,
        clock: &Clock,
        engines: io::Result<Vec<Engine>>,
        unqualified_i915: bool,
    ) {
        let specs = (|| {
            let directory = root.join("sys/bus/event_source/devices");
            let mut pmu = directory.join(format!(
                "{}_{}",
                device.driver,
                device.pci.replace(':', "_")
            ));
            if device.driver == "i915" && !pmu.exists() && unqualified_i915 {
                pmu = directory.join("i915");
            }
            let cpus = match sysfs::text(&pmu.join("cpumask")) {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    sysfs::text(&root.join("sys/devices/system/cpu/online"))?
                }
                Err(e) => return Err(e),
            };
            let cpu = first_cpu(&cpus)?;
            let engines = if device.driver == "xe" {
                engines?
            } else {
                engines.unwrap_or_default()
            };
            specifications(device, &pmu, &engines, cpu)
        })();
        self.sample_specs(s, &format!("intel-pci:{}", device.pci), clock, specs);
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Spec {
    pub id: String,
    pub pmu: PathBuf,
    pub kind: u32,
    pub cpu: i32,
    pub active: u64,
    pub total: Option<u64>,
    pub energy_denominator: u64,
    pub scope: String,
    pub event_path: Option<PathBuf>,
    pub metadata_error: Option<(io::ErrorKind, String)>,
}
fn first_cpu(list: &str) -> io::Result<i32> {
    let mut first = None;
    for range in list.trim().split(',') {
        let mut parts = range.split('-');
        let start = parts
            .next()
            .unwrap_or("")
            .parse::<i32>()
            .map_err(|_| invalid("Invalid PMU CPU list"))?;
        let end = parts
            .next()
            .map(str::parse::<i32>)
            .transpose()
            .map_err(|_| invalid("Invalid PMU CPU range"))?
            .unwrap_or(start);
        if start < 0 || end < start || parts.next().is_some() {
            return Err(invalid("Invalid PMU CPU range"));
        }
        first = Some(first.map_or(start, |old: i32| old.min(start)));
    }
    first.ok_or_else(|| invalid("Empty PMU CPU list"))
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn event(path: &Path, key: &str) -> io::Result<u64> {
    let text = sysfs::text(path)?;
    let value = text
        .strip_prefix(key)
        .ok_or_else(|| invalid("Unexpected PMU config format"))?;
    let value = value
        .strip_prefix("0x")
        .ok_or_else(|| invalid("PMU config must be hexadecimal"))?;
    u64::from_str_radix(value, 16).map_err(|_| invalid("Invalid PMU config value"))
}
fn specifications(
    device: &Device,
    pmu: &Path,
    engines: &[Engine],
    cpu: i32,
) -> io::Result<Vec<Spec>> {
    let kind = sysfs::text(&pmu.join("type"))?
        .parse()
        .map_err(|_| invalid("Invalid PMU type"))?;
    let mut specs = Vec::new();
    if device.driver == "i915" {
        for path in sysfs::entries(&pmu.join("events"))? {
            let Some(name) = path.file_name() else {
                continue;
            };
            let name = name.to_string_lossy().into_owned();
            if !name.ends_with("-busy") {
                continue;
            }
            specs.push(i915_spec(pmu, &path, &name, kind, cpu, &sysfs::text));
        }
    } else {
        for (key, expected) in [
            ("event", "config:0-11"),
            ("engine_class", "config:20-27"),
            ("engine_instance", "config:12-19"),
            ("gt", "config:60-63"),
        ] {
            if sysfs::text(&pmu.join(format!("format/{key}")))? != expected {
                return Err(invalid(
                    "xe PMU layout differs from supported documented format",
                ));
            }
        }
        let active = event(&pmu.join("events/engine-active-ticks"), "event=")?;
        let total = event(&pmu.join("events/engine-total-ticks"), "event=")?;
        if active != 2 || total != 3 {
            return Err(invalid("Unexpected xe active/total event IDs"));
        }
        for name in ["engine-active-ticks", "engine-total-ticks"] {
            match sysfs::text(&pmu.join(format!("events/{name}.unit"))) {
                Ok(unit) if unit != "ticks" => return Err(invalid("Unexpected xe tick unit")),
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        let sriov = sysfs::text(&device.path.join("sriov_numvfs"))
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|v| v > 0);
        for engine in engines {
            if engine.gt > 15 || engine.instance > 255 || engine.class > 4 {
                return Err(invalid("Engine does not fit xe PMU selectors"));
            }
            let selectors = (u64::from(engine.gt) << 60)
                | (u64::from(engine.class) << 20)
                | (u64::from(engine.instance) << 12);
            specs.push(Spec {
                id: format!(
                    "gt{}-engine-class{}-instance{}",
                    engine.gt, engine.class, engine.instance
                ),
                event_path: None,
                metadata_error: None,
                pmu: pmu.into(),
                kind,
                cpu,
                active: selectors | active,
                total: Some(selectors | total),
                energy_denominator: 0,
                scope: format!(
                    "{}; GT {}, engine class {}, instance {}; active/total GuC ticks, capacity 1",
                    if sriov {
                        "PCI physical function 0 only; excludes VF activity"
                    } else {
                        "Physical GPU engine"
                    },
                    engine.gt,
                    engine.class,
                    engine.instance
                ),
            });
        }
    }
    specs.sort_by(|a, b| a.id.cmp(&b.id));
    if specs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "No documented engine counters exposed",
        ));
    }
    if specs.windows(2).any(|s| s[0].id == s[1].id) {
        return Err(invalid("Duplicate engine PMU configuration"));
    }
    Ok(specs)
}
fn i915_spec(
    pmu: &Path,
    path: &Path,
    name: &str,
    kind: u32,
    cpu: i32,
    read: &impl Fn(&Path) -> io::Result<String>,
) -> Spec {
    let config = (|| {
        let unit_path = pmu.join(format!("events/{name}.unit"));
        let unit = read(&unit_path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", unit_path.display())))?;
        if unit != "ns" {
            return Err(invalid(&format!(
                "{}: i915 busy counter unit is not ns",
                unit_path.display()
            )));
        }
        let text = read(path)?;
        let value = text
            .strip_prefix("config=0x")
            .ok_or_else(|| invalid("Unexpected PMU config format"))?;
        let config =
            u64::from_str_radix(value, 16).map_err(|_| invalid("Invalid PMU config value"))?;
        if config & 0xf != 0 || config >> 12 > 4 {
            return Err(invalid("Unknown i915 engine busy configuration"));
        }
        Ok(config)
    })();
    let (id, active, scope, metadata_error) = match config {
        Ok(config) => {
            let class = config >> 12;
            let instance = (config >> 4) & 0xff;
            (
                format!("engine-class{class}-instance{instance}"),
                config,
                format!(
                    "Physical GPU engine class {class}, instance {instance}; busy nanoseconds / measured elapsed nanoseconds; capacity 1"
                ),
                None,
            )
        }
        Err(e) => (
            format!("event-{name}"),
            0,
            "PMU event metadata unavailable; no engine identity inferred".into(),
            Some((e.kind(), format!("{}: {e}", path.display()))),
        ),
    };
    Spec {
        id,
        pmu: pmu.into(),
        kind,
        cpu,
        active,
        total: None,
        energy_denominator: 0,
        scope,
        event_path: Some(path.into()),
        metadata_error,
    }
}
fn utilization(a: &RawObservation, b: &RawObservation, elapsed: f64) -> Result<f64, String> {
    if !elapsed.is_finite() || elapsed <= 0.0 {
        return Err("Invalid measured elapsed time".into());
    }
    for key in [
        "ticks",
        "capacity",
        "config_active",
        "config_total",
        "energy_denominator",
    ] {
        if a.integers.get(key) != b.integers.get(key) {
            return Err(format!("Counter metadata changed: {key}"));
        }
    }
    let enabled = delta(a, b, "enabled_ns")?;
    let running = delta(a, b, "running_ns")?;
    if enabled == 0 || running != enabled {
        return Err("PMU was not continuously scheduled; no scaled estimate".into());
    }
    let active = delta(a, b, "active")?;
    if let Some(denominator) = b.integers.get("energy_denominator").filter(|v| **v > 0) {
        if *denominator != 1u64 << 32 {
            return Err("Unknown energy unit".into());
        }
        return Ok(active as f64 / *denominator as f64 / elapsed);
    }
    let total = if b.integers.get("ticks") == Some(&1) {
        delta(a, b, "total")? as f64
    } else {
        elapsed * 1e9
    };
    if total <= 0.0 || active as f64 > total || b.integers.get("capacity") != Some(&1) {
        return Err("Invalid engine activity/capacity; no clamp".into());
    }
    Ok(active as f64 / total * 100.0)
}
#[cfg(test)]
#[path = "../pmu/tests.rs"]
mod tests;
