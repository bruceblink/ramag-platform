use gpui_kit::component::{ActiveTheme as _, Sizable as _, button::ButtonVariants as _, h_flex};
use gpui_kit::{
    Div, InteractiveElement as _, ParentElement as _, SharedString, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

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
