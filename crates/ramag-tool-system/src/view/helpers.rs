//! Small presentation helpers shared by the monitor's focused page modules.

use gpui_kit::IntoElement;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{InteractiveElement, ParentElement, Styled, div, px};
use ramag_infra_system::{SensorDescriptor, Unit};

use super::{Notice, SystemSection};
use crate::{ReadingStatus, TerminateResult};

const CHART_AXIS_WIDTH: f32 = 64.0;
const CHART_AXIS_GAP: f32 = 6.0;

pub(super) fn notice_for_termination(result: TerminateResult) -> Notice {
    match result {
        TerminateResult::RefusedSelf { pid } => Notice {
            message: format!("拒绝强制退出当前 Ramag 进程（PID {pid}）"),
            error: true,
        },
        TerminateResult::Missing { pid } => Notice {
            message: format!("进程 {pid} 已不存在"),
            error: true,
        },
        TerminateResult::Changed {
            pid,
            expected_name,
            actual_name,
        } => Notice {
            message: format!("进程 {pid} 已变化（{expected_name} → {actual_name}），未执行操作"),
            error: true,
        },
        TerminateResult::ChangedIdentity {
            pid,
            expected_start_time,
            actual_start_time,
        } => Notice {
            message: format!(
                "进程 {pid} 启动身份已变化（{expected_start_time} → {actual_start_time}），未执行操作"
            ),
            error: true,
        },
        TerminateResult::Sent { pid, name } => Notice {
            message: format!("已向 {name}（PID {pid}）发送强制退出请求"),
            error: false,
        },
        TerminateResult::Failed { pid, name, reason } => Notice {
            message: format!("无法强制退出 {name}（PID {pid}）：{reason}"),
            error: true,
        },
    }
}

pub(super) fn status(value: ReadingStatus) -> ramag_ui::pulse_ui::PulseStatus {
    match value {
        ReadingStatus::Current => ramag_ui::pulse_ui::PulseStatus::Current,
        ReadingStatus::WarmingUp => ramag_ui::pulse_ui::PulseStatus::Warming,
        ReadingStatus::Unavailable => ramag_ui::pulse_ui::PulseStatus::Unavailable,
        ReadingStatus::Failed => ramag_ui::pulse_ui::PulseStatus::Failed,
        ReadingStatus::Stale => ramag_ui::pulse_ui::PulseStatus::Stale,
    }
}

pub(super) fn unit_label(unit: &Unit) -> &'static str {
    match unit {
        Unit::Percent => "%",
        Unit::Bytes => "B",
        Unit::BytesPerSecond => "B/s",
        Unit::Celsius => "°C",
        Unit::Hertz => "Hz",
        Unit::Watts => "W",
        Unit::Rpm => "rpm",
        Unit::Count => "count",
        Unit::CountPerSecond => "count/s",
        Unit::Milliseconds => "ms",
        Unit::Seconds => "s",
        Unit::Load => "load",
    }
}

pub(super) fn format_value(value: f64, unit: &Unit) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    if matches!(unit, Unit::Bytes | Unit::BytesPerSecond) {
        let mut scaled = value;
        let mut prefix = "";
        for candidate in ["", "Ki", "Mi", "Gi", "Ti"] {
            prefix = candidate;
            if scaled.abs() < 1024.0 || candidate == "Ti" {
                break;
            }
            scaled /= 1024.0;
        }
        format!("{scaled:.1} {prefix}{}", unit_label(unit))
    } else if matches!(unit, Unit::Hertz) {
        let mut scaled = value;
        let mut prefix = "";
        for candidate in ["", "k", "M", "G", "T"] {
            prefix = candidate;
            if scaled.abs() < 1000.0 || candidate == "T" {
                break;
            }
            scaled /= 1000.0;
        }
        format!("{scaled:.2} {prefix}Hz")
    } else {
        format!("{value:.1} {}", unit_label(unit))
    }
}

pub(super) fn page_title(
    section: SystemSection,
    subtitle: &'static str,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    ramag_ui::pulse_ui::pulse_page_title(section.title(), Some(subtitle), cx)
}

pub(super) fn sensor_title(sensor: &SensorDescriptor) -> String {
    sensor.title.clone()
}

pub(super) fn diagnostic_text(snapshot: &crate::MonitorSnapshot) -> Option<String> {
    snapshot.collection_error.clone().or_else(|| {
        snapshot
            .host
            .diagnostics
            .iter()
            .find(|item| item.availability == ramag_infra_system::Availability::Failed)
            .map(|item| format!("{}：{}", item.backend, item.reason))
    })
}

/// Preserves collector timestamps and makes delayed sampling visible as a chart gap.
pub(super) fn chart_points(
    snapshot: &crate::MonitorSnapshot,
    sensor_id: &str,
    maximum_gap_seconds: f64,
) -> Vec<ramag_ui::pulse_ui::ChartPoint> {
    let Some(samples) = snapshot.histories.get(sensor_id) else {
        return Vec::new();
    };
    let mut points = Vec::with_capacity(samples.len().saturating_mul(2));
    let mut previous_time = None;
    for sample in samples {
        if let Some(previous) = previous_time
            && sample.at_seconds - previous > maximum_gap_seconds
        {
            points.push(ramag_ui::pulse_ui::ChartPoint {
                at_seconds: previous + (sample.at_seconds - previous) / 2.0,
                value: None,
            });
        }
        points.push(ramag_ui::pulse_ui::ChartPoint {
            at_seconds: sample.at_seconds,
            value: sample.chart_value(),
        });
        previous_time = Some(sample.at_seconds);
    }
    points
}

/// Places left-aligned physical bounds beside the plot and keeps its time labels on the same edge.
/// A bounded axis column gives the plot the remaining width; long units wrap within that column.
pub(super) fn render_chart(
    points: &[ramag_ui::pulse_ui::ChartPoint],
    maximum: f64,
    unit: &Unit,
    height: gpui_kit::Pixels,
    line: gpui_kit::Hsla,
    cx: &gpui_kit::App,
) -> gpui_kit::Div {
    render_chart_series(
        &[ramag_ui::pulse_ui::ChartSeries {
            points,
            color: line,
        }],
        maximum,
        unit,
        height,
        cx,
    )
}

/// Gives related physical series one scale and elapsed-time axis, without summing their values.
/// The time labels cover the same bounded samples that the shared chart copies for painting.
pub(super) fn render_chart_series(
    series: &[ramag_ui::pulse_ui::ChartSeries<'_>],
    maximum: f64,
    unit: &Unit,
    height: gpui_kit::Pixels,
    cx: &gpui_kit::App,
) -> gpui_kit::Div {
    let times = series
        .iter()
        .take(8)
        .flat_map(|series| series.points.iter().rev().take(120))
        .map(|point| point.at_seconds)
        .filter(|time| time.is_finite());
    let range = times.fold(None, |range, time| match range {
        Some((first, last)) => Some((f64::min(first, time), f64::max(last, time))),
        None => Some((time, time)),
    });
    let duration = range.map_or(0.0, |(first, last)| (last - first).max(0.0));
    let maximum = chart_axis_maximum(maximum);
    let theme = cx.theme();
    let mut axis = v_flex()
        .debug_selector(|| "system-chart-axis".into())
        .w(px(CHART_AXIS_WIDTH))
        .flex_none()
        .h(height)
        .justify_between()
        .items_start()
        .child(
            div()
                .debug_selector(|| "system-chart-axis-max".into())
                .min_w_0()
                .max_w(px(CHART_AXIS_WIDTH))
                .whitespace_normal()
                .text_xs()
                // Use the tick font's own height so wrapped units fit compact 54px sensor plots.
                .line_height(gpui_kit::rems(0.75))
                .text_color(theme.muted_foreground)
                .child(format_value(maximum, unit)),
        );
    axis = axis.child(
        div()
            .debug_selector(|| "system-chart-axis-min".into())
            .min_w_0()
            .max_w(px(CHART_AXIS_WIDTH))
            .whitespace_normal()
            .text_xs()
            .line_height(gpui_kit::rems(0.75))
            .text_color(theme.muted_foreground)
            .child("0"),
    );
    v_flex()
        .w_full()
        .min_w_0()
        .gap(px(3.0))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap(px(CHART_AXIS_GAP))
                .child(axis)
                .child(
                    ramag_ui::pulse_ui::pulse_time_chart_with_series(series, maximum, height, cx)
                        .flex_1()
                        .min_w_0(),
                ),
        )
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap(px(CHART_AXIS_GAP))
                .child(div().w(px(CHART_AXIS_WIDTH)).flex_none())
                .child(
                    h_flex()
                        .debug_selector(|| "system-chart-time-range".into())
                        .flex_1()
                        .min_w_0()
                        .justify_between()
                        .child(div().text_xs().text_color(theme.muted_foreground).child(
                            if duration > 0.0 {
                                format!("{duration:.0}s 前")
                            } else {
                                "开始采样".into()
                            },
                        ))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("现在"),
                        ),
                ),
        )
}

/// Keeps the axis scale positive and finite so its labels match the chart primitive's fallback.
fn chart_axis_maximum(maximum: f64) -> f64 {
    if maximum.is_finite() && maximum > 0.0 {
        maximum
    } else {
        1.0
    }
}

#[cfg(test)]
#[path = "helpers/chart_axis_tests.rs"]
mod chart_axis_tests;
