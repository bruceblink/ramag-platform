use gpui_kit::component::{WindowExt as _, notification::Notification};
use gpui_kit::{AppContext as _, Context, ParentElement, Styled, Window, px};
use tracing::info;

use super::RedisSessionPanel;
use crate::views::key_create::{KeyCreateEvent, KeyCreateForm};

impl RedisSessionPanel {
    pub(super) fn open_create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let svc = self.service.clone();
        let config = self.config.clone();
        let db = self.db;
        let form = cx.new(|cx| KeyCreateForm::new(svc, config, db, window, cx));
        let tree_for_refresh = self.tree.clone();
        let sub = cx.subscribe_in(
            &form,
            window,
            move |this: &mut Self, _, ev: &KeyCreateEvent, window, cx| {
                this.clear_dialog_subscription();
                match ev {
                    KeyCreateEvent::Created { key, ttl_warning } => {
                        info!(
                            operation = "redis_key_create",
                            connection_id = %this.config.id,
                            db = this.db,
                            key_bytes = key.len(),
                            "key created via dialog"
                        );
                        let new_key = key.clone();
                        window.close_dialog(cx);
                        if let Some(warning) = ttl_warning {
                            ramag_ui::push_responsive_notification(
                                window,
                                Notification::warning(warning.clone())
                                    .title("Key 已创建，但 TTL 未按预期设置"),
                                cx,
                            );
                        }
                        tree_for_refresh.update(cx, |tree, cx| {
                            tree.refresh(cx);
                            tree.select_key_external(new_key.clone(), cx);
                        });
                    }
                    KeyCreateEvent::Cancelled => window.close_dialog(cx),
                }
            },
        );
        self.set_dialog_subscription(sub);

        let form_for_dialog = form.clone();
        let session_for_close = cx.entity().clone();
        window.open_dialog(cx, move |dialog, window, _app| {
            let form = form_for_dialog.clone();
            let form_for_cancel = form_for_dialog.clone();
            let session_for_close = session_for_close.clone();
            dialog
                .title("新建 Key")
                .close_button(false)
                .on_cancel(move |_, _, app| !form_for_cancel.read(app).is_submitting())
                .on_close(move |_, _, app| {
                    session_for_close.update(app, |this, _| this.clear_dialog_subscription());
                })
                .w(px(640.0))
                .max_h(ramag_ui::responsive_dialog_max_height(window))
                .margin_top(ramag_ui::responsive_dialog_top(window))
                .p(px(24.0))
                .content(move |content, window, _| {
                    let body_height = (ramag_ui::responsive_dialog_max_height(window) - px(96.0))
                        .clamp(px(32.0), px(540.0));
                    content.child(
                        gpui_kit::div()
                            .w_full()
                            .h(body_height)
                            .max_h(body_height)
                            .min_h_0()
                            .flex_none()
                            .overflow_hidden()
                            .child(form.clone()),
                    )
                })
        });
    }
}
