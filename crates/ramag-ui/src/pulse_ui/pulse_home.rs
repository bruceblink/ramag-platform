//! Shared entry surfaces. Callers own content, column schemas and domain actions.

use gpui_kit::component::v_flex;
use gpui_kit::{Div, Styled as _, px};

use super::{pulse_palette, pulse_panel};

/// Center entry content while keeping the list viewport flexible and bounded.
pub fn pulse_home_frame(cx: &gpui_kit::App) -> Div {
    v_flex()
        .size_full()
        .min_h_0()
        .min_w_0()
        .items_center()
        .px(px(24.0))
        .pt(px(22.0))
        .pb(px(16.0))
        .gap(px(14.0))
        .bg(pulse_palette(cx).background)
}

/// Keep toolbar, headings, rows and all loading/error/empty states on one surface.
pub fn pulse_home_panel(cx: &gpui_kit::App) -> Div {
    pulse_panel(cx)
        .max_w(px(1080.0))
        .flex_1()
        .min_h_0()
        .p_0()
        .overflow_hidden()
        .bg(pulse_palette(cx).surface)
}

/// Wrap controls when needed; never shrink fixed action buttons out of reach.
pub fn pulse_home_toolbar(cx: &gpui_kit::App) -> Div {
    crate::responsive_toolbar()
        .w_full()
        .flex_none()
        .px(px(14.0))
        .py(px(10.0))
        .border_b_1()
        .border_color(pulse_palette(cx).border)
}

/// Shared title/actions strip for editors and connected workspaces.
pub fn pulse_entry_header(cx: &gpui_kit::App) -> Div {
    crate::responsive_toolbar()
        .w_full()
        .flex_none()
        .px(px(24.0))
        .py(px(16.0))
        .gap(px(12.0))
        .border_b_1()
        .border_color(pulse_palette(cx).border)
        .bg(pulse_palette(cx).background)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{
        Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, TestAppContext,
        Window, div, size,
    };

    struct Entry;

    impl Render for Entry {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            pulse_home_frame(cx)
                .child(
                    super::super::pulse_page_title("数据源管理", Some("连接与对象工作区"), cx)
                        .debug_selector(|| "home-test-title".into())
                        .max_w(px(1080.0)),
                )
                .child(
                    pulse_home_panel(cx)
                        .debug_selector(|| "home-test-panel".into())
                        .child(
                            pulse_home_toolbar(cx)
                                .debug_selector(|| "home-test-toolbar".into())
                                .children(
                                    ["新建连接", "导入", "管理"].into_iter().enumerate().map(
                                        |(index, label)| {
                                            crate::clickable_button(("home-test-action", index))
                                                .label(label)
                                                .debug_selector(move || {
                                                    format!("home-test-action-{index}")
                                                })
                                        },
                                    ),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .debug_selector(|| "home-test-body".into()),
                        ),
                )
        }
    }

    #[gpui_kit::test]
    fn shared_home_surfaces_keep_controls_and_body_reachable(cx: &mut TestAppContext) {
        cx.update(gpui_kit::component::init);
        let (_, cx) = cx.add_window_view(|_, _| Entry);
        for mode in [crate::Mode::Dark, crate::Mode::Light] {
            for text_size in [
                crate::InterfaceTextSize::Standard,
                crate::InterfaceTextSize::Large,
            ] {
                cx.update(|_, app| {
                    crate::set_system_settings(
                        crate::SystemSettings {
                            text_size,
                            ..Default::default()
                        },
                        app,
                    );
                    crate::apply_theme(mode, app);
                });
                for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
                    cx.simulate_resize(size(px(width), px(height)));
                    cx.run_until_parked();
                    let title = cx.debug_bounds("home-test-title");
                    let panel = cx.debug_bounds("home-test-panel");
                    let toolbar = cx.debug_bounds("home-test-toolbar");
                    let body = cx.debug_bounds("home-test-body");
                    assert!(title.is_some(), "共享首页标题应渲染");
                    assert!(panel.is_some(), "共享首页面板应渲染");
                    assert!(toolbar.is_some(), "共享首页工具栏应渲染");
                    assert!(body.is_some(), "共享首页主体应渲染");
                    if let (Some(title), Some(panel), Some(toolbar), Some(body)) =
                        (title, panel, toolbar, body)
                    {
                        assert!(title.bottom() <= panel.origin.y);
                        assert!(panel.right() <= px(width) && panel.bottom() <= px(height));
                        assert!(
                            toolbar.origin.x >= panel.origin.x && toolbar.right() <= panel.right()
                        );
                        assert!(body.origin.y >= toolbar.bottom() && body.size.height >= px(240.0));
                    }
                    for selector in [
                        "home-test-action-0",
                        "home-test-action-1",
                        "home-test-action-2",
                    ] {
                        let button = cx.debug_bounds(selector);
                        assert!(button.is_some(), "共享首页操作按钮应渲染");
                        if let (Some(button), Some(toolbar)) = (button, toolbar) {
                            assert!(
                                button.origin.x >= toolbar.origin.x
                                    && button.right() <= toolbar.right()
                            );
                            assert!(
                                button.origin.y >= toolbar.origin.y
                                    && button.bottom() <= toolbar.bottom()
                            );
                        }
                    }
                }
            }
        }
    }
}
