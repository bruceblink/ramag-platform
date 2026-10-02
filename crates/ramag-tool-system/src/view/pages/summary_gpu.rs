//! GPU utilization card used by the Summary page.

use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, div, px,
};
use ramag_infra_system::{MonitorDescriptor, MonitorKind, SensorDescriptor, SensorKind, Unit};

use super::super::{SystemSection, SystemView, helpers};
use crate::{MonitorSnapshot, ReadingStatus, SensorSample};

/// Resolves a saved GPU before falling back to the first available device.
fn selected_gpu<'a, 'v>(
    snapshot: &'a MonitorSnapshot,
    view: &'v SystemView,
) -> (Option<&'a MonitorDescriptor>, Option<&'v str>) {
    let monitors = snapshot
        .host
        .monitors
        .iter()
        .filter(|monitor| monitor.kind == MonitorKind::Gpu)
        .collect::<Vec<_>>();
    let saved = view
        .presentation
        .selected_devices
        .get("gpu")
        .map(String::as_str);
    if let Some(saved) = saved {
        return (
            monitors.iter().copied().find(|monitor| monitor.id == saved),
            Some(saved),
        );
    }
    (monitors.first().copied(), None)
}

/// Selects a percentage summary sensor that belongs to the selected GPU.
pub(crate) fn gpu_sensor<'a>(
    snapshot: &'a MonitorSnapshot,
    monitor: Option<&MonitorDescriptor>,
) -> Option<&'a SensorDescriptor> {
    let monitor = monitor?;
    snapshot
        .host
        .sensors
        .iter()
        .find(|sensor| {
            sensor.monitor_id == monitor.id
                && sensor.id == monitor.summary_sensor_id
                && sensor.kind == SensorKind::Percentage
                && sensor.unit == Unit::Percent
        })
        .or_else(|| {
            snapshot.host.sensors.iter().find(|sensor| {
                sensor.monitor_id == monitor.id
                    && sensor.kind == SensorKind::Percentage
                    && sensor.unit == Unit::Percent
            })
        })
}

pub(crate) fn current_usage(sample: Option<&SensorSample>, collection_stale: bool) -> Option<f64> {
    if collection_stale {
        return None;
    }
    sample
        .filter(|sample| sample.status == ReadingStatus::Current)
        .and_then(|sample| sample.value)
        .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
}

pub(crate) fn usage_status(
    sample: Option<&SensorSample>,
    collection_stale: bool,
    value: Option<f64>,
) -> ReadingStatus {
    if collection_stale && sample.is_some() {
        return ReadingStatus::Stale;
    }
    match sample {
        Some(sample) if sample.status == ReadingStatus::Current && value.is_none() => {
            ReadingStatus::Failed
        }
        Some(sample) => sample.status,
        None => ReadingStatus::Unavailable,
    }
}

fn usage_detail(
    sample: Option<&SensorSample>,
    collection_stale: bool,
    status: ReadingStatus,
) -> String {
    if collection_stale && sample.is_some() {
        return "采集快照已过期".into();
    }
    sample
        .and_then(|sample| sample.reason.clone())
        .unwrap_or_else(|| {
            if sample.is_none() {
                "等待采样".into()
            } else {
                status.label().to_owned()
            }
        })
}

pub(crate) fn usage_points(
    snapshot: &MonitorSnapshot,
    descriptor: Option<&SensorDescriptor>,
    maximum_gap: f64,
) -> Vec<ramag_ui::pulse_ui::ChartPoint> {
    let Some(descriptor) = descriptor else {
        return Vec::new();
    };
    let latest_time = snapshot
        .latest(&descriptor.id)
        .map(|sample| sample.at_seconds);
    helpers::chart_points(snapshot, &descriptor.id, maximum_gap)
        .into_iter()
        .map(|mut point| {
            let invalid_usage = point
                .value
                .is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value));
            let stalled_latest = snapshot.collection_stale
                && latest_time.is_some_and(|latest| point.at_seconds >= latest);
            if invalid_usage || stalled_latest {
                point.value = None;
            }
            point
        })
        .collect()
}

/// Renders GPU utilization, history, source state, and the detailed-page action.
pub(super) fn render_activity_card(
    view: &SystemView,
    snapshot: &MonitorSnapshot,
    cx: &mut Context<SystemView>,
) -> AnyElement {
    let (monitor, saved_id) = selected_gpu(snapshot, view);
    let sensor = gpu_sensor(snapshot, monitor);
    let sample = sensor.and_then(|sensor| snapshot.latest(&sensor.id));
    let value = current_usage(sample, snapshot.collection_stale);
    let status = usage_status(sample, snapshot.collection_stale, value);
    let points = usage_points(
        snapshot,
        sensor,
        view.monitor.refresh_interval().duration().as_secs_f64() * 3.0,
    );
    let device_count = snapshot
        .host
        .monitors
        .iter()
        .filter(|candidate| candidate.kind == MonitorKind::Gpu)
        .count();
    let device_label = monitor.map_or_else(
        || {
            saved_id.map_or_else(
                || "当前平台没有发现 GPU".to_owned(),
                |id| format!("已保存设备 {id} 当前不可用"),
            )
        },
        |monitor| format!("{} · {device_count} 个设备", monitor.title),
    );
    let value_text = value.map_or_else(
        || "—".into(),
        |value| helpers::format_value(value, &Unit::Percent),
    );
    let state_text = if sensor.is_none() {
        "传感器不可用".to_owned()
    } else {
        usage_detail(sample, snapshot.collection_stale, status)
    };
    let color = cx.theme().info;
    let chart = helpers::render_chart_series(
        &[ramag_ui::pulse_ui::ChartSeries {
            points: &points,
            color,
        }],
        100.0,
        &Unit::Percent,
        px(116.0),
        cx,
    );
    let mut legend = h_flex()
        .w_full()
        .items_baseline()
        .gap(px(5.0))
        .child(div().text_xs().text_color(color).child("利用率"))
        .child(
            div()
                .debug_selector(|| "system-summary-gpu-value".into())
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(value_text),
        );
    if status != ReadingStatus::Current {
        legend = legend.child(
            div()
                .debug_selector(|| "system-summary-gpu-status".into())
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(state_text),
        );
    }
    let view_entity = cx.entity().clone();
    let action = ramag_ui::clickable_button("system-summary-open-gpu")
        .debug_selector(|| "system-summary-open-gpu".into())
        .xsmall()
        .ghost()
        .icon(Icon::new(IconName::ArrowRight))
        .tooltip("打开 GPU 页面")
        .on_click(move |_, _, app| {
            view_entity.update(app, |view, cx| view.select_section(SystemSection::Gpu, cx))
        });
    let card_selector = monitor.map_or_else(
        || "system-summary-gpu-missing".to_owned(),
        |monitor| format!("system-summary-gpu-{}", selector_id(&monitor.id)),
    );
    ramag_ui::pulse_ui::pulse_panel(cx)
        .flex_1()
        .min_w(px(280.0))
        .debug_selector(move || card_selector.clone())
        .border_color(color.opacity(0.45))
        .bg(color.opacity(0.055))
        .child(
            v_flex()
                .gap(px(8.0))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .child("GPU"),
                        )
                        .child(action),
                )
                .child(
                    div()
                        .debug_selector(|| "system-summary-gpu-device".into())
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(device_label),
                )
                .child(chart)
                .child(legend),
        )
        .into_any_element()
}

fn selector_id(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "../summary_gpu_tests.rs"]
mod tests;
