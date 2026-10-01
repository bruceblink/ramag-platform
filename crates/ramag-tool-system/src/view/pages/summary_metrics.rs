//! CPU and memory composition helpers for the Summary page.

use gpui_kit::StatefulInteractiveElement as _;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, div, px,
};
use ramag_infra_system::{MonitorKind, SensorDescriptor, SensorKind, Unit};

use super::super::{SystemView, helpers};
use crate::{MonitorSnapshot, ReadingStatus, SensorSample};

#[derive(Clone)]
pub(super) struct CpuMetrics<'a> {
    pub(super) usage: Option<&'a SensorDescriptor>,
    pub(super) clock: Option<&'a SensorDescriptor>,
    pub(super) temperature: Option<&'a SensorDescriptor>,
    pub(super) gpu: Option<&'a SensorDescriptor>,
    pub(super) cores: Vec<&'a SensorDescriptor>,
}

/// Resolves Summary meters from existing host descriptors; absent metrics stay absent.
pub(super) fn cpu_metrics<'a>(
    snapshot: &'a MonitorSnapshot,
    selected_gpu_id: Option<&str>,
    usage: Option<&'a SensorDescriptor>,
) -> CpuMetrics<'a> {
    let cpu_monitor_id = snapshot
        .host
        .monitors
        .iter()
        .find(|monitor| monitor.kind == MonitorKind::Cpu)
        .map(|monitor| monitor.id.as_str());
    let selected_gpu = snapshot
        .host
        .monitors
        .iter()
        .filter(|monitor| monitor.kind == MonitorKind::Gpu)
        .find(|monitor| selected_gpu_id.is_none_or(|selected| selected == monitor.id));
    let gpu = selected_gpu.and_then(|monitor| {
        snapshot.host.sensors.iter().find(|sensor| {
            sensor.monitor_id == monitor.id && sensor.id == monitor.summary_sensor_id
        })
    });
    let mut temperature = None;
    let mut temperature_max = f64::NEG_INFINITY;
    let mut clock = None;
    let mut clock_max = f64::NEG_INFINITY;
    let mut cores = Vec::new();
    for sensor in &snapshot.host.sensors {
        let sample = snapshot.latest(&sensor.id);
        let current = (!snapshot.collection_stale)
            .then(|| {
                sample
                    .filter(|item| item.status == ReadingStatus::Current)
                    .and_then(|item| item.value)
            })
            .flatten()
            .filter(|value| value.is_finite());
        match &sensor.kind {
            SensorKind::Temperature
                if sensor.unit == Unit::Celsius
                    && Some(sensor.monitor_id.as_str()) == cpu_monitor_id =>
            {
                if let Some(value) = current.filter(|value| *value > temperature_max) {
                    temperature_max = value;
                    temperature = Some(sensor);
                }
            }
            SensorKind::Frequency
                if sensor.unit == Unit::Hertz
                    && Some(sensor.monitor_id.as_str()) == cpu_monitor_id =>
            {
                if let Some(value) = current.filter(|value| *value > clock_max) {
                    clock_max = value;
                    clock = Some(sensor);
                }
            }
            SensorKind::Percentage
                if sensor.unit == Unit::Percent
                    && Some(sensor.monitor_id.as_str()) == cpu_monitor_id
                    && sensor.id.contains("/core-") =>
            {
                cores.push(sensor);
            }
            _ => {}
        }
    }
    CpuMetrics {
        usage,
        clock,
        temperature,
        gpu,
        cores,
    }
}

/// Renders the compact meter strip and its per-logical-CPU activity tiles.
pub(super) fn render_cpu_meters(
    snapshot: &MonitorSnapshot,
    metrics: &CpuMetrics<'_>,
    cx: &Context<SystemView>,
) -> AnyElement {
    let mut meters = h_flex().w_full().min_w_0().flex_wrap().gap(px(8.0));
    for (selector, title, descriptor, color) in [
        (
            "system-summary-cpu-meter",
            "CPU",
            metrics.usage,
            cx.theme().success,
        ),
        (
            "system-summary-clock-meter",
            "Clock",
            metrics.clock,
            cx.theme().success,
        ),
        (
            "system-summary-temperature-meter",
            "Temp",
            metrics.temperature,
            cx.theme().warning,
        ),
        (
            "system-summary-gpu-meter",
            "GPU",
            metrics.gpu,
            cx.theme().info,
        ),
    ] {
        meters = meters.child(render_meter(
            snapshot, selector, title, descriptor, color, cx,
        ));
    }
    let mut panel = v_flex()
        .debug_selector(|| "system-summary-meters".into())
        .flex_1()
        .min_w(px(190.0))
        .gap(px(10.0))
        .child(meters);
    let mut cores = v_flex()
        .debug_selector(|| "system-summary-cpu-cores".into())
        .w_full()
        .gap(px(5.0))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Logical CPU"),
        );
    if metrics.cores.is_empty() {
        cores = cores.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("暂无单核读数"),
        );
    } else {
        let mut tiles = h_flex().w_full().min_w_0().flex_wrap().gap(px(3.0));
        for core in metrics.cores.iter().take(32) {
            let value = live_sensor_value(snapshot, core);
            let label = value.map_or_else(|| "—".into(), |value| format!("{value:.0}%"));
            tiles = tiles.child(
                div()
                    .flex_1()
                    .min_w(px(34.0))
                    .h(px(24.0))
                    .rounded(px(3.0))
                    .bg(cx.theme().success.opacity(value.map_or(0.12_f32, |value| {
                        0.18 + value.clamp(0.0, 100.0) as f32 / 125.0
                    })))
                    .text_xs()
                    .text_color(cx.theme().foreground)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(label),
            );
        }
        cores = cores.child(tiles);
    }
    panel = panel.child(cores);
    ramag_ui::pulse_ui::pulse_panel(cx)
        .flex_1()
        .min_w(px(210.0))
        .border_color(cx.theme().success.opacity(0.35))
        .bg(cx.theme().success.opacity(0.045))
        .child(panel)
        .into_any_element()
}

fn render_meter(
    snapshot: &MonitorSnapshot,
    selector: &'static str,
    title: &'static str,
    descriptor: Option<&SensorDescriptor>,
    color: gpui_kit::Hsla,
    cx: &Context<SystemView>,
) -> AnyElement {
    let sample = descriptor.and_then(|sensor| snapshot.latest(&sensor.id));
    let value = descriptor.and_then(|sensor| live_sensor_value(snapshot, sensor));
    let reading = match (descriptor, value) {
        (Some(sensor), Some(value)) => helpers::format_value(value, &sensor.unit),
        (Some(sensor), None) => sensor_reason(snapshot, sensor, sample),
        (None, _) => "不可用".into(),
    };
    let scale = descriptor
        .and_then(|sensor| {
            sensor
                .scale
                .filter(|scale| scale.is_finite() && *scale > 0.0)
        })
        .unwrap_or_else(|| {
            descriptor.map_or(100.0, |sensor| match sensor.unit {
                Unit::Percent | Unit::Celsius => 100.0,
                Unit::Hertz => 5_000_000_000.0,
                _ => 1.0,
            })
        });
    let fill = value.map_or(0.0, |value| (value / scale).clamp(0.0, 1.0) as f32);
    ramag_ui::pulse_ui::pulse_panel(cx)
        .debug_selector(|| selector.into())
        .flex_1()
        .min_w(px(72.0))
        .p(px(8.0))
        .child(
            v_flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(title),
                )
                .child(
                    v_flex()
                        .justify_end()
                        .w(px(20.0))
                        .h(px(70.0))
                        .rounded(px(3.0))
                        .bg(cx.theme().muted.opacity(0.25))
                        .child(div().w_full().h(px(70.0 * fill)).rounded(px(3.0)).bg(color)),
                )
                .child(div().text_xs().text_color(color).child(reading)),
        )
        .into_any_element()
}

pub(super) fn render_memory_panel(
    descriptor: Option<&SensorDescriptor>,
    snapshot: &MonitorSnapshot,
    maximum_gap: f64,
    cx: &Context<SystemView>,
) -> AnyElement {
    let sample = descriptor.and_then(|sensor| snapshot.latest(&sensor.id));
    let value = descriptor.and_then(|sensor| live_sensor_value(snapshot, sensor));
    let total = sample
        .filter(|sample| !snapshot.collection_stale && sample.status == ReadingStatus::Current)
        .and_then(|sample| sample.total)
        .filter(|total| total.is_finite() && *total >= 0.0);
    let large_value = match (descriptor, value, total) {
        (Some(sensor), Some(value), Some(total)) => format!(
            "{} / {}",
            helpers::format_value(value, &sensor.unit),
            helpers::format_value(total, &sensor.unit)
        ),
        (Some(sensor), Some(value), None) => helpers::format_value(value, &sensor.unit),
        (Some(sensor), None, _) => sensor_reason(snapshot, sensor, sample),
        (None, _, _) => "内存用量不可用".into(),
    };
    let chart = descriptor.map(|sensor| {
        let points = helpers::chart_points(snapshot, &sensor.id, maximum_gap);
        helpers::render_chart(
            &points,
            chart_max(sensor, snapshot),
            &sensor.unit,
            px(116.0),
            cx.theme().magenta,
            cx,
        )
    });
    let mut panel = ramag_ui::pulse_ui::pulse_panel(cx)
        .debug_selector(|| "system-summary-memory".into())
        .w_full()
        .min_w_0()
        .border_color(cx.theme().magenta.opacity(0.38))
        .bg(cx.theme().magenta.opacity(0.045))
        .gap(px(8.0))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child("Memory utilization"),
                )
                .child(
                    div()
                        .text_lg()
                        .text_color(cx.theme().magenta)
                        .child(large_value),
                ),
        );
    panel = if let Some(chart) = chart {
        panel.child(chart)
    } else {
        panel.child(ramag_ui::pulse_ui::pulse_status_notice(
            ramag_ui::pulse_ui::PulseStatus::Unavailable,
            "暂无可用的内存趋势传感器",
            cx,
        ))
    };
    let mut details = h_flex().w_full().min_w_0().flex_wrap().gap(px(14.0));
    for (selector, label, suffix) in [
        ("system-summary-memory-available", "Available", "available"),
        ("system-summary-memory-cache", "Cache", "cache"),
        ("system-summary-memory-swap", "Swap used", "swap"),
    ] {
        details = details.child(memory_detail(snapshot, selector, label, suffix, cx));
    }
    panel.child(details).into_any_element()
}

fn memory_detail(
    snapshot: &MonitorSnapshot,
    selector: &'static str,
    label: &'static str,
    suffix: &'static str,
    cx: &Context<SystemView>,
) -> AnyElement {
    let descriptor = snapshot.host.sensors.iter().find(|sensor| {
        sensor.monitor_id == "memory:host" && sensor.id.ends_with(&format!("/{suffix}"))
    });
    let value = descriptor.and_then(|sensor| live_sensor_value(snapshot, sensor));
    let sample = descriptor.and_then(|sensor| snapshot.latest(&sensor.id));
    let (text, detail) = match (descriptor, value) {
        (Some(sensor), Some(value)) => (helpers::format_value(value, &sensor.unit), None),
        (Some(sensor), None) => {
            // Keep the compact row scannable while retaining the collector reason in a tooltip.
            let detail = sensor_reason(snapshot, sensor, sample);
            let status = if snapshot.collection_stale {
                "已过期"
            } else {
                sample.map_or("不可用", |sample| sample.status.label())
            };
            (status.to_owned(), Some(detail))
        }
        (None, _) => ("不可用".into(), None),
    };
    let mut value_view = div()
        .id(format!("system-summary-memory-detail-{suffix}"))
        .text_xs()
        .text_color(cx.theme().foreground)
        .max_w(px(220.0))
        .overflow_hidden()
        .text_ellipsis()
        .child(text);
    if let Some(detail) = detail {
        value_view =
            value_view.tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx));
    }
    h_flex()
        .debug_selector(|| selector.into())
        .flex_1()
        .min_w(px(180.0))
        .min_w_0()
        .justify_between()
        .gap(px(8.0))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(value_view)
        .into_any_element()
}

fn live_sensor_value(snapshot: &MonitorSnapshot, sensor: &SensorDescriptor) -> Option<f64> {
    if snapshot.collection_stale {
        return None;
    }
    snapshot
        .latest(&sensor.id)
        .filter(|sample| sample.status == ReadingStatus::Current)
        .and_then(|sample| sample.value)
        .filter(|value| value.is_finite())
}

fn sensor_reason(
    snapshot: &MonitorSnapshot,
    sensor: &SensorDescriptor,
    sample: Option<&SensorSample>,
) -> String {
    if snapshot.collection_stale {
        return sample
            .and_then(|sample| sample.reason.as_deref())
            .map_or_else(
                || "采集快照已过期".into(),
                |reason| format!("采集快照已过期：{reason}"),
            );
    }
    sample.map_or_else(
        || format!("{}：等待采样", sensor.title),
        |sample| {
            sample
                .reason
                .clone()
                .unwrap_or_else(|| sample.status.label().to_owned())
        },
    )
}

pub(super) fn primary_sensor(
    snapshot: &MonitorSnapshot,
    kind: MonitorKind,
) -> Option<&SensorDescriptor> {
    let monitor = snapshot
        .host
        .monitors
        .iter()
        .find(|monitor| monitor.kind == kind)?;
    snapshot
        .host
        .sensors
        .iter()
        .find(|sensor| sensor.id == monitor.summary_sensor_id)
        .or_else(|| {
            snapshot
                .host
                .sensors
                .iter()
                .find(|sensor| sensor.monitor_id == monitor.id)
        })
}

pub(super) fn primary_chart(
    title: &'static str,
    descriptor: Option<&SensorDescriptor>,
    snapshot: &MonitorSnapshot,
    maximum_gap: f64,
    line: gpui_kit::Hsla,
    cx: &Context<SystemView>,
) -> AnyElement {
    let Some(descriptor) = descriptor else {
        return ramag_ui::pulse_ui::pulse_panel(cx)
            .flex_1()
            .min_w(px(280.0))
            .child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                format!("{title}暂无可用传感器"),
                cx,
            ))
            .into_any_element();
    };
    let points = helpers::chart_points(snapshot, &descriptor.id, maximum_gap);
    ramag_ui::pulse_ui::pulse_panel(cx)
        .flex_1()
        .min_w(px(280.0))
        .gap(px(8.0))
        .child(
            v_flex()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{} · {}",
                            descriptor.title,
                            helpers::unit_label(&descriptor.unit)
                        )),
                )
                .child(helpers::render_chart(
                    &points,
                    chart_max(descriptor, snapshot),
                    &descriptor.unit,
                    px(132.0),
                    line,
                    cx,
                )),
        )
        .into_any_element()
}

pub(super) fn chart_max(descriptor: &SensorDescriptor, snapshot: &MonitorSnapshot) -> f64 {
    descriptor
        .scale
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .or_else(|| {
            snapshot
                .histories
                .get(&descriptor.id)
                .into_iter()
                .flatten()
                .filter_map(|sample| sample.chart_value())
                .filter(|value| value.is_finite())
                .max_by(f64::total_cmp)
        })
        .unwrap_or(match descriptor.unit {
            Unit::Percent | Unit::Celsius => 100.0,
            Unit::Hertz => 5_000_000_000.0,
            _ => 1.0,
        })
        .max(1.0)
}

#[cfg(test)]
#[path = "../summary_memory_tests.rs"]
mod memory_tests;
