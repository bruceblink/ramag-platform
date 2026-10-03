//! SSH 工作区：SFTP 浏览器与多 Terminal 标签。

mod file_browser;
use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    button::ButtonVariants as _,
    h_flex,
    resizable::{h_resizable, resizable_panel},
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px, uniform_list,
};
use ramag_domain::entities::{
    MAX_SSH_TERMINALS_PER_WORKSPACE, RemoteOperatingSystem, RemotePlatformPreference,
    SftpTransportKind, SshProfileId, SshProfileOrigin,
};
use std::ops::Range;

use super::SshView;
use super::model::{
    SshWorkspace, can_close_terminal, session_pulse_status, session_state_text,
    terminal_has_exited, terminal_pulse_status, terminal_tab_label,
};
use super::render_directory_helpers::{
    RemoteDirectoryDrag, RemoteEntryMenuState, centered_message, directory_counts,
    directory_counts_at, filtered_entry_indices, remote_breadcrumbs, remote_entry_row,
};

const FILE_BROWSER_WIDTH_INITIAL: f32 = 280.0;
const FILE_BROWSER_WIDTH_MIN: f32 = 180.0;
const FILE_BROWSER_WIDTH_MAX: f32 = 600.0;

impl SshView {
    pub(super) fn render_workspace(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(workspace) = self.active_workspace() else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap(px(10.0))
                .child("暂无连接")
                .child(
                    ramag_ui::clickable_button("ssh-empty-manager")
                        .primary()
                        .label("返回")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.show_manager(cx);
                        })),
                )
                .into_any_element();
        };
        let workspace_id = workspace.profile.id.clone();
        let endpoint = workspace.profile.port.map_or_else(
            || workspace.profile.host.clone(),
            |port| format!("{}:{port}", workspace.profile.host),
        );
        let session_state = workspace.session_state;
        let workspace_header = h_flex()
            .id("ssh-workspace-page-header")
            .debug_selector(|| "ssh-workspace-page-header".into())
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.78))
            .child(
                ramag_ui::pulse_ui::pulse_page_title(
                    workspace.profile.name.clone(),
                    Some(endpoint),
                    cx,
                )
                .id("ssh-workspace-page-title")
                .debug_selector(|| "ssh-workspace-page-title".into())
                .flex_1()
                .min_w_0(),
            )
            .child(
                h_flex()
                    .id("ssh-workspace-connection-status")
                    .debug_selector(|| "ssh-workspace-connection-status".into())
                    .flex_none()
                    .items_center()
                    .gap(px(8.0))
                    .child(ramag_ui::pulse_ui::pulse_status_badge_with_label(
                        session_pulse_status(session_state),
                        session_state_text(session_state),
                        cx,
                    ))
                    .child(
                        ramag_ui::clickable_button("ssh-workspace-back-to-manager")
                            .outline()
                            .small()
                            .icon(IconName::Network)
                            .label("连接管理")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.show_manager(cx);
                            })),
                    ),
            );
        let workspace_resize = if let Some(state) = self.workspace_resizes.get(&workspace_id) {
            state.clone()
        } else {
            let state = cx.new(|_| gpui_kit::component::resizable::ResizableState::default());
            let workspace_id_for_resize = workspace_id.clone();
            let subscription = cx.subscribe_in(
                &state,
                window,
                move |this, state, _: &gpui_kit::component::ResizablePanelEvent, _, cx| {
                    if let Some(width) = state.read(cx).sizes().first().copied() {
                        this.workspace_panel_widths
                            .insert(workspace_id_for_resize.clone(), width);
                        cx.notify();
                    }
                },
            );
            self.workspace_resizes
                .insert(workspace_id.clone(), state.clone());
            self.workspace_panel_widths
                .insert(workspace_id.clone(), px(FILE_BROWSER_WIDTH_INITIAL));
            self.workspace_resize_subscriptions
                .insert(workspace_id.clone(), subscription);
            state
        };
        // gpui-kit keeps resizable panels proportional when the window changes size.
        // Restore the user's pixel width so an existing SSH workspace does not
        // unexpectedly collapse after a window resize; the target is still bounded
        // by the minimum width of the terminal panel and the current viewport.
        let desired_width = self
            .workspace_panel_widths
            .get(&workspace_id)
            .copied()
            .unwrap_or_else(|| px(FILE_BROWSER_WIDTH_INITIAL));
        let max_width = (f32::from(window.viewport_size().width) - FILE_BROWSER_WIDTH_MIN)
            .clamp(FILE_BROWSER_WIDTH_MIN, FILE_BROWSER_WIDTH_MAX);
        let target_width = desired_width.min(px(max_width));
        let should_restore_width = {
            let state = workspace_resize.read(cx);
            state.container_size() > px(0.0)
                && state
                    .sizes()
                    .first()
                    .is_some_and(|current| *current != target_width)
        };
        if should_restore_width {
            workspace_resize.update(cx, |state, state_cx| {
                state.resize_panel(0, target_width, window, state_cx);
            });
        }
        let main = div()
            .id("ssh-workspace-main")
            .debug_selector(|| "ssh-workspace-main".into())
            .flex_1()
            .min_h_0()
            .child(
                h_resizable("ssh-workspace-resize")
                    .with_state(&workspace_resize)
                    .child(
                        resizable_panel()
                            .flex_none()
                            .size(px(FILE_BROWSER_WIDTH_INITIAL))
                            .size_range(px(FILE_BROWSER_WIDTH_MIN)..px(FILE_BROWSER_WIDTH_MAX))
                            .child(self.render_file_browser(workspace_id.clone(), cx)),
                    )
                    .child(resizable_panel().child(
                        div().size_full().min_w_0().child(self.render_terminal_pane(
                            workspace_id,
                            window,
                            cx,
                        )),
                    )),
            );
        v_flex()
            .size_full()
            .relative()
            .child(workspace_header)
            .child(main)
            .child(self.render_transfer_queue(cx))
            .into_any_element()
    }

    fn render_directory_breadcrumb(
        &self,
        workspace_id: SshProfileId,
        path: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let parts = remote_breadcrumbs(path);
        let last = parts.len().saturating_sub(1);
        let link = cx.theme().link;
        let link_hover = cx.theme().link_hover;
        let muted = cx.theme().muted_foreground;
        let path_drag = RemoteDirectoryDrag::from_current_path(workspace_id.clone(), path);
        let workspace_for_prompt = workspace_id.clone();
        let mut path_parts = h_flex()
            .id("ssh-directory-path-scroll")
            .flex_1()
            .min_w_0()
            .gap(px(5.0))
            .overflow_x_scrollbar();
        for (index, (label, target)) in parts.into_iter().enumerate() {
            if index > 0 {
                path_parts = path_parts.child(
                    div()
                        .flex_none()
                        .text_color(muted)
                        .child(SharedString::from("›")),
                );
            }
            let id = SharedString::from(format!("ssh-path-part-{index}"));
            let target_for_click = target.clone();
            let workspace_id_for_click = workspace_id.clone();
            path_parts = path_parts.child(
                div()
                    .id(id)
                    .flex_none()
                    .cursor_pointer()
                    .text_color(link)
                    .when(index == last, |part| {
                        part.font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    })
                    .hover(move |part| part.text_color(link_hover))
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.refresh_directory(
                            workspace_id_for_click.clone(),
                            Some(target_for_click.clone()),
                            cx,
                        );
                    })),
            );
        }
        h_flex()
            .id("ssh-directory-breadcrumb")
            .debug_selector(|| "ssh-directory-breadcrumb".into())
            .w_full()
            .h(px(40.0))
            .flex_none()
            .items_center()
            .gap(px(5.0))
            .px(px(10.0))
            .border_b_1()
            .border_color(cx.theme().border)
            .text_xs()
            .child(
                div()
                    .id("ssh-directory-path-label")
                    .debug_selector(|| "ssh-directory-path-label".into())
                    .flex_none()
                    .cursor_pointer()
                    .text_color(muted)
                    .hover(move |label| label.text_color(link_hover))
                    .child("路径")
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.prompt_remote_path(workspace_for_prompt.clone(), window, cx);
                    })),
            )
            .child(path_parts)
            .when_some(path_drag, |breadcrumb, drag| {
                breadcrumb
                    .cursor_pointer()
                    .on_drag(drag, |drag, position, _, cx| {
                        cx.new(|_| drag.clone().position(position))
                    })
            })
    }

    fn render_terminal_pane(
        &self,
        workspace_id: SshProfileId,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(workspace) = self
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &workspace_id)
        else {
            return div().into_any_element();
        };
        let terminal_loading = workspace.terminal_loading;
        let production = workspace.profile.production;
        let connection_available = self.profile_connection_available(&workspace.profile);
        let active_terminal_id = workspace.active_terminal_id;
        let terminal_views = workspace
            .terminals
            .iter()
            .map(|terminal| (terminal.id, terminal.label.clone(), terminal.view.clone()))
            .collect::<Vec<_>>();
        let terminals_can_close = can_close_terminal(terminal_views.len());
        let terminal_limit_reached = terminal_views.len() >= MAX_SSH_TERMINALS_PER_WORKSPACE;
        let border = cx.theme().border;
        let secondary = cx.theme().secondary;
        let muted_bg = cx.theme().muted;
        let muted = cx.theme().muted_foreground;
        let foreground = cx.theme().foreground;
        let accent = cx.theme().accent;
        let mut drop_background = accent;
        drop_background.a = 0.08;
        let warning = cx.theme().warning;

        let mut tabs_strip = h_flex()
            .id(SharedString::from(format!(
                "ssh-terminal-tabs-{workspace_id}"
            )))
            .flex_1()
            .min_w_0()
            .gap(px(4.0))
            .px(px(8.0))
            .py(px(2.0))
            .overflow_x_scrollbar();
        for (terminal_id, fallback_label, terminal) in &terminal_views {
            let id = *terminal_id;
            let id_for_close = id;
            let reconnect_workspace_id = workspace_id.clone();
            let selected = active_terminal_id == Some(id);
            let state = terminal.read(cx);
            let label = fallback_label.to_string();
            let core = state.core();
            let exit_status = core.exit_status();
            let finished = terminal_has_exited(core);
            let can_reconnect = finished;
            let (terminal_status, terminal_status_label) =
                terminal_pulse_status(exit_status.as_ref(), finished);
            let display = terminal_tab_label(&label, exit_status.as_ref(), finished);
            let mut tab = h_flex()
                .id(("ssh-terminal-tab", id))
                .debug_selector(move || format!("ssh-terminal-tab-{id}"))
                .flex_none()
                .max_w(px(260.0))
                .items_center()
                .gap_2()
                .px_3()
                .py(px(7.0))
                .border_1()
                .border_color(border)
                .rounded(px(4.0))
                .cursor_pointer()
                .on_click(cx.listener({
                    let workspace_id = workspace_id.clone();
                    move |this, _: &ClickEvent, window, cx| {
                        this.select_terminal(workspace_id.clone(), id, window, cx);
                    }
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(if selected { foreground } else { muted })
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(display),
                )
                .child(
                    div()
                        .id(SharedString::from(format!(
                            "ssh-terminal-session-status-{id}"
                        )))
                        .debug_selector(move || format!("ssh-terminal-session-status-{id}"))
                        .flex_none()
                        .child(ramag_ui::pulse_ui::pulse_status_badge_with_label(
                            terminal_status,
                            terminal_status_label,
                            cx,
                        )),
                )
                .when(can_reconnect, |tab| {
                    tab.child(
                        div()
                            .debug_selector(move || format!("ssh-terminal-reconnect-{id}"))
                            .flex_none()
                            .child(
                                ramag_ui::clickable_button(("reconnect-ssh-terminal", id))
                                    .ghost()
                                    .xsmall()
                                    .label("重连")
                                    .disabled(terminal_loading || !connection_available)
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            cx.stop_propagation();
                                            this.reconnect_terminal(
                                                reconnect_workspace_id.clone(),
                                                id,
                                                window,
                                                cx,
                                            );
                                        },
                                    )),
                            ),
                    )
                })
                .when(terminals_can_close, |tab| {
                    tab.child(
                        div()
                            .debug_selector(move || format!("ssh-terminal-close-{id_for_close}"))
                            .flex_none()
                            .child(
                                ramag_ui::clickable_button(("close-ssh-terminal", id_for_close))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .tooltip("关闭")
                                    .on_click(cx.listener({
                                        let workspace_id = workspace_id.clone();
                                        move |this, _: &ClickEvent, window, cx| {
                                            cx.stop_propagation();
                                            this.close_terminal(
                                                workspace_id.clone(),
                                                id_for_close,
                                                window,
                                                cx,
                                            );
                                        }
                                    })),
                            ),
                    )
                });
            if selected {
                let mut active_bg = accent;
                active_bg.a = 0.15;
                tab = tab.bg(active_bg);
            } else {
                tab = tab.hover(move |tab| tab.bg(muted_bg));
            }
            tabs_strip = tabs_strip.child(tab);
        }
        tabs_strip = tabs_strip.child(
            ramag_ui::clickable_button("new-ssh-terminal")
                .ghost()
                .small()
                .icon(IconName::Plus)
                .tooltip("新建")
                .disabled(terminal_loading || terminal_limit_reached || !connection_available)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.start_active_terminal(window, cx);
                })),
        );
        let tabs = h_flex()
            .w_full()
            .flex_none()
            .border_b_1()
            .border_color(border)
            .bg(secondary)
            .child(tabs_strip);

        let empty_workspace_id = workspace_id.clone();
        let body = terminal_views
            .iter()
            .find(|(id, _, _)| Some(*id) == active_terminal_id)
            .map(|(_, _, terminal)| terminal.clone().into_any_element())
            .unwrap_or_else(|| {
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if terminal_loading {
                                "连接中…"
                            } else {
                                "未连接"
                            }),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("close-empty-ssh-workspace")
                                    .debug_selector(|| "close-empty-ssh-workspace".into())
                                    .child(
                                        ramag_ui::clickable_button(
                                            "close-empty-ssh-workspace-button",
                                        )
                                        .outline()
                                        .icon(IconName::Close)
                                        .label("关闭连接")
                                        .on_click(
                                            cx.listener(move |this, _: &ClickEvent, window, cx| {
                                                this.request_close_workspace(
                                                    empty_workspace_id.clone(),
                                                    window,
                                                    cx,
                                                );
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                ramag_ui::clickable_button("connect-restored-ssh")
                                    .primary()
                                    .icon(IconName::SquareTerminal)
                                    .label("连接")
                                    .disabled(terminal_loading || !connection_available)
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.connect_active_workspace(window, cx);
                                    })),
                            ),
                    )
                    .into_any_element()
            });
        let can_drop_workspace = workspace_id.clone();
        let style_workspace = workspace_id.clone();
        let dropped_workspace = workspace_id.clone();
        v_flex()
            .id("ssh-terminal-drop-target")
            .debug_selector(|| "ssh-terminal-drop-target".into())
            .flex_1()
            .min_w_0()
            .h_full()
            .can_drop(move |value, _, _| {
                value
                    .downcast_ref::<RemoteDirectoryDrag>()
                    .is_some_and(|drag| drag.workspace_id == can_drop_workspace)
            })
            .drag_over(move |style, drag: &RemoteDirectoryDrag, _, _| {
                if drag.workspace_id == style_workspace {
                    style.border_2().border_color(accent).bg(drop_background)
                } else {
                    style
                }
            })
            .on_drop(
                cx.listener(move |this, drag: &RemoteDirectoryDrag, window, cx| {
                    if drag.workspace_id == dropped_workspace {
                        this.start_terminal_in_directory(
                            dropped_workspace.clone(),
                            drag.path.clone(),
                            window,
                            cx,
                        );
                    }
                }),
            )
            .child(tabs)
            .child(self.render_port_forwarding_panel(workspace_id.clone(), cx))
            .when(production, |pane| {
                pane.child(
                    h_flex().w_full().flex_none().px(px(8.0)).py(px(3.0)).child(
                        div()
                            .id("ssh-production-terminal-warning")
                            .debug_selector(|| "ssh-production-terminal-warning".into())
                            .flex_none()
                            .text_xs()
                            .text_color(warning)
                            .child("终端不受生产只读限制，请谨慎操作。"),
                    ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .bg(cx.theme().background)
                    .child(body),
            )
            .into_any_element()
    }
}

fn empty_directory_message(workspace: &SshWorkspace) -> &'static str {
    let windows = workspace.profile.remote_platform == RemotePlatformPreference::Windows
        || workspace.capabilities.as_ref().is_some_and(|capabilities| {
            capabilities.operating_system == RemoteOperatingSystem::Windows
        });
    if workspace.directory_loaded
        && workspace.profile.origin == SshProfileOrigin::JumpServer
        && windows
        && workspace.path == "/"
    {
        "未返回可访问盘符"
    } else {
        "目录为空"
    }
}

#[cfg(test)]
mod tests;
