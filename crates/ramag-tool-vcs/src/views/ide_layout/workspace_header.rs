use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::{
    AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, prelude::*, px,
};

use super::super::vcs_view::VcsView;

impl VcsView {
    /// 显示当前仓库、分支、同步计数和 Git 操作状态，保持工作区页头的层级稳定。
    pub(super) fn render_workspace_header(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let Some(repo) = self.repo.as_ref() else {
            return div().into_any_element();
        };
        let branch = self
            .status
            .as_ref()
            .and_then(|status| status.head_branch.clone())
            .unwrap_or_else(|| "已分离 HEAD".into());
        let changes = self
            .status
            .as_ref()
            .map(|status| status.files.len())
            .unwrap_or_default();
        let operation = self
            .status
            .as_ref()
            .and_then(|status| status.operation)
            .map(super::super::helpers::operation_label);
        let sync = self.status.as_ref().and_then(|status| {
            match (
                status.ahead.unwrap_or_default(),
                status.behind.unwrap_or_default(),
            ) {
                (0, 0) => None,
                (ahead, behind) => Some(format!("↑{ahead} ↓{behind}")),
            }
        });
        let title = super::super::inline_text_preview(&repo.name, 80);
        let title_width = if f32::from(window.viewport_size().width) < 720.0 {
            150.0
        } else {
            240.0
        };
        h_flex()
            .id("vcs-session-header")
            .debug_selector(|| "vcs-session-header".into())
            .w_full()
            .min_w_0()
            .h(px(ramag_ui::WORKBENCH_TOOLBAR_HEIGHT))
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .border_b_1()
            .border_color(theme.border.opacity(0.65))
            .bg(theme.background)
            .child(
                div()
                    .id("vcs-session-page-title")
                    .debug_selector(|| "vcs-session-page-title".into())
                    .min_w_0()
                    .max_w(px(title_width))
                    .flex_1()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .child(title),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("分支 {branch}")),
            )
            .when_some(sync, |header, sync| {
                header.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(sync),
                )
            })
            .child(
                div()
                    .text_xs()
                    .text_color(if operation.is_some() {
                        theme.warning
                    } else {
                        theme.muted_foreground
                    })
                    .child(operation.map_or_else(|| format!("{changes} 项变更"), |op| op.into())),
            )
            .into_any_element()
    }
}
