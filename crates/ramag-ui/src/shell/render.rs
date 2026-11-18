//! Shell 的窗口布局、导航外壳和窄屏快捷入口渲染。

use gpui_kit::base::{FocusTrapElement as _, Selectable as _};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyView, Context, IntoElement, KeyDownEvent, ParentElement, Render, Role, Styled, Window, div,
    prelude::*, px,
};
use ramag_app::ToolRegistry;

use crate::activity_bar::{ActivityBar, NavTarget};

use super::modal_element::ModalElement;
use super::{SHELL_HEADER_HEIGHT, SHELL_NARROW_BREAKPOINT, Shell};

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 先拷颜色避开 theme 借用与 cx 可变借用冲突
        let bg_color = cx.theme().background;
        let fg_color = cx.theme().foreground;
        let border_color = cx.theme().border;
        let muted_foreground = cx.theme().muted_foreground;
        let compact = f32::from(window.viewport_size().width) <= SHELL_NARROW_BREAKPOINT;
        // 顶栏留白按原型的窄屏、中宽和桌面断点调整。
        let header_padding = if compact {
            16.0
        } else if f32::from(window.viewport_size().width) <= 1100.0 {
            23.0
        } else {
            26.0
        };

        let content_view: Option<AnyView> = if self.settings_selected {
            self.settings_view.clone()
        } else {
            match &self.selected {
                None => self.home_view.clone(),
                Some(id) => self.tool_views.get(id).cloned(),
            }
        };

        let theme_icon = match crate::theme::current_mode(cx) {
            crate::theme::Mode::Light => IconName::Moon,
            crate::theme::Mode::Dark => IconName::Sun,
        };
        let theme_toggle = crate::clickable_button("shell-theme-toggle")
            .debug_selector(|| "shell-theme-toggle".into())
            .ghost()
            .size(px(28.0))
            .p_0()
            .flex_none()
            .icon(Icon::new(theme_icon))
            .tooltip("切换主题")
            .on_click(|_, _, cx| crate::theme::toggle_theme(cx));

        let navigation_toggle = crate::clickable_button("shell-navigation-toggle")
            .debug_selector(|| "shell-navigation-toggle".into())
            .track_focus(&self.navigation_toggle_focus)
            .ghost()
            .size(px(28.0))
            .p_0()
            .flex_none()
            .icon(Icon::new(if self.navigation_open {
                IconName::PanelLeftClose
            } else {
                IconName::PanelLeftOpen
            }))
            .tooltip(if self.navigation_open {
                "关闭工具导航"
            } else {
                "打开工具导航"
            })
            .accessibility_label(if self.navigation_open {
                "关闭工具导航"
            } else {
                "打开工具导航"
            })
            .on_click(cx.listener(|this, _, window, cx| {
                this.navigation_open = !this.navigation_open;
                if this.navigation_open {
                    cx.on_next_frame(window, |this, window, cx| {
                        // 延迟回调执行前可能已关闭抽屉，此时保留关闭操作设置的焦点。
                        if this.navigation_open {
                            this.focus_navigation_drawer(window, cx);
                        }
                    });
                }
                cx.notify();
            }));

        let shell_label = self.page_title();
        let shell_label_tooltip = shell_label.clone();
        let mut header = h_flex()
            .id("workbench-shell-header")
            .debug_selector(|| "workbench-shell-header".into())
            .w_full()
            .min_w_0()
            .h(px(SHELL_HEADER_HEIGHT))
            .flex_none()
            .items_center()
            .gap(px(if compact { 9.0 } else { 12.0 }))
            .px(px(header_padding))
            .bg(bg_color)
            .border_b_1()
            .border_color(border_color.opacity(0.72));
        if compact {
            header = header.child(navigation_toggle);
        }
        if !compact {
            header = header.child(
                h_flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .id("workbench-shell-workspace")
                            .debug_selector(|| "workbench-shell-workspace".into())
                            .flex_none()
                            .text_size(px(13.0))
                            .text_color(muted_foreground)
                            .child("本地工作区"),
                    )
                    .child(
                        div()
                            .id("workbench-shell-separator")
                            .debug_selector(|| "workbench-shell-separator".into())
                            .text_lg()
                            .font_weight(gpui_kit::FontWeight::NORMAL)
                            .text_color(border_color)
                            .child("/"),
                    ),
            );
        }
        // 窄屏顶栏只显示当前页面名称，与原型中的单段面包屑一致。
        header = header
            .child(
                div()
                    .id("workbench-shell-page")
                    .debug_selector(|| "workbench-shell-page".into())
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_size(px(13.0))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(fg_color)
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(shell_label_tooltip.clone())
                            .build(window, cx)
                    })
                    .child(shell_label),
            )
            .child(theme_toggle);

        // 桌面侧栏从窗口顶部开始，顶栏只覆盖右侧工作区；窄屏保留独立顶栏。
        let (outer_header, workbench_header) = if compact {
            (Some(header), None)
        } else {
            (None, Some(header))
        };
        let page_view = div()
            .debug_selector(|| "workbench-shell-content".into())
            .flex_1()
            .w_full()
            .min_w_0()
            .min_h_0()
            .items_stretch()
            .when_some(content_view, |this, view| this.child(view))
            .when(
                (self.settings_selected && self.settings_view.is_none())
                    || (!self.settings_selected
                        && self.selected.is_some()
                        && self
                            .selected
                            .as_ref()
                            .and_then(|id| self.tool_views.get(id))
                            .is_none()),
                |this| this.child(render_view_missing(cx)),
            );
        let mut page_column = v_flex()
            .flex_1()
            .h_full()
            .w_full()
            .min_w_0()
            .min_h_0()
            .items_stretch()
            .when_some(workbench_header, |column, header| column.child(header))
            .child(page_view);
        if compact {
            page_column = page_column.child(render_compact_navigation(
                &self.registry,
                self.selected.as_deref(),
                self.settings_selected,
                cx,
            ));
        }

        let mut body = h_flex().flex_1().min_h_0();
        if !compact {
            body = body.child(self.activity_bar.clone());
        }
        body = body.child(page_column);
        let viewport = window.viewport_size();
        let viewport_width = f32::from(viewport.width);
        let viewport_height = f32::from(viewport.height);
        let drawer_width = super::navigation_drawer_width(viewport_width);
        let drawer_height = (viewport_height - 32.0).clamp(0.0, 640.0);

        v_flex()
            .size_full()
            .relative()
            .bg(bg_color)
            .text_color(fg_color)
            .key_context("Shell")
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.navigation_open && event.keystroke.key == "escape" {
                    this.close_navigation(window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_action(
                cx.listener(|this, action: &crate::actions::OpenTool, window, cx| {
                    this.navigate_to(NavTarget::Tool(action.tool_id.clone()), window, cx);
                }),
            )
            .on_action(cx.listener(
                |this, action: &crate::actions::OpenToolSettings, window, cx| {
                    if let Some(view) = this
                        .settings_view
                        .clone()
                        .and_then(|view| view.downcast::<crate::SettingsView>().ok())
                    {
                        view.update(cx, |settings, cx| {
                            settings.open_tool_page(&action.tool_id, window, cx)
                        });
                        this.navigate_to(NavTarget::Settings, window, cx);
                    }
                },
            ))
            .on_action(
                cx.listener(|this, _: &crate::actions::OpenSystemSettings, window, cx| {
                    if let Some(view) = this
                        .settings_view
                        .clone()
                        .and_then(|view| view.downcast::<crate::SettingsView>().ok())
                    {
                        view.update(cx, |settings, cx| {
                            settings.open_tool_page("system", window, cx)
                        });
                        this.navigate_to(NavTarget::Settings, window, cx);
                    }
                }),
            )
            .when_some(outer_header, |shell, header| shell.child(header))
            .child(body)
            .when(compact && self.navigation_open, |shell| {
                // 遮罩覆盖顶栏和内容，避免抽屉打开时触发背景操作。
                shell
                    .child(
                        div()
                            .id("shell-navigation-backdrop")
                            .debug_selector(|| "shell-navigation-backdrop".into())
                            .absolute()
                            .inset_0()
                            .occlude()
                            .bg(gpui_kit::black().opacity(0.5))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_navigation(window, cx);
                            })),
                    )
                    .child(ModalElement::new(
                        div()
                            .id("shell-navigation-drawer")
                            .debug_selector(|| "shell-navigation-drawer".into())
                            .role(Role::Dialog)
                            .aria_label("工具导航")
                            .absolute()
                            .top(px((viewport_height - drawer_height) / 2.0))
                            .left(px((viewport_width - drawer_width) / 2.0))
                            .w(px(drawer_width))
                            .h(px(drawer_height))
                            .occlude()
                            .rounded(px(12.0))
                            .overflow_hidden()
                            .shadow_sm()
                            .child(self.activity_bar.clone())
                            .child(
                                crate::clickable_button("shell-navigation-close")
                                    .debug_selector(|| "shell-navigation-close".into())
                                    .absolute()
                                    .top(px(8.0))
                                    .right(px(8.0))
                                    .ghost()
                                    .size(px(28.0))
                                    .p_0()
                                    .icon(IconName::Close)
                                    .tooltip("关闭工具导航")
                                    .accessibility_label("关闭工具导航")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_navigation(window, cx);
                                    })),
                            )
                            .focus_trap(
                                "shell-navigation-focus-trap",
                                &self.navigation_drawer_focus,
                            ),
                    ))
            })
            .child(self.data_sync_overlay.clone())
    }
}

/// 底部入口的图标、目标和选中状态。
struct CompactNavigationItem {
    id: &'static str,
    label: &'static str,
    icon: Icon,
    target: NavTarget,
    selected: bool,
}

/// 同一组底部入口共享的选中、正文和弱化颜色。
#[derive(Clone, Copy)]
struct CompactNavigationColors {
    selected_bg: gpui_kit::Hsla,
    foreground: gpui_kit::Hsla,
    muted_foreground: gpui_kit::Hsla,
}

/// 创建遵守侧栏显隐设置的窄屏快捷入口。
fn compact_navigation_button(
    item: CompactNavigationItem,
    colors: CompactNavigationColors,
    cx: &mut Context<Shell>,
) -> gpui_kit::component::button::Button {
    let CompactNavigationItem {
        id,
        label,
        icon,
        target,
        selected,
    } = item;
    let CompactNavigationColors {
        selected_bg,
        foreground,
        muted_foreground,
    } = colors;
    let selector = if selected {
        format!("shell-compact-nav-selected-{id}")
    } else {
        format!("shell-compact-nav-{id}")
    };
    let mut button = crate::clickable_button(format!("compact-nav-{id}"))
        .debug_selector(move || selector.clone())
        .ghost()
        .flex_1()
        .min_w_0()
        .h(px(52.0))
        .px(px(2.0))
        .selected(selected)
        .tooltip(label)
        .accessibility_label(if selected {
            format!("{label}，当前页面")
        } else {
            label.to_owned()
        })
        .on_click(cx.listener(move |this, _, window, cx| {
            this.navigate_to(target.clone(), window, cx);
        }))
        .child(
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap(px(2.0))
                .child(icon.size(px(16.0)))
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_center()
                        .text_xs()
                        .child(label),
                ),
        );
    if selected {
        // 标记当前入口为已选中，让按钮组件在悬停时继续使用选中样式。
        button = button.bg(selected_bg).text_color(foreground);
    } else {
        button = button.text_color(muted_foreground);
    }
    button
}

/// 在窄窗口保留总览、常用工具和设置入口；工具名称仍由抽屉提供。
fn render_compact_navigation(
    registry: &ToolRegistry,
    selected: Option<&str>,
    settings_selected: bool,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let (background, border, selected_bg, foreground, muted_foreground) = {
        let theme = cx.theme();
        (
            theme.sidebar,
            theme.border,
            theme.list_active,
            theme.foreground,
            theme.muted_foreground,
        )
    };
    let navigation_colors = CompactNavigationColors {
        selected_bg,
        foreground,
        muted_foreground,
    };
    let mut navigation = h_flex()
        .id("shell-compact-navigation")
        .debug_selector(|| "shell-compact-navigation".into())
        // 为屏幕阅读器标识底部快捷入口所在的导航区域。
        .role(Role::Navigation)
        .aria_label("快捷导航")
        .w_full()
        .h(px(56.0))
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .px(px(8.0))
        .bg(background)
        .border_t_1()
        .border_color(border);

    // 窄屏底栏使用原型指定的布局网格与花括号图标。
    navigation = navigation.child(compact_navigation_button(
        CompactNavigationItem {
            id: "home",
            label: "总览",
            icon: Icon::new(gpui_kit::assets::IconName::LayoutGrid),
            target: NavTarget::Home,
            selected: selected.is_none() && !settings_selected,
        },
        navigation_colors,
        cx,
    ));
    if registry
        .visibility("api")
        .is_some_and(|visibility| visibility.sidebar)
        && registry.find("api").is_some()
    {
        navigation = navigation.child(compact_navigation_button(
            CompactNavigationItem {
                id: "api",
                label: "API",
                icon: Icon::new(gpui_kit::assets::IconName::Braces),
                target: NavTarget::Tool("api".to_owned()),
                selected: selected == Some("api"),
            },
            navigation_colors,
            cx,
        ));
    }
    if registry
        .visibility("dbclient")
        .is_some_and(|visibility| visibility.sidebar)
        && let Some(tool) = registry.find("dbclient")
    {
        navigation = navigation.child(compact_navigation_button(
            CompactNavigationItem {
                id: "database",
                label: "数据库",
                icon: ActivityBar::icon_for_meta(tool.meta()),
                target: NavTarget::Tool("dbclient".to_owned()),
                selected: selected == Some("dbclient"),
            },
            navigation_colors,
            cx,
        ));
    }
    navigation.child(compact_navigation_button(
        CompactNavigationItem {
            id: "settings",
            label: "设置",
            icon: crate::icons::settings(),
            target: NavTarget::Settings,
            selected: settings_selected,
        },
        navigation_colors,
        cx,
    ))
}

fn render_view_missing(cx: &Context<Shell>) -> impl IntoElement {
    v_flex()
        .debug_selector(|| "shell-view-unavailable".into())
        .size_full()
        .items_center()
        .justify_center()
        .p(px(16.0))
        .child(
            div()
                .w_full()
                .max_w(px(360.0))
                .child(crate::pulse_ui::pulse_status_notice(
                    crate::pulse_ui::PulseStatus::Unavailable,
                    "该页面暂不可用",
                    cx,
                )),
        )
}
