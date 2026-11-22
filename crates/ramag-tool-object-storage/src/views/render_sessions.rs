use gpui_kit::component::{
    ActiveTheme, IconName, Sizable as _, button::ButtonVariants as _, h_flex,
    scroll::ScrollableElement as _,
};
use gpui_kit::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::model::{AccountSessionState, ObjectStorageView};
use ramag_ui::{WORKBENCH_TAB_HEIGHT, pulse_ui::pulse_palette};

impl ObjectStorageView {
    pub(super) fn render_sessions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let palette = pulse_palette(cx);
        let border = palette.border;
        let foreground = palette.text;
        let muted = theme.muted_foreground;
        let selected_bg = palette.selected;
        let hover_bg = palette.raised;
        let manager_selected = self.management_visible;
        let mut manager_tab = h_flex()
            .id("object-account-management")
            .flex_none()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(WORKBENCH_TAB_HEIGHT))
            .border_r_1()
            .border_color(border)
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.show_account_management(cx);
            }))
            .child(
                ramag_ui::icons::cloud()
                    .small()
                    .text_color(if manager_selected { foreground } else { muted }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if manager_selected { foreground } else { muted })
                    .child("云存储"),
            );
        if manager_selected {
            manager_tab = manager_tab.bg(selected_bg);
        } else {
            manager_tab = manager_tab.hover(move |tab| tab.bg(hover_bg));
        }

        let mut sessions = h_flex()
            .id("object-workspace-tabs-scroll")
            .flex_1()
            .min_w_0()
            .overflow_x_scrollbar();
        for id in &self.open_account_ids {
            let Some(account) = self.accounts.iter().find(|account| &account.id == id) else {
                continue;
            };
            let selected = !manager_selected && self.selected_account_id.as_ref() == Some(id);
            let account_id = id.clone();
            let close_id = id.clone();
            let dot_color = account_session_state_color(
                self.account_session_states.get(id).copied(),
                theme.warning,
                theme.success,
                muted,
            );
            let mut tab = h_flex()
                .id(SharedString::from(format!("object-session-{id}")))
                .flex_none()
                .items_center()
                .gap_2()
                .px_3()
                .h(px(WORKBENCH_TAB_HEIGHT))
                .border_r_1()
                .border_color(border)
                .cursor_pointer()
                .child(div().size(px(8.0)).rounded_full().bg(dot_color))
                .child(
                    div()
                        .text_xs()
                        .text_color(if selected { foreground } else { muted })
                        .child(account.name.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(account.provider.display_name()),
                )
                .when(account.read_only, |tab| {
                    let mut chip_bg = theme.danger;
                    chip_bg.a = 0.15;
                    tab.child(
                        div()
                            .px(px(6.0))
                            .py(px(1.0))
                            .rounded(px(4.0))
                            .bg(chip_bg)
                            .text_xs()
                            .text_color(theme.danger)
                            .child(ramag_ui::PRODUCTION_BADGE_LABEL),
                    )
                })
                .child(
                    ramag_ui::clickable_button(SharedString::from(format!(
                        "close-object-session-{id}"
                    )))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .tooltip("关闭")
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.close_session(close_id.clone(), window, cx);
                        },
                    )),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.select_account(account_id.clone(), window, cx);
                }));
            if selected {
                tab = tab.bg(selected_bg);
            } else {
                tab = tab.hover(move |tab| tab.bg(hover_bg));
            }
            sessions = sessions.child(tab);
        }

        h_flex()
            .id("object-workspace-tabs")
            .w_full()
            .flex_none()
            .border_b_1()
            .border_color(border)
            .bg(palette.surface)
            .child(manager_tab)
            .child(sessions)
    }
}

fn account_session_state_color(
    state: Option<AccountSessionState>,
    warning: gpui_kit::Hsla,
    success: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
) -> gpui_kit::Hsla {
    match state {
        Some(AccountSessionState::Loading | AccountSessionState::Unverified) => warning,
        Some(AccountSessionState::Configured) => success,
        None => muted,
    }
}

#[cfg(test)]
mod tests {
    use super::account_session_state_color;
    use crate::views::model::AccountSessionState;

    #[test]
    fn account_session_dots_follow_theme_status_colors() {
        let warning = gpui_kit::hsla(0.1, 0.2, 0.3, 1.0);
        let success = gpui_kit::hsla(0.2, 0.3, 0.4, 1.0);
        let muted = gpui_kit::hsla(0.3, 0.4, 0.5, 1.0);

        assert_eq!(
            account_session_state_color(
                Some(AccountSessionState::Loading),
                warning,
                success,
                muted
            ),
            warning
        );
        assert_eq!(
            account_session_state_color(
                Some(AccountSessionState::Configured),
                warning,
                success,
                muted,
            ),
            success
        );
        assert_eq!(
            account_session_state_color(
                Some(AccountSessionState::Unverified),
                warning,
                success,
                muted,
            ),
            warning
        );
        assert_eq!(
            account_session_state_color(None, warning, success, muted),
            muted
        );
    }
}
