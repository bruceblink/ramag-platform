//! 工具栏筛选框的共享外观；输入状态和补全仍由调用方持有。

use gpui_kit::component::{ActiveTheme, h_flex};
use gpui_kit::{App, Div, FocusHandle, InteractiveElement, SharedString, Stateful, Styled, px};

/// 为单行输入和带补全的编辑器创建同样的外框。
/// 高度随全局字号增长，预留编辑器上下各 8px 内边距及 1px 边框；
/// 外框只跟随真实输入自身的焦点，不把父级工作区焦点误显示为输入框焦点；
/// 不创建额外 Tab 停靠点，也不截断补全弹层。
pub fn filter_field(id: impl Into<SharedString>, focus: &FocusHandle, cx: &App) -> Stateful<Div> {
    let id = id.into();
    let selector = id.to_string();
    let theme = cx.theme();
    let ring = theme.ring;
    h_flex()
        .id(id)
        .debug_selector(move || selector.clone())
        .w_full()
        .min_w_0()
        .h(theme.font_size * 1.25 + px(18.0))
        .items_center()
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
        .rounded(px(6.0))
        .track_focus(focus)
        // `in_focus` 会把父级工作区获得焦点也算作输入框聚焦，导致多个输入框同时亮起。
        // `focus` 只匹配当前输入的 FocusHandle，确保未选中的输入框保持普通边框。
        .focus(move |style| style.border_color(ring))
}
