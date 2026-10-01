//! 可复用的 System Pulse 风格监控 UI 基础组件。

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::{
    Bounds, Div, Hsla, InteractiveElement as _, ParentElement as _, PathBuilder, Pixels,
    SharedString, Styled as _, Window, canvas, div, fill, point, prelude::FluentBuilder as _, px,
    size,
};

/// 图表样本使用相对秒数，`None` 表示传感器缺失或采样间断。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartPoint {
    pub at_seconds: f64,
    pub value: Option<f64>,
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

/// 创建页面标题区域；返回可继续添加按钮或状态的 `Div`。
pub fn pulse_page_title(
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    cx: &gpui_kit::App,
) -> Div {
    let theme = cx.theme();
    let foreground = theme.foreground;
    let muted = theme.muted_foreground;
    v_flex()
        .debug_selector(|| "pulse-page-title".into())
        .w_full()
        .min_w_0()
        .gap(px(4.0))
        .child(
            div()
                .text_xl()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(foreground)
                .child(title.into()),
        )
        .when_some(subtitle, |this, subtitle| {
            this.child(div().text_sm().text_color(muted).child(subtitle.into()))
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
        .border_color(theme.border.opacity(0.78))
        .rounded(px(6.0))
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
    let color = status.color(cx.theme());
    div()
        .debug_selector(|| "pulse-status-badge".into())
        .px(px(7.0))
        .py(px(3.0))
        .rounded(px(6.0))
        .bg(color.opacity(0.12))
        .text_xs()
        .text_color(color)
        .child(status.label())
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

/// 单个监控页签；ID 是调用方稳定的业务标识，标题仅供显示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulseTab {
    pub id: SharedString,
    pub title: SharedString,
}

/// 创建窄窗口可换行的页签带，点击后把稳定 ID 交回调用方。
pub fn pulse_tabs(
    tabs: &[PulseTab],
    selected_id: &str,
    window: &Window,
    cx: &gpui_kit::App,
    on_select: impl Fn(SharedString, &mut Window, &mut gpui_kit::App) + 'static,
) -> Div {
    let compact = f32::from(window.viewport_size().width) < 720.0;
    let theme = cx.theme();
    let on_select = std::rc::Rc::new(on_select);
    let mut row = h_flex()
        .debug_selector(|| "pulse-tabs".into())
        .w_full()
        .min_w_0()
        .flex_wrap()
        .gap(px(if compact { 4.0 } else { 8.0 }));
    for (index, tab) in tabs.iter().enumerate() {
        let selected = tab.id.as_ref() == selected_id;
        let callback = on_select.clone();
        let id = tab.id.clone();
        let selector = format!("pulse-tab-{index}");
        let button = crate::clickable_button(format!("pulse-tab-{index}"))
            .debug_selector(move || selector.clone())
            .small()
            .max_w(px(220.0))
            .label(tab.title.clone())
            .when(selected, |button| {
                button
                    .bg(theme.list_active)
                    .text_color(theme.foreground)
                    .border_color(theme.list_active_border)
            })
            .when(!selected, |button| {
                button.ghost().text_color(theme.muted_foreground)
            })
            .on_click(move |_, window, cx| callback(id.clone(), window, cx));
        row = row.child(button);
    }
    row
}

/// 创建设备选择控件；空集合时显示提示，设备变化不会改变控件布局规则。
pub fn pulse_device_selector(
    devices: &[(SharedString, SharedString)],
    selected_id: Option<&str>,
    cx: &gpui_kit::App,
    on_select: impl Fn(SharedString, &mut Window, &mut gpui_kit::App) + 'static,
) -> Div {
    let theme = cx.theme();
    let callback = std::rc::Rc::new(on_select);
    let mut row = h_flex()
        .debug_selector(|| "pulse-device-selector".into())
        .w_full()
        .min_w_0()
        .flex_wrap()
        .gap(px(6.0));
    if devices.is_empty() {
        return row.child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("无可用设备"),
        );
    }
    for (index, (id, label)) in devices.iter().enumerate() {
        let selected = selected_id == Some(id.as_ref());
        let selector = format!("pulse-device-{index}");
        let id = id.clone();
        let callback = callback.clone();
        let button = crate::clickable_button(format!("pulse-device-{index}"))
            .debug_selector(move || selector.clone())
            .small()
            .max_w(px(280.0))
            .label(label.clone())
            .when(selected, |button| {
                button
                    .bg(theme.list_active)
                    .text_color(theme.foreground)
                    .border_color(theme.list_active_border)
            })
            .when(!selected, |button| {
                button.ghost().text_color(theme.muted_foreground)
            })
            .on_click(move |_, window, cx| callback(id.clone(), window, cx));
        row = row.child(button);
    }
    row
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
    let points = bounded_chart_points(points);
    let theme = cx.theme();
    let chart_bg = theme.muted;
    let grid_color = theme.border.opacity(0.42);
    let max_value = if max_value.is_finite() && max_value > 0.0 {
        max_value
    } else {
        1.0
    };
    let chart = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            paint_time_chart(
                bounds, window, &points, max_value, chart_bg, grid_color, line_color,
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
    points: &[ChartPoint],
    max_value: f64,
    background: Hsla,
    grid_color: Hsla,
    line_color: Hsla,
) {
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
    let segments = normalized_chart_segments(points, max_value);
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
        window.paint_path(path, line_color);
    }
    if let Some((x, y)) = points
        .last()
        .and_then(|sample| normalized_chart_point(*sample, points, max_value))
    {
        let marker = px(5.0).min(width).min(height);
        let current = point(origin.x + width * x, origin.y + height * (1.0 - y));
        window.paint_quad(
            fill(
                Bounds::new(
                    current - point(marker / 2.0, marker / 2.0),
                    size(marker, marker),
                ),
                line_color,
            )
            .corner_radii(marker / 2.0),
        );
    }
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

fn normalized_chart_point(
    sample: ChartPoint,
    points: &[ChartPoint],
    max_value: f64,
) -> Option<(f32, f32)> {
    let first_time = points
        .iter()
        .map(|point| point.at_seconds)
        .filter(|time| time.is_finite())
        .reduce(f64::min)?;
    let last_time = points
        .iter()
        .map(|point| point.at_seconds)
        .filter(|time| time.is_finite())
        .reduce(f64::max)?;
    if !sample.at_seconds.is_finite() {
        return None;
    }
    let value = sample.value.filter(|value| value.is_finite())?;
    let span = (last_time - first_time).max(f64::EPSILON);
    Some((
        ((sample.at_seconds - first_time) / span).clamp(0.0, 1.0) as f32,
        (value / max_value).clamp(0.0, 1.0) as f32,
    ))
}

/// 提取相邻有效样本组成的归一化线段；None 和非递增时间会中断连接。
fn normalized_chart_segments(points: &[ChartPoint], max_value: f64) -> Vec<(f32, f32, f32, f32)> {
    let mut segments = Vec::new();
    let mut previous: Option<(f64, (f32, f32))> = None;
    for sample in points.iter().copied() {
        let current = normalized_chart_point(sample, points, max_value);
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

#[cfg(test)]
#[path = "pulse_ui_tests.rs"]
mod tests;
