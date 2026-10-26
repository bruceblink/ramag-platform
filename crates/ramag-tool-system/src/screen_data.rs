//! Screen projections over the accepted snapshot. No collection or synthetic samples.
#[cfg(test)]
use crate::test_support::TestUnwrapExt;
use crate::workspace::Data;
use ramag_infra_system::MonitorKind;
use system_pulse_model::{PhysicalUnit, Quantity, ReadingStatus, Sample, Screen};

#[derive(Clone, Debug)]
pub(crate) struct Channel {
    pub monitor: String,
    pub sensor: String,
    pub label: String,
    pub device: String,
    pub scope: String,
    pub quantity: Quantity,
    pub unit: PhysicalUnit,
}

impl Channel {
    pub fn latest<'a>(&self, data: &'a Data) -> Option<&'a Sample> {
        data.history.latest(&self.monitor, &self.sensor)
    }

    pub fn samples(&self, data: &Data) -> Vec<Sample> {
        data.history
            .samples(&self.monitor, &self.sensor)
            .map(|samples| samples.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn value(&self, data: &Data) -> String {
        compact_value(self.latest(data))
    }

    pub fn measured(&self, data: &Data) -> Option<f64> {
        self.latest(data).and_then(Sample::chart_value)
    }

    pub fn visible(&self, data: &Data) -> bool {
        sensor_visible(data, &self.monitor, &self.sensor)
            && self
                .latest(data)
                .is_some_and(|sample| sample.status != ReadingStatus::Unavailable)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeviceChoice {
    pub id: String,
    pub label: String,
}

pub(crate) fn compact_value(sample: Option<&Sample>) -> String {
    match sample {
        Some(sample) if sample.value.is_some() => {
            let value = format!("{} {}", sample.text, sample.unit).trim().to_owned();
            if sample.status == ReadingStatus::Stale {
                format!("{value} · 数据过期")
            } else {
                value
            }
        }
        Some(sample) => match sample.status {
            ReadingStatus::WarmingUp => "数据准备中".into(),
            ReadingStatus::Failed => "读取失败".into(),
            ReadingStatus::Stale => "数据过期".into(),
            _ => "不可用".into(),
        },
        None => "等待数据".into(),
    }
}

pub(crate) fn sensor_visible(data: &Data, monitor: &str, sensor: &str) -> bool {
    data.session
        .workspace
        .panels
        .get(monitor)
        .and_then(|panel| panel.sensors.get(sensor))
        .is_none_or(|sensor| sensor.visible)
}

fn channel(
    monitor: &system_pulse_model::MonitorDescriptor,
    sensor: &system_pulse_model::SensorDescriptor,
    scope: String,
) -> Channel {
    Channel {
        monitor: monitor.id.clone(),
        sensor: sensor.id.clone(),
        device: monitor.title.clone(),
        label: display_sensor_title(&sensor.title),
        scope,
        quantity: sensor.quantity,
        unit: sensor.unit,
    }
}

/// Translate stable collector labels while preserving unknown platform-provided names.
fn display_sensor_title(title: &str) -> String {
    if let Some(core) = title
        .strip_prefix("CPU ")
        .and_then(|title| title.strip_suffix(" utilization"))
    {
        return format!("CPU {core} 使用率");
    }
    if let Some(core) = title
        .strip_prefix("CPU ")
        .and_then(|title| title.strip_suffix(" frequency"))
    {
        return format!("CPU {core} 频率");
    }

    match title {
        "Overall utilization" => "总体使用率".into(),
        "RAM used" => "内存使用量".into(),
        "RAM total" => "内存总量".into(),
        "RAM available" => "可用内存".into(),
        "RAM free" => "空闲内存".into(),
        "Swap used" => "已用交换空间".into(),
        "Swap total" => "交换空间总量".into(),
        "Cache" | "cache" => "缓存".into(),
        "Other" => "其他".into(),
        "Buffers" | "buffers" => "缓冲区".into(),
        "Free" => "可用".into(),
        "GPU utilization" => "GPU 使用率".into(),
        "VRAM used" => "已用显存".into(),
        "Dedicated GPU memory" => "GPU 专用内存".into(),
        "Shared GPU memory" => "GPU 共享内存".into(),
        "CPU temperature" => "CPU 温度".into(),
        "GPU temperature" => "GPU 温度".into(),
        "GPU power" => "GPU 功耗".into(),
        "Graphics clock" => "图形频率".into(),
        "Memory clock" => "显存频率".into(),
        "Read throughput" => "读取速率".into(),
        "Write throughput" => "写入速率".into(),
        "Bytes received" => "接收字节数".into(),
        "Bytes sent" => "发送字节数".into(),
        "Packets received" => "接收数据包".into(),
        "Packets sent" => "发送数据包".into(),
        "read" | "Read" => "读取".into(),
        "write" | "Write" => "写入".into(),
        "rx" => "接收".into(),
        "tx" => "发送".into(),
        other => other.to_owned(),
    }
}

pub(crate) fn channels(data: &Data) -> Vec<Channel> {
    let scopes: std::collections::BTreeMap<_, _> = data
        .snapshot
        .iter()
        .flat_map(|snapshot| snapshot.sensors.iter())
        .map(|sensor| (sensor.id.as_str(), sensor.scope.as_str()))
        .collect();
    data.catalog
        .iter()
        .flat_map(|monitor| {
            monitor.sensors.iter().map(|sensor| {
                channel(
                    monitor,
                    sensor,
                    scopes
                        .get(sensor.id.as_str())
                        .copied()
                        .unwrap_or_default()
                        .to_owned(),
                )
            })
        })
        .collect()
}

pub(crate) fn monitor_channels(data: &Data, id: &str) -> Vec<Channel> {
    ordered_monitor_channels(data, id)
}

pub(crate) fn gpu_channels(data: &Data, id: &str) -> Vec<Channel> {
    ordered_monitor_channels(data, id)
}

fn ordered_monitor_channels(data: &Data, id: &str) -> Vec<Channel> {
    let mut rows: Vec<_> = channels(data)
        .into_iter()
        .filter(|channel| channel.monitor == id && channel.visible(data))
        .collect();
    rows.sort_by_key(|channel| {
        let order = data
            .session
            .workspace
            .panels
            .get(id)
            .and_then(|panel| panel.sensors.get(&channel.sensor))
            .map_or(0, |sensor| sensor.order);
        (order, channel.sensor.clone())
    });
    rows
}

pub(crate) fn find(data: &Data, monitor: &str, suffix: &str) -> Option<Channel> {
    let monitor = data
        .catalog
        .iter()
        .find(|candidate| candidate.id == monitor)?;
    let id = format!("{}/{suffix}", monitor.id);
    let sensor = monitor.sensors.iter().find(|sensor| sensor.id == id)?;
    let scope = data
        .snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.sensors.iter().find(|candidate| candidate.id == id))
        .map(|sensor| sensor.scope.clone())
        .unwrap_or_default();
    let channel = channel(monitor, sensor, scope);
    channel.visible(data).then_some(channel)
}

pub(crate) fn by_quantity(data: &Data, quantity: Quantity, unit: PhysicalUnit) -> Vec<Channel> {
    channels(data)
        .into_iter()
        .filter(|channel| {
            channel.quantity == quantity && channel.unit == unit && channel.visible(data)
        })
        .collect()
}

/// Hide unavailable sensors while retaining their reasons in collector diagnostics.
pub(crate) fn environmental_channels(
    data: &Data,
    quantity: Quantity,
    unit: PhysicalUnit,
) -> Vec<Channel> {
    channels(data)
        .into_iter()
        .filter(|channel| {
            channel.quantity == quantity
                && channel.unit == unit
                && sensor_visible(data, &channel.monitor, &channel.sensor)
                && channel
                    .latest(data)
                    .is_some_and(|sample| sample.status != ReadingStatus::Unavailable)
                && data.snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot
                        .sensors
                        .iter()
                        .any(|sensor| sensor.id == channel.sensor)
                })
        })
        .collect()
}

/// Return the saved Energy or Thermals channel even when its latest sample is unavailable.
pub(crate) fn selected_environmental_channel(data: &Data, screen: Screen) -> Option<Channel> {
    let (quantity, unit) = match screen {
        Screen::Energy => (Quantity::Power, PhysicalUnit::Watts),
        Screen::Thermals => (Quantity::Temperature, PhysicalUnit::Celsius),
        _ => return None,
    };
    let id = selected_device(data, screen)?;
    let snapshot = data.snapshot.as_ref()?;
    channels(data).into_iter().find(|channel| {
        channel.sensor == id
            && channel.quantity == quantity
            && channel.unit == unit
            && sensor_visible(data, &channel.monitor, &channel.sensor)
            && snapshot
                .sensors
                .iter()
                .any(|sensor| sensor.id == channel.sensor)
    })
}

/// Build the full accessible description for an unavailable environmental sensor.
pub(crate) fn unavailable_detail(channel: &Channel, data: &Data) -> Option<String> {
    let sample = channel.latest(data)?;
    (sample.status == ReadingStatus::Unavailable).then(|| {
        let mut parts = vec![
            channel.device.clone(),
            channel.label.clone(),
            "不可用".into(),
        ];
        if !sample.unit.is_empty() {
            parts.push(sample.unit.clone());
        }
        if let Some(reason) = sample.reason.as_ref().filter(|reason| !reason.is_empty()) {
            parts.push(reason.clone());
        }
        if !channel.scope.is_empty() {
            parts.push(channel.scope.clone());
        }
        parts.join(" · ")
    })
}

pub(crate) fn highest_current(data: &Data, channels: &[Channel]) -> Option<Channel> {
    channels
        .iter()
        .filter_map(|channel| channel.measured(data).map(|value| (channel, value)))
        .max_by(|(a, av), (b, bv)| av.total_cmp(bv).then_with(|| b.sensor.cmp(&a.sensor)))
        .map(|(channel, _)| channel.clone())
}

pub(crate) fn devices(data: &Data, screen: Screen) -> Vec<DeviceChoice> {
    let mut result = if matches!(screen, Screen::Energy | Screen::Thermals) {
        let (quantity, unit) = if screen == Screen::Energy {
            (Quantity::Power, PhysicalUnit::Watts)
        } else {
            (Quantity::Temperature, PhysicalUnit::Celsius)
        };
        environmental_channels(data, quantity, unit)
            .into_iter()
            .map(|channel| DeviceChoice {
                id: channel.sensor,
                label: format!("{} · {}", channel.device, channel.label),
            })
            .collect::<Vec<_>>()
    } else {
        data.snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .monitors
                    .iter()
                    .filter(|monitor| {
                        matches!(
                            (screen, &monitor.kind),
                            (Screen::Gpu, MonitorKind::Gpu)
                                | (Screen::Disks, MonitorKind::Volume)
                                | (Screen::Network, MonitorKind::Network)
                        )
                    })
                    .map(|monitor| DeviceChoice {
                        id: monitor.id.clone(),
                        label: data
                            .catalog
                            .iter()
                            .find(|candidate| candidate.id == monitor.id)
                            .map(|monitor| monitor.title.clone())
                            .unwrap_or_else(|| monitor.title.clone()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    result.sort_by(|a, b| a.label.cmp(&b.label).then_with(|| a.id.cmp(&b.id)));
    result
}

/// A saved absent device never silently becomes a different device.
pub(crate) fn selected_device(data: &Data, screen: Screen) -> Option<String> {
    if let Some(id) = data.session.workspace.screens.devices.get(&screen) {
        return Some(id.clone());
    }
    let choices = devices(data, screen);
    if screen == Screen::Network
        && let Some(id) = data
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.preferred_network_monitor_id.as_ref())
        && choices.iter().any(|choice| &choice.id == id)
    {
        return Some(id.clone());
    }
    if screen == Screen::Thermals {
        return highest_current(
            data,
            &by_quantity(data, Quantity::Temperature, PhysicalUnit::Celsius),
        )
        .map(|channel| channel.sensor)
        .or_else(|| choices.first().map(|choice| choice.id.clone()));
    }
    // Prefer a device with a current measurement. Identity, not discovery order, breaks ties.
    let channels = channels(data);
    choices
        .iter()
        .find(|choice| {
            channels.iter().any(|channel| {
                (channel.monitor == choice.id || channel.sensor == choice.id)
                    && channel.measured(data).is_some()
            })
        })
        .or(choices.first())
        .map(|choice| choice.id.clone())
}

pub(crate) fn selected_channel(data: &Data, screen: Screen) -> Option<Channel> {
    if matches!(screen, Screen::Energy | Screen::Thermals) {
        let channel = selected_environmental_channel(data, screen)?;
        return channel
            .latest(data)
            .is_some_and(|sample| sample.status != ReadingStatus::Unavailable)
            .then_some(channel);
    }
    let id = selected_device(data, screen)?;
    channels(data)
        .into_iter()
        .find(|channel| channel.sensor == id && channel.visible(data))
}

pub(crate) fn cpu_cores(data: &Data) -> Vec<Channel> {
    let mut result: Vec<_> = monitor_channels(data, "cpu:host")
        .into_iter()
        .filter(|channel| crate::dashboard::core_number(&channel.sensor).is_some())
        .collect();
    result.sort_by_key(|channel| crate::dashboard::core_number(&channel.sensor));
    result
}

pub(crate) fn current_ratio(channel: &Channel, data: &Data) -> Option<f64> {
    let sample = channel.latest(data)?;
    if channel.quantity == Quantity::Capacity {
        return sample.capacity_ratio();
    }
    if channel.quantity == Quantity::Percentage {
        return sample.chart_value().map(|value| value / 100.);
    }
    let current = sample.chart_value()?;
    let samples = channel.samples(data);
    let (low, high) = system_pulse_model::chart_range(&samples);
    (high > low).then_some((current - low) / (high - low))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_collector_labels_are_localized_and_unknown_device_labels_are_preserved() {
        assert_eq!(display_sensor_title("GPU utilization"), "GPU 使用率");
        assert_eq!(display_sensor_title("CPU 7 frequency"), "CPU 7 频率");
        assert_eq!(
            display_sensor_title("PCIe controller temperature"),
            "PCIe controller temperature"
        );
    }

    #[test]
    fn zero_is_visible_and_stale_does_not_become_current() {
        let mut sample =
            Sample::measured(1, Quantity::Power, 0., None, PhysicalUnit::Watts).test_unwrap();
        assert_eq!(sample.chart_value(), Some(0.));
        assert!(compact_value(Some(&sample)).starts_with('0'));
        sample.status = ReadingStatus::Stale;
        assert!(sample.chart_value().is_none());
        assert!(compact_value(Some(&sample)).ends_with("数据过期"));
        sample.status = ReadingStatus::Unavailable;
        sample.value = None;
        assert_eq!(compact_value(Some(&sample)), "不可用");
    }

    #[test]
    fn missing_sensor_is_not_a_zero_measurement() {
        assert_eq!(compact_value(None), "等待数据");
    }
}
