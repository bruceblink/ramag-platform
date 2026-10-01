//! Process cell formatting and bounded presentation details.

use gpui_kit::component::v_flex;
use gpui_kit::{
    InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div, px,
};
use ramag_infra_system::{Availability, ProcessRow, Reading, Unit};

use super::super::super::helpers;
use crate::{MonitorSnapshot, ProcessSort};

/// Maps each sortable physical metric to its reading, formatter unit, and desktop width.
pub(super) fn metric_columns(process: &ProcessRow) -> [(ProcessSort, &Reading, &Unit, f32); 4] {
    [
        (ProcessSort::Cpu, &process.cpu_percent, &Unit::Percent, 74.0),
        (
            ProcessSort::Memory,
            &process.memory_bytes,
            &Unit::Bytes,
            90.0,
        ),
        (
            ProcessSort::Read,
            &process.read_bytes_per_second,
            &Unit::BytesPerSecond,
            100.0,
        ),
        (
            ProcessSort::Write,
            &process.write_bytes_per_second,
            &Unit::BytesPerSecond,
            100.0,
        ),
    ]
}

/// Uses freshness and field availability so real zero differs from missing readings.
pub(super) fn metric_display(
    snapshot: &MonitorSnapshot,
    reading: &Reading,
    unit: &Unit,
) -> (String, Option<String>) {
    if snapshot.collection_stale {
        let reason = reading
            .reason
            .clone()
            .or_else(|| Some("采集器未返回新快照".into()));
        return (format!("已过期 {}", helpers::unit_label(unit)), reason);
    }
    if let Some(value) = snapshot.process_value(reading) {
        return (helpers::format_value(value, unit), None);
    }
    let (label, reason) = match reading.availability {
        Availability::Available => (
            "读取失败",
            reading
                .reason
                .clone()
                .or_else(|| Some("采集源返回了无效读数".into())),
        ),
        Availability::WarmingUp => ("预热中", reading.reason.clone()),
        Availability::Unavailable => ("不可用", reading.reason.clone()),
        Availability::Failed => ("读取失败", reading.reason.clone()),
    };
    (format!("{label} {}", helpers::unit_label(unit)), reason)
}

/// Adds a stable selector and reveals clipped values or full failure reasons on hover.
pub(super) fn cell(
    pid: u32,
    key: &str,
    value: String,
    width: f32,
    reason: Option<String>,
    theme: &gpui_kit::component::theme::Theme,
) -> impl IntoElement + gpui_kit::Styled {
    let selector = format!("system-process-cell-{pid}-{key}");
    let full_text = reason.unwrap_or_else(|| value.clone());
    let cell = div()
        .id(selector.clone())
        .debug_selector(move || selector.clone())
        .min_w_0()
        .overflow_hidden()
        .text_ellipsis()
        .text_sm()
        .text_color(theme.foreground)
        .child(value);
    if width > 0.0 {
        cell.w(px(width)).flex_none().tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(full_text.clone()).build(window, cx)
        })
    } else {
        cell.tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(full_text.clone()).build(window, cx)
        })
    }
}

/// Stacks the metric name above a readable value in each compact grid cell.
pub(super) fn metric_cell(
    pid: u32,
    sort: ProcessSort,
    value: String,
    reason: Option<String>,
    theme: &gpui_kit::component::theme::Theme,
) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap(px(1.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(sort.label()),
        )
        .child(cell(pid, sort.stable_id(), value, 0.0, reason, theme).text_xs())
}

pub(super) fn col(
    value: &str,
    width: f32,
    theme: &gpui_kit::component::theme::Theme,
) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .min_w_0()
        .overflow_hidden()
        .text_ellipsis()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramag_infra_system::RawObservation;

    fn reading(availability: Availability, value: Option<f64>, reason: Option<&str>) -> Reading {
        Reading {
            sensor_id: "process-test".into(),
            value,
            total: None,
            availability,
            reason: reason.map(str::to_owned),
            observations: Vec::<RawObservation>::new(),
        }
    }

    #[test]
    fn metric_display_preserves_current_zero_and_large_single_core_cpu() {
        let snapshot = MonitorSnapshot::default();
        assert_eq!(
            metric_display(
                &snapshot,
                &reading(Availability::Available, Some(0.0), None),
                &Unit::Percent,
            )
            .0,
            "0.0 %"
        );
        assert_eq!(
            metric_display(
                &snapshot,
                &reading(Availability::Available, Some(155.0), None),
                &Unit::Percent,
            )
            .0,
            "155.0 %"
        );
    }

    #[test]
    fn metric_display_keeps_state_unit_reason_and_staleness() {
        let snapshot = MonitorSnapshot::default();
        for (availability, expected) in [
            (Availability::WarmingUp, "预热中 B/s"),
            (Availability::Unavailable, "不可用 B/s"),
            (Availability::Failed, "读取失败 B/s"),
        ] {
            let (value, reason) = metric_display(
                &snapshot,
                &reading(availability, None, Some("测试读取原因")),
                &Unit::BytesPerSecond,
            );
            assert_eq!(value, expected);
            assert_eq!(reason.as_deref(), Some("测试读取原因"));
        }
        let (invalid, _) = metric_display(
            &snapshot,
            &reading(Availability::Available, Some(f64::NAN), None),
            &Unit::Bytes,
        );
        assert_eq!(invalid, "读取失败 B");

        let stale = MonitorSnapshot {
            collection_stale: true,
            ..MonitorSnapshot::default()
        };
        let (value, reason) = metric_display(
            &stale,
            &reading(Availability::Failed, None, Some("旧读数失败原因")),
            &Unit::BytesPerSecond,
        );
        assert_eq!(value, "已过期 B/s");
        assert_eq!(reason.as_deref(), Some("旧读数失败原因"));
    }
}
