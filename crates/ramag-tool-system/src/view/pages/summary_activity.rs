//! Selected disk and network activity summaries reuse collector-owned sensors.

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px,
};
use ramag_infra_system::{MonitorDescriptor, MonitorKind, SensorDescriptor, SensorKind, Unit};

use super::super::{SystemSection, SystemView, helpers};
use super::summary_gpu;
use crate::{MonitorSnapshot, ReadingStatus, SensorSample};

#[derive(Clone, Copy)]
struct ActivityKind {
    section: SystemSection,
    title: &'static str,
    first_suffix: &'static str,
    second_suffix: &'static str,
}

impl ActivityKind {
    const DISK: Self = Self {
        section: SystemSection::Disks,
        title: "磁盘活动",
        first_suffix: "read",
        second_suffix: "write",
    };
    const NETWORK: Self = Self {
        section: SystemSection::Network,
        title: "网络活动",
        first_suffix: "rx",
        second_suffix: "tx",
    };
}

/// Resolves the saved device first, then the collector's network default, then inventory order.
fn selected_monitor<'a, 'v>(
    snapshot: &'a MonitorSnapshot,
    view: &'v SystemView,
    kind: ActivityKind,
) -> (Option<&'a MonitorDescriptor>, Option<&'v str>) {
    // A saved but missing device stays visible as unavailable instead of silently changing sources.
    let monitor_kind = match kind.section {
        SystemSection::Disks => MonitorKind::Volume,
        SystemSection::Network => MonitorKind::Network,
        _ => unreachable!("activity cards only represent devices with byte-rate sensors"),
    };
    let monitors = snapshot
        .host
        .monitors
        .iter()
        .filter(|monitor| monitor.kind == monitor_kind)
        .collect::<Vec<_>>();
    let saved = view
        .presentation
        .selected_devices
        .get(kind.section.id())
        .map(String::as_str);
    if let Some(saved) = saved {
        return (
            monitors.iter().copied().find(|monitor| monitor.id == saved),
            Some(saved),
        );
    }
    let preferred = (kind.section == SystemSection::Network)
        .then_some(snapshot.host.preferred_network_monitor_id.as_deref())
        .flatten()
        .and_then(|id| monitors.iter().copied().find(|monitor| monitor.id == id));
    (preferred.or_else(|| monitors.first().copied()), None)
}

fn activity_sensor<'a>(
    snapshot: &'a MonitorSnapshot,
    monitor: Option<&MonitorDescriptor>,
    suffix: &str,
) -> Option<&'a SensorDescriptor> {
    // Rate IDs must belong to the selected device so similarly named sensors cannot leak across devices.
    let monitor = monitor?;
    snapshot.host.sensors.iter().find(|sensor| {
        sensor.monitor_id == monitor.id
            && sensor.id == format!("{}/{suffix}", monitor.id)
            && sensor.kind == SensorKind::Rate
            && sensor.unit == Unit::BytesPerSecond
    })
}

fn capacity_detail(
    snapshot: &MonitorSnapshot,
    monitor: Option<&MonitorDescriptor>,
) -> Option<String> {
    // The volume summary sensor owns its capacity and total; never infer these from other disks.
    let monitor = monitor?;
    let sensor =
        snapshot.host.sensors.iter().find(|sensor| {
            sensor.id == monitor.summary_sensor_id && sensor.monitor_id == monitor.id
        })?;
    if sensor.kind != SensorKind::Capacity || sensor.unit != Unit::Bytes {
        return Some("容量传感器单位不匹配".into());
    }
    let sample = snapshot.latest(&sensor.id);
    if !snapshot.collection_stale
        && let Some(sample) = sample.filter(|sample| sample.status == ReadingStatus::Current)
        && let (Some(used), Some(total)) = (sample.value, sample.total)
        && used.is_finite()
        && used >= 0.0
        && total.is_finite()
        && total >= 0.0
    {
        return Some(format!(
            "已用 {} / {}",
            helpers::format_value(used, &Unit::Bytes),
            helpers::format_value(total, &Unit::Bytes)
        ));
    }
    let status = match sample {
        Some(_) if snapshot.collection_stale => ReadingStatus::Stale,
        Some(sample) if sample.status == ReadingStatus::Current => ReadingStatus::Failed,
        Some(sample) => sample.status,
        None => return Some("容量等待采样".into()),
    };
    Some(format!(
        "容量{}",
        sample
            .and_then(|sample| sample.reason.as_deref())
            .unwrap_or_else(|| status.label())
    ))
}

/// Prevents a retained or invalid reading from appearing as a live transfer rate.
fn current_rate(sample: Option<&SensorSample>, collection_stale: bool) -> Option<f64> {
    if collection_stale {
        return None;
    }
    sample
        .filter(|sample| sample.status == ReadingStatus::Current)
        .and_then(|sample| sample.value)
        .filter(|value| value.is_finite() && *value >= 0.0)
}

fn rate_status(
    sample: Option<&SensorSample>,
    collection_stale: bool,
    value: Option<f64>,
) -> ReadingStatus {
    // Keep an invalid Current sample distinct from warming, unsupported, failed, and stale states.
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

fn rate_detail(
    sample: Option<&SensorSample>,
    collection_stale: bool,
    status: ReadingStatus,
) -> String {
    // Show the collector's reason when available, with a useful fallback for every state.
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

fn rate_source_label(sensor: Option<&SensorDescriptor>) -> String {
    // Name the exact backend and measurement scope in the value tooltip.
    sensor.map_or_else(
        || "传感器不可用".to_owned(),
        |sensor| {
            [
                sensor.title.as_str(),
                sensor.source.as_str(),
                sensor.scope.as_str(),
            ]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
        },
    )
}

fn rate_points(
    snapshot: &MonitorSnapshot,
    descriptor: Option<&SensorDescriptor>,
    maximum_gap: f64,
) -> Vec<ramag_ui::pulse_ui::ChartPoint> {
    // Reuse source times and encode invalid, negative, or stalled latest rates as gaps.
    let Some(descriptor) = descriptor else {
        return Vec::new();
    };
    let latest_time = snapshot
        .latest(&descriptor.id)
        .map(|sample| sample.at_seconds);
    helpers::chart_points(snapshot, &descriptor.id, maximum_gap)
        .into_iter()
        .map(|mut point| {
            let invalid_rate = point
                .value
                .is_some_and(|value| !value.is_finite() || value < 0.0);
            let stalled_latest = snapshot.collection_stale
                && latest_time.is_some_and(|latest| point.at_seconds >= latest);
            if invalid_rate || stalled_latest {
                point.value = None;
            }
            point
        })
        .collect()
}

fn shared_rate_maximum(
    snapshot: &MonitorSnapshot,
    first: Option<&SensorDescriptor>,
    second: Option<&SensorDescriptor>,
) -> f64 {
    // Both directions share one range, using measured rates only and never capacity totals.
    let observed = [first, second]
        .into_iter()
        .flatten()
        .filter_map(|sensor| snapshot.histories.get(&sensor.id))
        .flatten()
        .filter_map(SensorSample::chart_value)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .reduce(f64::max);
    observed.map_or(1.0, |value| {
        let padded = value * 1.15;
        if padded.is_finite() {
            padded.max(1.0)
        } else {
            value
        }
    })
}

/// Renders the selected disk and network rates while keeping each card's page navigation.
pub(super) fn render_activity_cards(
    view: &SystemView,
    snapshot: &MonitorSnapshot,
    cx: &mut Context<SystemView>,
) -> AnyElement {
    let mut cards = h_flex().w_full().min_w_0().flex_wrap().gap(px(8.0));
    for kind in [ActivityKind::DISK, ActivityKind::NETWORK] {
        cards = cards.child(activity_card(view, snapshot, kind, cx));
    }
    cards = cards.child(summary_gpu::render_activity_card(view, snapshot, cx));
    cards.into_any_element()
}

fn activity_card(
    view: &SystemView,
    snapshot: &MonitorSnapshot,
    kind: ActivityKind,
    cx: &mut Context<SystemView>,
) -> AnyElement {
    // Resolve both directions from one selected device and keep status separate from plotted values.
    let (monitor, saved_id) = selected_monitor(snapshot, view, kind);
    let first = activity_sensor(snapshot, monitor, kind.first_suffix);
    let second = activity_sensor(snapshot, monitor, kind.second_suffix);
    let first_sample = first.and_then(|sensor| snapshot.latest(&sensor.id));
    let second_sample = second.and_then(|sensor| snapshot.latest(&sensor.id));
    let first_value = current_rate(first_sample, snapshot.collection_stale);
    let second_value = current_rate(second_sample, snapshot.collection_stale);
    let first_status = rate_status(first_sample, snapshot.collection_stale, first_value);
    let second_status = rate_status(second_sample, snapshot.collection_stale, second_value);
    let maximum_gap = view.monitor.refresh_interval().duration().as_secs_f64() * 3.0;
    let first_points = rate_points(snapshot, first, maximum_gap);
    let second_points = rate_points(snapshot, second, maximum_gap);
    let maximum = shared_rate_maximum(snapshot, first, second);
    let (primary_color, secondary_color) = if kind.section == SystemSection::Disks {
        (cx.theme().success, cx.theme().warning)
    } else {
        (cx.theme().accent, cx.theme().warning)
    };
    let series = [
        ramag_ui::pulse_ui::ChartSeries {
            points: &first_points,
            color: primary_color,
        },
        ramag_ui::pulse_ui::ChartSeries {
            points: &second_points,
            color: secondary_color,
        },
    ];
    let monitor_kind = if kind.section == SystemSection::Disks {
        MonitorKind::Volume
    } else {
        MonitorKind::Network
    };
    let device_count = snapshot
        .host
        .monitors
        .iter()
        .filter(|candidate| candidate.kind == monitor_kind)
        .count();
    let device_label = monitor.map_or_else(
        || {
            saved_id.map_or_else(
                || "当前平台没有发现此类设备".to_owned(),
                |id| format!("已保存设备 {id} 当前不可用"),
            )
        },
        |monitor| format!("{} · {device_count} 个设备", monitor.title),
    );
    let line_labels = [
        (
            kind.first_suffix,
            first,
            first_sample,
            first_value,
            first_status,
        ),
        (
            kind.second_suffix,
            second,
            second_sample,
            second_value,
            second_status,
        ),
    ];
    let mut legend = h_flex()
        .w_full()
        .flex_wrap()
        .justify_between()
        .gap(px(12.0));
    for (index, (_, sensor, sample, value, status)) in line_labels.into_iter().enumerate() {
        let (label, color) = if index == 0 {
            (kind.first_suffix, primary_color)
        } else {
            (kind.second_suffix, secondary_color)
        };
        let value_text = value.map_or_else(
            || "—".into(),
            |value| helpers::format_value(value, &Unit::BytesPerSecond),
        );
        let state_text = if sensor.is_none() {
            "传感器不可用".to_owned()
        } else {
            rate_detail(sample, snapshot.collection_stale, status)
        };
        let source_text = rate_source_label(sensor);
        let legend_label = if kind.section == SystemSection::Network {
            if index == 0 { "接收" } else { "发送" }
        } else if index == 0 {
            "读取"
        } else {
            "写入"
        };
        let mut rate = h_flex()
            .id(format!(
                "system-summary-activity-{}-{label}",
                kind.section.id()
            ))
            .debug_selector(|| format!("system-summary-activity-{}-{label}", kind.section.id()))
            .items_baseline()
            .gap(px(5.0))
            .tooltip({
                let source_text = source_text.clone();
                move |window, cx| Tooltip::new(source_text.clone()).build(window, cx)
            })
            .child(div().text_xs().text_color(color).child(legend_label))
            .child(
                div()
                    .debug_selector(|| {
                        format!(
                            "system-summary-activity-{}-{label}-value",
                            kind.section.id()
                        )
                    })
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(value_text),
            );
        if status != ReadingStatus::Current {
            rate = rate.child(
                div()
                    .debug_selector(|| {
                        format!(
                            "system-summary-activity-{}-{label}-status",
                            kind.section.id()
                        )
                    })
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(state_text),
            );
        }
        legend = legend.child(rate);
    }
    let capacity = (kind.section == SystemSection::Disks)
        .then(|| capacity_detail(snapshot, monitor))
        .flatten();
    let chart =
        helpers::render_chart_series(&series, maximum, &Unit::BytesPerSecond, px(116.0), cx);
    let view_entity = cx.entity().clone();
    let section = kind.section;
    let action = ramag_ui::clickable_button(format!("system-summary-open-{}", section.id()))
        .debug_selector(|| format!("system-summary-open-{}", section.id()))
        .xsmall()
        .ghost()
        .icon(Icon::new(IconName::ArrowRight))
        .tooltip(format!("打开{}页面", kind.title))
        .on_click(move |_, _, app| {
            view_entity.update(app, |view, cx| view.select_section(section, cx))
        });
    ramag_ui::pulse_ui::pulse_panel(cx)
        .debug_selector(|| {
            let suffix = monitor.map_or("missing".to_owned(), |monitor| selector_id(&monitor.id));
            format!("system-summary-activity-{}-{suffix}", kind.section.id())
        })
        .flex_1()
        .min_w(px(280.0))
        .border_color(primary_color.opacity(0.45))
        .bg(primary_color.opacity(0.055))
        .gap(px(8.0))
        .child(
            v_flex()
                .gap(px(3.0))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .child(kind.title),
                        )
                        .child(action),
                )
                .child(
                    div()
                        .debug_selector(|| {
                            format!("system-summary-activity-{}-device", kind.section.id())
                        })
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .whitespace_normal()
                        .child(device_label),
                )
                .when_some(capacity, |view, detail| {
                    view.child(
                        div()
                            .debug_selector(|| "system-summary-activity-disks-capacity".into())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(detail),
                    )
                }),
        )
        .child(
            chart.debug_selector(|| format!("system-summary-activity-{}-chart", kind.section.id())),
        )
        .child(legend)
        .into_any_element()
}

#[cfg(test)]
#[path = "../summary_activity_tests.rs"]
pub(crate) mod tests;

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
