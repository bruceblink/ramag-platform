//! 可复用的 System Pulse 风格监控 UI 基础组件。

mod pulse_home;
mod pulse_navigation;
pub use pulse_home::{pulse_entry_header, pulse_home_frame, pulse_home_panel, pulse_home_toolbar};
pub use pulse_navigation::{PulseTab, pulse_device_selector, pulse_tabs};

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{
    Bounds, Div, Hsla, InteractiveElement as _, ParentElement as _, PathBuilder, Pixels,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, fill, point,
    prelude::FluentBuilder as _, px, rgb, size,
};

/// 图表样本使用相对秒数，`None` 表示传感器缺失或采样间断。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartPoint {
    pub at_seconds: f64,
    pub value: Option<f64>,
}

/// Borrows one sensor series and assigns its line color. The chart copies at
/// most 120 recent points from each of the first 8 series before painting, so
/// callers retain ownership and rendering work stays bounded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartSeries<'a> {
    pub points: &'a [ChartPoint],
    pub color: Hsla,
}

/// 指标当前可用状态；颜色始终从 Ramag 当前主题读取。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseStatus {
    Current,
    Warming,
    Unavailable,
    Failed,
    Stale,
}

impl PulseStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Current => "当前",
            Self::Warming => "预热中",
            Self::Unavailable => "不可用",
            Self::Failed => "采集失败",
            Self::Stale => "数据过期",
        }
    }

    fn color(self, theme: &gpui_kit::component::Theme) -> Hsla {
        match self {
            Self::Current => theme.success,
            Self::Warming => theme.warning,
            Self::Unavailable | Self::Stale => theme.muted_foreground,
            Self::Failed => theme.danger,
        }
    }
}

/// System Tool 的中性表面色；应用主题与各工具共享这组明暗令牌。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulsePalette {
    pub background: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub selected: Hsla,
}

pub fn pulse_palette(cx: &gpui_kit::App) -> PulsePalette {
    pulse_palette_for_mode(crate::theme::current_mode(cx))
}

pub(crate) fn pulse_palette_for_mode(mode: crate::theme::Mode) -> PulsePalette {
    if mode == crate::theme::Mode::Dark {
        PulsePalette {
            background: rgb(0x242523).into(),
            surface: rgb(0x1f201e).into(),
            raised: rgb(0x2c2d2a).into(),
            border: rgb(0x42443e).into(),
            text: rgb(0xe2e4df).into(),
            muted: rgb(0xa4a79e).into(),
            selected: rgb(0x303b54).into(),
        }
    } else {
        PulsePalette {
            background: rgb(0xf3f4f1).into(),
            surface: rgb(0xffffff).into(),
            raised: rgb(0xe6e9e1).into(),
            border: rgb(0xcbd0c5).into(),
            text: rgb(0x252923).into(),
            muted: rgb(0x596252).into(),
            selected: rgb(0xdce8f7).into(),
        }
    }
}

/// Draws the shared System Tool display face at a caller-selected hierarchy size.
pub fn pulse_display_heading(text: impl Into<SharedString>, size: f32, cx: &gpui_kit::App) -> Div {
    div()
        .text_size(px(size * 0.86))
        .font_family("Michroma")
        .font_weight(gpui_kit::FontWeight::MEDIUM)
        .text_color(pulse_palette(cx).text)
        .child(text.into())
}

/// Creates an unframed, compact workbench header. The title shrinks before the
/// caller's fixed-size actions, and its tooltip retains the complete page name.
pub fn pulse_workbench_header(title: impl Into<SharedString>, cx: &gpui_kit::App) -> Div {
    let theme = cx.theme();
    let title = title.into();
    let tooltip = title.clone();
    h_flex()
        .debug_selector(|| "pulse-workbench-header".into())
        .w_full()
        .min_w_0()
        .h(px(crate::workbench::WORKBENCH_TOOLBAR_HEIGHT))
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .bg(theme.background)
        .border_b_1()
        .border_color(theme.border.opacity(0.65))
        .child(
            div()
                .debug_selector(|| "pulse-workbench-brand".into())
                .flex_none()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("Ramag"),
        )
        .child(
            gpui_kit::component::Icon::new(gpui_kit::component::IconName::ChevronRight)
                .size(px(12.0))
                .text_color(theme.muted_foreground),
        )
        .child(
            div()
                .id("pulse-workbench-title")
                .debug_selector(|| "pulse-workbench-title".into())
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tooltip.clone()).build(window, cx)
                })
                .child(title),
        )
}

/// 创建页面标题区域；返回可继续添加按钮或状态的 `Div`。
pub fn pulse_page_title(
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    cx: &gpui_kit::App,
) -> Div {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let title = title.into();
    v_flex()
        .debug_selector(|| "pulse-page-title".into())
        .w_full()
        .min_w_0()
        .gap(px(4.0))
        .child(
            pulse_display_heading(title, 22.0, cx)
                .debug_selector(|| "pulse-page-title-text".into()),
        )
        .when_some(subtitle, |this, subtitle| {
            this.child(
                div()
                    .debug_selector(|| "pulse-page-subtitle".into())
                    .text_sm()
                    .text_color(muted)
                    .child(subtitle.into()),
            )
        })
}

/// 创建边界克制的主题面板，内容可继续组合 GPUI 子元素。
pub fn pulse_panel(cx: &gpui_kit::App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .min_w_0()
        .items_stretch()
        .p(px(14.0))
        .border_1()
        .border_color(theme.border)
        .rounded(px(8.0))
        .bg(theme.secondary)
}

/// 创建带标签、主读数、单位和可选说明的指标卡。
pub fn pulse_metric_card(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    unit: Option<impl Into<SharedString>>,
    detail: Option<impl Into<SharedString>>,
    cx: &gpui_kit::App,
) -> Div {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let foreground = theme.foreground;
    let accent = theme.accent;
    pulse_panel(cx)
        .debug_selector(|| "pulse-metric-card".into())
        .flex_1()
        .min_w(px(132.0))
        .child(div().text_xs().text_color(muted).child(label.into()))
        .child(
            h_flex()
                .items_baseline()
                .gap(px(5.0))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(foreground)
                        .child(value.into()),
                )
                .when_some(unit, |row, unit| {
                    row.child(div().text_sm().text_color(accent).child(unit.into()))
                }),
        )
        .when_some(detail, |card, detail| {
            card.child(div().text_xs().text_color(muted).child(detail.into()))
        })
}

/// 创建简洁状态标签。
pub fn pulse_status_badge(status: PulseStatus, cx: &gpui_kit::App) -> Div {
    pulse_status_badge_with_label(status, status.label(), cx)
}
/// 创建保留领域文案的状态标签；颜色和布局仍由统一 Pulse 状态决定。
pub fn pulse_status_badge_with_label(
    status: PulseStatus,
    label: impl Into<SharedString>,
    cx: &gpui_kit::App,
) -> Div {
    let color = status.color(cx.theme());
    div()
        .debug_selector(|| "pulse-status-badge".into())
        .px(px(7.0))
        .py(px(3.0))
        .rounded(px(6.0))
        .bg(color.opacity(0.12))
        .text_xs()
        .text_color(color)
        .child(label.into())
}

/// 创建带状态色和原因的读数提示。
pub fn pulse_status_notice(
    status: PulseStatus,
    message: impl Into<SharedString>,
    cx: &gpui_kit::App,
) -> Div {
    let color = status.color(cx.theme());
    let foreground = cx.theme().foreground;
    h_flex()
        .debug_selector(|| "pulse-status-notice".into())
        .w_full()
        .min_w_0()
        .items_start()
        .gap(px(8.0))
        .p(px(10.0))
        .border_1()
        .border_color(color.opacity(0.28))
        .rounded(px(6.0))
        .child(pulse_status_badge(status, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(foreground)
                .child(message.into()),
        )
}

/// 创建按真实时间间距绘制的趋势图；缺失样本会断开折线，最多绘制最近 120 点。
pub fn pulse_time_chart(
    points: &[ChartPoint],
    max_value: f64,
    height: Pixels,
    cx: &gpui_kit::App,
) -> Div {
    pulse_time_chart_with_color(points, max_value, height, cx.theme().accent, cx)
}

/// 复用相同网格和有界采样，允许调用方用主题色区分 CPU、内存等物理指标。
pub fn pulse_time_chart_with_color(
    points: &[ChartPoint],
    max_value: f64,
    height: Pixels,
    line_color: Hsla,
    cx: &gpui_kit::App,
) -> Div {
    pulse_time_chart_with_series(
        &[ChartSeries {
            points,
            color: line_color,
        }],
        max_value,
        height,
        cx,
    )
}

/// Draws at most 8 independently gapped, colored series on one shared time axis.
pub fn pulse_time_chart_with_series(
    series: &[ChartSeries<'_>],
    max_value: f64,
    height: Pixels,
    cx: &gpui_kit::App,
) -> Div {
    pulse_time_chart_with_series_range(series, 0.0, max_value, height, cx)
}

/// Draws chart series against an explicit physical range, including signed temperatures.
pub fn pulse_time_chart_with_series_range(
    series: &[ChartSeries<'_>],
    min_value: f64,
    max_value: f64,
    height: Pixels,
    cx: &gpui_kit::App,
) -> Div {
    let series = bounded_chart_series(series);
    let time_bounds = chart_time_bounds(&series);
    let theme = cx.theme();
    let chart_bg = theme.muted;
    let grid_color = theme.border.opacity(0.42);
    let (min_value, max_value) = chart_value_range(min_value, max_value);
    let chart = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            paint_time_chart(
                bounds,
                window,
                &series,
                time_bounds,
                (min_value, max_value),
                chart_bg,
                grid_color,
            );
        },
    );
    div()
        .debug_selector(|| "pulse-time-chart".into())
        .w_full()
        .min_w_0()
        .h(height)
        .overflow_hidden()
        .child(chart.w_full().h_full().min_w_0())
}

/// 映射样本时间戳到真实横轴位置，并只连接连续且有效的相邻样本。
fn paint_time_chart(
    bounds: Bounds<Pixels>,
    window: &mut Window,
    series: &[BoundedChartSeries],
    time_bounds: Option<(f64, f64)>,
    value_range: (f64, f64),
    background: Hsla,
    grid_color: Hsla,
) {
    let (min_value, max_value) = value_range;
    let origin = bounds.origin + point(px(1.0), px(1.0));
    let width = (bounds.size.width - px(2.0)).max(px(1.0));
    let height = (bounds.size.height - px(2.0)).max(px(1.0));
    window.paint_quad(fill(Bounds::new(origin, size(width, height)), background));
    let mut grid = PathBuilder::stroke(px(1.0));
    for index in 1..=2 {
        let y = origin.y + height * (index as f32 / 3.0);
        grid.move_to(point(origin.x, y));
        grid.line_to(point(origin.x + width, y));
    }
    for index in 1..=3 {
        let x = origin.x + width * (index as f32 / 4.0);
        grid.move_to(point(x, origin.y));
        grid.line_to(point(x, origin.y + height));
    }
    if let Ok(path) = grid.build() {
        window.paint_path(path, grid_color);
    }
    for series in series {
        let segments =
            normalized_chart_segments_with_range(&series.points, min_value, max_value, time_bounds);
        let mut line = PathBuilder::stroke(px(2.0));
        for (x1, y1, x2, y2) in &segments {
            line.move_to(point(
                origin.x + width * *x1,
                origin.y + height * (1.0 - *y1),
            ));
            line.line_to(point(
                origin.x + width * *x2,
                origin.y + height * (1.0 - *y2),
            ));
        }
        if !segments.is_empty()
            && let Ok(path) = line.build()
        {
            window.paint_path(path, series.color);
        }
        if let Some((x, y)) = series.points.last().and_then(|sample| {
            normalized_chart_point_with_range(*sample, min_value, max_value, time_bounds)
        }) {
            let marker = px(5.0).min(width).min(height);
            let current = point(origin.x + width * x, origin.y + height * (1.0 - y));
            window.paint_quad(
                fill(
                    Bounds::new(
                        current - point(marker / 2.0, marker / 2.0),
                        size(marker, marker),
                    ),
                    series.color,
                )
                .corner_radii(marker / 2.0),
            );
        }
    }
}

// Owns bounded copies only; it never retains collector handles or caller data.
struct BoundedChartSeries {
    points: Vec<ChartPoint>,
    color: Hsla,
}

// The axis is the finite timestamp union so sensors with different windows align.
fn chart_time_bounds(series: &[BoundedChartSeries]) -> Option<(f64, f64)> {
    let mut times = series
        .iter()
        .flat_map(|series| series.points.iter().map(|point| point.at_seconds))
        .filter(|time| time.is_finite());
    let first = times.next()?;
    Some(times.fold((first, first), |(min, max), time| {
        (min.min(time), max.max(time))
    }))
}

// Copies recent samples while keeping both per-series and total work bounded.
fn bounded_chart_series(series: &[ChartSeries<'_>]) -> Vec<BoundedChartSeries> {
    series
        .iter()
        .take(8)
        .map(|series| BoundedChartSeries {
            points: bounded_chart_points(series.points),
            color: series.color,
        })
        .collect()
}

fn bounded_chart_points(points: &[ChartPoint]) -> Vec<ChartPoint> {
    points
        .iter()
        .rev()
        .take(120)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

// Rejects invalid samples, which breaks only their own line and suppresses its marker.
#[cfg(test)]
fn normalized_chart_point_with_bounds(
    sample: ChartPoint,
    max_value: f64,
    time_bounds: Option<(f64, f64)>,
) -> Option<(f32, f32)> {
    normalized_chart_point_with_range(sample, 0.0, max_value, time_bounds)
}

fn normalized_chart_point_with_range(
    sample: ChartPoint,
    min_value: f64,
    max_value: f64,
    time_bounds: Option<(f64, f64)>,
) -> Option<(f32, f32)> {
    let (first_time, last_time) = time_bounds?;
    if !sample.at_seconds.is_finite() {
        return None;
    }
    let value = sample.value.filter(|value| value.is_finite())?;
    let span = (last_time - first_time).max(f64::EPSILON);
    Some((
        ((sample.at_seconds - first_time) / span).clamp(0.0, 1.0) as f32,
        ((value - min_value) / (max_value - min_value)).clamp(0.0, 1.0) as f32,
    ))
}

/// Joins increasing adjacent valid samples; missing or invalid points break this series only.
#[cfg(test)]
fn normalized_chart_segments_with_bounds(
    points: &[ChartPoint],
    max_value: f64,
    time_bounds: Option<(f64, f64)>,
) -> Vec<(f32, f32, f32, f32)> {
    normalized_chart_segments_with_range(points, 0.0, max_value, time_bounds)
}

fn normalized_chart_segments_with_range(
    points: &[ChartPoint],
    min_value: f64,
    max_value: f64,
    time_bounds: Option<(f64, f64)>,
) -> Vec<(f32, f32, f32, f32)> {
    let mut segments = Vec::new();
    let mut previous: Option<(f64, (f32, f32))> = None;
    for sample in points.iter().copied() {
        let current = normalized_chart_point_with_range(sample, min_value, max_value, time_bounds);
        let Some(current) = current else {
            previous = None;
            continue;
        };
        if let Some((previous_time, previous_point)) = previous
            && sample.at_seconds > previous_time
        {
            segments.push((previous_point.0, previous_point.1, current.0, current.1));
        }
        previous = Some((sample.at_seconds, current));
    }
    segments
}

fn chart_value_range(min_value: f64, max_value: f64) -> (f64, f64) {
    let min_value = if min_value.is_finite() {
        min_value
    } else {
        0.0
    };
    let max_value = if max_value.is_finite() {
        max_value
    } else {
        1.0
    };
    if max_value > min_value {
        (min_value, max_value)
    } else {
        (min_value - 1.0, max_value + 1.0)
    }
}

#[cfg(test)]
#[path = "pulse_ui_tests.rs"]
mod tests;
