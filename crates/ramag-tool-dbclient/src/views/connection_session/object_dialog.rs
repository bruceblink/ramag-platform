//! 在窄窗口通过对话框打开当前会话的对象树。

use gpui_kit::component::{
    IconName, Sizable as _, WindowExt as _, button::ButtonVariants as _, dialog::DialogEntrance,
    h_flex,
};
use gpui_kit::{Context, InteractiveElement as _, ParentElement, Styled, Window, div, px};

use super::ConnectionSession;
use crate::views::is_compact_session_width;

impl ConnectionSession {
    pub(super) fn open_object_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.object_dialog_open
            || !is_compact_session_width(f32::from(window.viewport_size().width))
            || window.has_active_dialog(cx)
        {
            return;
        }

        self.object_dialog_open = true;
        let tree = self.tree.clone();
        let session = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let tree = tree.clone();
            let session = session.clone();
            let limit = ramag_ui::responsive_dialog_max_height(window);
            dialog
                .title(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child("数据库对象")
                        .child(
                            ramag_ui::clickable_button("database-object-dialog-close")
                                .debug_selector(|| "database-object-dialog-close".into())
                                .ghost()
                                .small()
                                .icon(IconName::Close)
                                .accessibility_label("关闭数据库对象")
                                .on_click({
                                    let session = session.clone();
                                    move |_, window, cx| {
                                        let _ = session.update(cx, |session, cx| {
                                            session.close_object_dialog(window, cx);
                                            session.restore_wide_focus(window, cx);
                                        });
                                    }
                                }),
                        ),
                )
                .close_button(false)
                .entrance(DialogEntrance::None)
                .centered()
                .width(ramag_ui::responsive_dialog_width(window, 560.0))
                .h(px(560.0).min(limit))
                .max_h(limit)
                .on_close(move |_, window, cx| {
                    let _ = session.update(cx, |session, cx| {
                        session.object_dialog_open = false;
                        session.restore_wide_focus(window, cx);
                        cx.notify();
                    });
                })
                .content(move |content, _, _| {
                    content.min_h_0().px_0().child(
                        div()
                            .debug_selector(|| "database-object-dialog-tree".into())
                            .size_full()
                            .min_h_0()
                            .child(tree.clone()),
                    )
                })
        });
        cx.notify();
    }

    /// 先关闭对象树，再将焦点交给结果区或该操作打开的对话框。
    pub(super) fn close_object_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.object_dialog_open) {
            window.close_dialog(cx);
            cx.notify();
        }
    }

    /// 放大窗口后，“对象”按钮已卸载，关闭时由桌面会话接收焦点。
    fn restore_wide_focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !is_compact_session_width(f32::from(window.viewport_size().width)) {
            cx.on_next_frame(window, |session, window, cx| session.focus(window, cx));
        }
    }
}
