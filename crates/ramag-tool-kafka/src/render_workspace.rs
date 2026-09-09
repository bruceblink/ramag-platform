use super::*;

impl KafkaView {
    pub(super) fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        v_flex()
            .id("kafka-welcome")
            .size_full()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .child(Icon::new(IconName::Network).text_color(theme.accent))
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("开始浏览 Kafka"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("新建一个本地集群配置，连接后读取真实的 Broker、Topic 和消息"),
            )
            .child(
                ramag_ui::clickable_button("kafka-welcome-add")
                    .debug_selector(|| "kafka-welcome-add".into())
                    .primary()
                    .small()
                    .icon(IconName::Plus)
                    .label("新建集群配置")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.new_profile(window, cx);
                    })),
            )
    }

    pub(super) fn render_workspace(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let compact = kafka_main_content_width(window) < 700.0;
        let tabs = KafkaSection::ALL
            .into_iter()
            .fold(h_flex().gap(px(2.0)), |tabs, section| {
                let selected = self.section == section;
                tabs.child(
                    ramag_ui::clickable_button(SharedString::from(format!(
                        "kafka-section-{:?}",
                        section
                    )))
                    .debug_selector(move || format!("kafka-section-{:?}", section))
                    .small()
                    .label(section.label())
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, window, cx| {
                            this.section = section;
                            if section == KafkaSection::ConsumerGroups
                                && !this.loading_runtime
                                && let Some(config) = this.selected_config()
                            {
                                this.load_consumer_groups(config, window, cx);
                            }
                            if section == KafkaSection::SchemaRegistry
                                && !this.loading_runtime
                                && !this.schema_subjects_loaded
                                && let Some(config) = this.selected_config()
                            {
                                this.load_schema_subjects(config, window, cx);
                            }
                            if section == KafkaSection::Connect
                                && !this.loading_runtime
                                && !this.connectors_loaded
                                && let Some(config) = this.selected_config()
                            {
                                this.load_connectors(config, window, cx);
                            }
                            if section == KafkaSection::Acls
                                && !this.loading_runtime
                                && !this.acls_loaded
                                && let Some(config) = this.selected_config()
                            {
                                this.load_acls(config, window, cx);
                            }
                            if section == KafkaSection::Config
                                && this.selected_cluster_id.is_some()
                                && this.config_entries.is_empty()
                                && this.config_resource_name.read(cx).value().trim().is_empty()
                            {
                                set_value(
                                    &this.config_resource_name,
                                    this.selected_topic.clone().unwrap_or_default(),
                                    window,
                                    cx,
                                );
                            }
                            cx.notify();
                        },
                    )),
                )
            });
        let panel = match self.section {
            KafkaSection::Overview => self.render_overview(window, cx).into_any_element(),
            KafkaSection::Topics => self.render_topics(window, cx).into_any_element(),
            KafkaSection::Messages => self.render_messages(window, cx).into_any_element(),
            KafkaSection::ConsumerGroups => {
                self.render_consumer_groups(window, cx).into_any_element()
            }
            KafkaSection::SchemaRegistry => {
                self.render_schema_registry(window, cx).into_any_element()
            }
            KafkaSection::Connect => self.render_connect(window, cx).into_any_element(),
            KafkaSection::Acls => self.render_acls(window, cx).into_any_element(),
            KafkaSection::Config => self.render_config(window, cx).into_any_element(),
        };
        v_flex()
            .id("kafka-workspace")
            .flex_1()
            .min_h_0()
            .child(
                h_flex()
                    .debug_selector(|| "kafka-workspace-tabs".into())
                    .id("kafka-workspace-tabs")
                    .w_full()
                    .h(px(48.0))
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .px(px(22.0))
                    .when(compact, |row| {
                        row.h(px(112.0))
                            .flex_wrap()
                            .items_start()
                            .justify_start()
                            .gap(px(8.0))
                            .px(px(12.0))
                            .py(px(8.0))
                    })
                    .border_b_1()
                    .border_color(theme.border)
                    .child(tabs)
                    .when(self.section == KafkaSection::Config, |row| {
                        row.child(
                            h_flex()
                                .debug_selector(|| "kafka-config-actions".into())
                                .gap(px(8.0))
                                .when(compact, |row| row.w_full().justify_end())
                                .child(
                                    ramag_ui::clickable_button("kafka-save-profile")
                                        .primary()
                                        .small()
                                        .icon(IconName::Check)
                                        .label("保存")
                                        .disabled(
                                            self.saving
                                                || self.testing
                                                || self.deleting
                                                || self.acl_operation,
                                        )
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, window, cx| {
                                                this.save_profile(window, cx);
                                            },
                                        )),
                                )
                                .when(self.selected_cluster_id.is_some(), |row| {
                                    row.child(
                                        ramag_ui::clickable_button("kafka-delete-profile")
                                            .ghost()
                                            .small()
                                            .icon(IconName::Delete)
                                            .tooltip("删除本地配置")
                                            .disabled(
                                                self.saving
                                                    || self.testing
                                                    || self.deleting
                                                    || self.acl_operation,
                                            )
                                            .on_click(cx.listener(
                                                |this, _: &ClickEvent, window, cx| {
                                                    this.delete_profile(window, cx);
                                                },
                                            )),
                                    )
                                }),
                        )
                    }),
            )
            .child(panel)
    }
}
