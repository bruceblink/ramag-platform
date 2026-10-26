//! Display durable preference-write results without blocking other settings controls.

use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, Styled, div, px,
};

use crate::preferences::{PreferenceSaveStatus, preference_save_status, retry_failed_preference};

/// Render only keys changed in this session; a failed draft stays active until retried.
pub(crate) fn preference_status(
    keys: &[(&'static str, &'static str)],
    cx: &App,
) -> Option<AnyElement> {
    let theme = cx.theme();
    let mut rows = v_flex().w_full().min_w_0().gap(px(4.0));
    let mut has_status = false;
    for &(key, label) in keys {
        let Some(status) = preference_save_status(key, cx) else {
            continue;
        };
        has_status = true;
        let (message, color, failed, status_label, status_icon) = match status {
            PreferenceSaveStatus::Saving => (
                format!("{label}：正在保存…"),
                theme.muted_foreground,
                false,
                "保存中",
                IconName::Info,
            ),
            PreferenceSaveStatus::Saved => (
                format!("{label}：已保存"),
                theme.success,
                false,
                "已保存",
                IconName::Info,
            ),
            PreferenceSaveStatus::Failed(reason) => (
                format!("{label}：保存失败，当前选择尚未保存。{reason}"),
                theme.danger,
                true,
                "保存失败",
                IconName::CircleX,
            ),
        };
        let selector = format!("settings-save-status-{key}");
        let row_selector = selector.clone();
        let mut row = h_flex()
            .debug_selector(move || row_selector.clone())
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .child(
                h_flex()
                    .id(format!("{selector}-badge"))
                    .debug_selector({
                        let selector = format!("{selector}-badge");
                        move || selector.clone()
                    })
                    .flex_none()
                    .items_center()
                    .gap(px(3.0))
                    .px(px(5.0))
                    .py(px(1.0))
                    .rounded(px(5.0))
                    .bg(color.opacity(0.12))
                    .child(Icon::new(status_icon).xsmall().text_color(color))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(color)
                            .child(status_label),
                    ),
            )
            .child(
                div()
                    .id(format!("{selector}-message"))
                    .debug_selector({
                        let selector = format!("{selector}-message");
                        move || selector.clone()
                    })
                    .flex_1()
                    .min_w(px(160.0))
                    .text_xs()
                    .text_color(color)
                    .whitespace_normal()
                    .child(message),
            );
        if failed {
            let retry_id = format!("settings-save-retry-{key}");
            let selector = retry_id.clone();
            row = row.child(
                crate::clickable_button(retry_id)
                    .debug_selector(move || selector.clone())
                    .outline()
                    .small()
                    .icon(crate::icons::refresh_cw())
                    .label("重试")
                    .on_click(move |_, _, cx| retry_failed_preference(key, cx)),
            );
        }
        rows = rows.child(row);
    }
    has_status.then(|| rows.into_any_element())
}
