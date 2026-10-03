use super::*;

impl KafkaView {
    fn render_key_data_card(
        label: &'static str,
        value: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        status: ramag_ui::pulse_ui::PulseStatus,
        status_label: &'static str,
        theme: &gpui_kit::component::Theme,
        cx: &gpui_kit::App,
    ) -> gpui_kit::Div {
        v_flex()
            .flex_1()
            .min_w(px(148.0))
            .min_h(px(92.0))
            .gap(px(5.0))
            .p(px(12.0))
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .bg(theme.secondary.opacity(0.45))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap(px(6.0))
                    .child(
                        div()
                            .min_w_0()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(label),
                    )
                    .child(
                        ramag_ui::pulse_ui::pulse_status_badge_with_label(status, status_label, cx)
                            .flex_none(),
                    ),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .truncate()
                    .child(value.into()),
            )
            .child(
                div()
                    .min_w_0()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .whitespace_normal()
                    .child(detail.into()),
            )
    }

    pub(super) fn render_key_data_summary(
        &self,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let config = self.selected_config();
        let (topic_value, topic_detail, topic_status, topic_label) = if self.loading_runtime {
            (
                "读取中".to_owned(),
                "正在同步 Broker 元数据".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Warming,
                "同步中",
            )
        } else if self.topics_loaded {
            (
                self.topics.len().to_string(),
                "Broker Topic 快照".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Current,
                "已读取",
            )
        } else {
            (
                "未读取".to_owned(),
                "请刷新元数据".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "未读取",
            )
        };

        let (group_value, group_detail, group_status, group_label) = if self.loading_consumer_groups
        {
            (
                "读取中".to_owned(),
                "正在读取消费者组".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Warming,
                "读取中",
            )
        } else if self.consumer_group_error.is_some() {
            (
                "读取失败".to_owned(),
                "打开消费者组页签重试".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Failed,
                "失败",
            )
        } else if self.consumer_groups_loaded {
            (
                self.consumer_groups.len().to_string(),
                "Broker 消费者组快照".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Current,
                "已读取",
            )
        } else {
            (
                "未读取".to_owned(),
                "打开消费者组页签读取".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "未读取",
            )
        };

        let (schema_value, schema_detail, schema_status, schema_label) = if config
            .as_ref()
            .and_then(|config| config.schema_registry.endpoint.as_ref())
            .is_none()
        {
            (
                "未配置".to_owned(),
                "配置页未填写 Schema Registry".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "未配置",
            )
        } else if self.loading_schema_subjects {
            (
                "读取中".to_owned(),
                "正在读取 Schema Subject".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Warming,
                "读取中",
            )
        } else if self.schema_subject_error.is_some() {
            (
                "读取失败".to_owned(),
                "打开 Schema Registry 页签重试".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Failed,
                "失败",
            )
        } else if self.schema_subjects_loaded {
            (
                self.schema_subjects.len().to_string(),
                "Schema Subject 快照".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Current,
                "已读取",
            )
        } else {
            (
                "未读取".to_owned(),
                "打开 Schema Registry 页签读取".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "未读取",
            )
        };

        let (connect_value, connect_detail, connect_status, connect_label) = if config
            .as_ref()
            .and_then(|config| config.connect.endpoint.as_ref())
            .is_none()
        {
            (
                "未配置".to_owned(),
                "配置页未填写 Kafka Connect".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "未配置",
            )
        } else if self.loading_connectors {
            (
                "读取中".to_owned(),
                "正在读取连接器状态".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Warming,
                "读取中",
            )
        } else if self.connect_error.is_some() {
            (
                "读取失败".to_owned(),
                "打开 Kafka Connect 页签重试".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Failed,
                "失败",
            )
        } else if self.connectors_loaded {
            (
                self.connect_connectors.len().to_string(),
                "Kafka Connect 连接器快照".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Current,
                "已读取",
            )
        } else {
            (
                "未读取".to_owned(),
                "打开 Kafka Connect 页签读取".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "未读取",
            )
        };

        let (acl_value, acl_detail, acl_status, acl_label) = if self.loading_acls {
            (
                "读取中".to_owned(),
                "正在读取 Kafka ACL".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Warming,
                "读取中",
            )
        } else if self.acl_error.is_some() {
            (
                "读取失败".to_owned(),
                "打开 ACL 页签重试".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Failed,
                "失败",
            )
        } else if self.acls_loaded {
            (
                self.acls.len().to_string(),
                "Kafka ACL 规则快照".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Current,
                "已读取",
            )
        } else {
            (
                "未读取".to_owned(),
                "打开 ACL 页签读取".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "未读取",
            )
        };

        let (ksqldb_value, ksqldb_detail, ksqldb_status, ksqldb_label) = if config
            .as_ref()
            .and_then(|config| config.ksqldb.endpoint.as_ref())
            .is_some()
        {
            (
                "已配置".to_owned(),
                "ksqlDB 只读查询可用".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Current,
                "已配置",
            )
        } else {
            (
                "未配置".to_owned(),
                "配置页未填写 ksqlDB 地址".to_owned(),
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "未配置",
            )
        };

        v_flex()
            .id("kafka-overview-key-data")
            .debug_selector(|| "kafka-overview-key-data".into())
            .w_full()
            .min_w_0()
            .flex_none()
            .gap(px(10.0))
            .p(px(14.0))
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .child(section_heading(
                "关键数据",
                "集中显示已读取、未配置和待读取的 Broker 关联资源",
                theme,
            ))
            .child(
                h_flex()
                    .id("kafka-overview-key-data-grid")
                    .debug_selector(|| "kafka-overview-key-data-grid".into())
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_stretch()
                    .gap(px(10.0))
                    .child(
                        Self::render_key_data_card(
                            "Topics",
                            topic_value,
                            topic_detail,
                            topic_status,
                            topic_label,
                            theme,
                            cx,
                        )
                        .debug_selector(|| "kafka-overview-key-data-topics".into()),
                    )
                    .child(
                        Self::render_key_data_card(
                            "消费者组",
                            group_value,
                            group_detail,
                            group_status,
                            group_label,
                            theme,
                            cx,
                        )
                        .debug_selector(|| "kafka-overview-key-data-consumer-groups".into()),
                    )
                    .child(
                        Self::render_key_data_card(
                            "Schema Registry",
                            schema_value,
                            schema_detail,
                            schema_status,
                            schema_label,
                            theme,
                            cx,
                        )
                        .debug_selector(|| "kafka-overview-key-data-schema".into()),
                    )
                    .child(
                        Self::render_key_data_card(
                            "Kafka Connect",
                            connect_value,
                            connect_detail,
                            connect_status,
                            connect_label,
                            theme,
                            cx,
                        )
                        .debug_selector(|| "kafka-overview-key-data-connect".into()),
                    )
                    .child(
                        Self::render_key_data_card(
                            "ACL", acl_value, acl_detail, acl_status, acl_label, theme, cx,
                        )
                        .debug_selector(|| "kafka-overview-key-data-acls".into()),
                    )
                    .child(
                        Self::render_key_data_card(
                            "ksqlDB",
                            ksqldb_value,
                            ksqldb_detail,
                            ksqldb_status,
                            ksqldb_label,
                            theme,
                            cx,
                        )
                        .debug_selector(|| "kafka-overview-key-data-ksqldb".into()),
                    ),
            )
    }
}
