use super::*;

impl KafkaView {
    pub(super) fn render_main(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let compact = kafka_main_content_width(window) < 700.0;
        let narrow = kafka_sidebar_is_narrow(window);
        let selected = self.selected_config();
        let title = selected
            .as_ref()
            .map(|config| config.name.clone())
            .unwrap_or_else(|| "新建 Kafka 集群".into());
        let status = if self.loading_runtime {
            ("同步中", ramag_ui::pulse_ui::PulseStatus::Warming)
        } else if self.runtime_error.is_some() {
            ("同步失败", ramag_ui::pulse_ui::PulseStatus::Failed)
        } else if self.metadata.is_some() {
            ("已连接", ramag_ui::pulse_ui::PulseStatus::Current)
        } else {
            ("未连接", ramag_ui::pulse_ui::PulseStatus::Unavailable)
        };
        let admin_mode = if self.read_only.allows_admin() {
            ("管理已启用", ramag_ui::pulse_ui::PulseStatus::Warming)
        } else {
            ("只读", ramag_ui::pulse_ui::PulseStatus::Unavailable)
        };
        // 新建草稿尚未分配集群 ID，但必须先显示配置表单；只有初始概览才显示欢迎页。
        let show_welcome = selected.is_none() && self.section != KafkaSection::Config;
        let body = if show_welcome {
            self.render_welcome(window, cx).into_any_element()
        } else {
            self.render_workspace(window, cx).into_any_element()
        };

        v_flex()
            .id("kafka-main")
            .debug_selector(|| "kafka-main".into())
            .flex_1()
            .h_full()
            .min_w_0()
            .min_h_0()
            .bg(theme.background)
            .child(
                ramag_ui::pulse_ui::pulse_entry_header(cx)
                    .debug_selector(|| "kafka-header".into())
                    .min_h(px(58.0))
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .when(compact, |row| {
                        row.min_h(px(106.0))
                            .flex_col()
                            .items_stretch()
                            .justify_start()
                            .gap(px(8.0))
                            .px(px(14.0))
                            .py(px(10.0))
                    })
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .debug_selector(|| "kafka-header-status".into())
                            .flex_1()
                            .min_w_0()
                            .when(compact, |row| row.w_full().flex_none())
                            .gap(px(10.0))
                            .child(
                                ramag_ui::pulse_ui::pulse_page_title(
                                    title,
                                    Some("Kafka 工作区"),
                                    cx,
                                )
                                .id("kafka-page-title")
                                .debug_selector(|| "kafka-page-title".into())
                                .flex_1()
                                .min_w_0(),
                            )
                            .child(
                                ramag_ui::pulse_ui::pulse_status_badge_with_label(
                                    status.1, status.0, cx,
                                )
                                .debug_selector(|| "kafka-header-status-badge".into()),
                            ),
                    )
                    .child(
                        h_flex()
                            .debug_selector(|| "kafka-header-actions".into())
                            .flex_none()
                            .items_center()
                            .gap(px(8.0))
                            .when(compact, |row| row.w_full().justify_between())
                            .child(ramag_ui::pulse_ui::pulse_status_badge_with_label(
                                admin_mode.1,
                                admin_mode.0,
                                cx,
                            ))
                            .when(selected.is_some(), |row| {
                                row.child(
                                    ramag_ui::clickable_button("kafka-test-connection")
                                        .outline()
                                        .small()
                                        .icon(IconName::CircleCheck)
                                        .label("测试连接")
                                        .disabled(
                                            self.testing
                                                || self.saving
                                                || self.deleting
                                                || self.topic_operation
                                                || self.acl_operation,
                                        )
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, window, cx| {
                                                this.test_connection(window, cx);
                                            },
                                        )),
                                )
                            })
                            .child(
                                ramag_ui::clickable_button("kafka-refresh")
                                    .ghost()
                                    .small()
                                    .icon(ramag_ui::icons::refresh_cw())
                                    .accessibility_label("刷新元数据")
                                    .tooltip("刷新元数据")
                                    .disabled(
                                        self.loading_runtime
                                            || selected.is_none()
                                            || self.testing
                                            || self.saving
                                            || self.deleting
                                            || self.topic_operation
                                            || self.acl_operation,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.retry_runtime(window, cx);
                                    })),
                            )
                            .when(narrow, |row| row.child(self.cluster_dialog_button(cx))),
                    ),
            )
            .when_some(self.notice.clone(), |view, notice| {
                view.child(self.render_notice(notice, cx))
            })
            .child(body)
    }

    pub(super) fn render_notice(
        &self,
        notice: (String, bool),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (message, is_error) = notice;
        ramag_ui::pulse_ui::pulse_status_notice(
            if is_error {
                ramag_ui::pulse_ui::PulseStatus::Failed
            } else {
                ramag_ui::pulse_ui::PulseStatus::Current
            },
            message,
            cx,
        )
        .id("kafka-notice")
        .w_full()
        .flex_none()
        .px(px(22.0))
        .py(px(8.0))
    }
}
