use super::*;

impl KafkaView {
    /// 检查消息生产的页面上下文；真正的只读保护仍由 `KafkaService` 再次执行。
    fn ensure_message_produce_ready(&mut self, cx: &mut Context<Self>) -> bool {
        if self.selected_cluster_id.is_none() {
            self.notice = Some(("请先选择已保存的 Kafka 集群".into(), true));
            cx.notify();
            return false;
        }
        if !self.read_only.allows_admin() {
            self.notice = Some((
                format!(
                    "{}；请先在配置页开启消息生产管理模式",
                    ramag_domain::error::READ_ONLY_MESSAGE
                ),
                true,
            ));
            cx.notify();
            return false;
        }
        if self.producing
            || self.loading_runtime
            || self.testing
            || self.saving
            || self.deleting
            || self.topic_operation
            || self.acl_operation
            || self.consumer_group_operation
            || self.updating_config
        {
            return false;
        }
        true
    }

    /// 从消息生产表单构造一个有界请求；Value 作为 UTF-8 文本转成原始字节。
    fn build_message_produce_request(
        &self,
        cx: &App,
    ) -> std::result::Result<KafkaMessageProduceRequest, String> {
        let partition = match value(&self.produce_partition_input, cx).as_str() {
            "" => None,
            text => Some(
                text.parse::<i32>()
                    .map_err(|_| "生产消息 Partition 必须是非负整数".to_string())?,
            ),
        };
        let key_text = self.produce_key_input.read(cx).value().to_string();
        let value_text = self.produce_value_input.read(cx).value().to_string();
        let key = (!key_text.is_empty()).then_some(key_text.into_bytes());
        let request = KafkaMessageProduceRequest::new(
            value(&self.produce_topic_input, cx),
            value_text.into_bytes(),
        );
        let request = match partition {
            Some(partition) => request.with_partition(partition),
            None => request,
        };
        let request = match key {
            Some(key) => request.with_key(key),
            None => request,
        };
        request.validate().map(|()| request)
    }

    /// 打开发送确认对话框；确认回调之前不会触发任何 Kafka 写请求。
    pub(super) fn begin_message_produce(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ensure_message_produce_ready(cx) {
            return;
        }
        let config = match self.form_config(cx) {
            Ok(config) => config,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let request = match self.build_message_produce_request(cx) {
            Ok(request) => request,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let partition = request.partition.map_or_else(
            || "由 Broker 分配".into(),
            |partition| partition.to_string(),
        );
        let key = request.key.as_ref().map_or_else(
            || "未设置".to_string(),
            |key| format!("已设置（{} bytes）", key.len()),
        );
        let value_preview =
            ramag_domain::entities::preview_bytes(&request.value, MESSAGE_PREVIEW_BYTES);
        let value_summary = if value_preview.text.is_empty() {
            "<空>".to_string()
        } else {
            value_preview.text
        };
        let description = format!(
            "集群「{}」的 Topic「{}」将写入一条消息。Partition：{}；Key：{}；Value 摘要：{}（{} bytes）。确认后才会调用 Kafka Producer。",
            config.name,
            request.topic,
            partition,
            key,
            value_summary,
            request.value.len(),
        );
        let view = cx.entity();
        ramag_ui::open_confirm(
            "发送 Kafka 消息？",
            description,
            "发送",
            true,
            move |window, app| {
                view.update(app, |this, cx| {
                    this.execute_message_produce(config, request, window, cx);
                });
            },
            window,
            cx,
        );
    }

    fn execute_message_produce(
        &mut self,
        config: KafkaClusterConfig,
        request: KafkaMessageProduceRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.producing {
            return;
        }
        self.producing = true;
        self.produce_operation_id = self.produce_operation_id.wrapping_add(1);
        let operation_id = self.produce_operation_id;
        let cluster_id = config.id.clone();
        let topic = request.topic.clone();
        let service = self.service.clone();
        self.notice = Some((format!("正在向 Topic「{topic}」发送消息…"), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service.produce_message(&config, &request).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.produce_operation_id != operation_id
                    || this.selected_cluster_id.as_ref() != Some(&cluster_id)
                {
                    return;
                }
                this.producing = false;
                match result {
                    Ok(result) => {
                        let timestamp = result.timestamp.map_or_else(
                            || "Timestamp 未返回".to_string(),
                            |timestamp| timestamp.to_rfc3339(),
                        );
                        this.notice = Some((
                            format!(
                                "消息已写入 Topic「{}」；Partition {}，Offset {}，{}",
                                result.topic, result.partition, result.offset, timestamp
                            ),
                            false,
                        ));
                        set_value(&this.produce_partition_input, "", window, cx);
                        set_value(&this.produce_key_input, "", window, cx);
                        set_value(&this.produce_value_input, "", window, cx);
                    }
                    Err(error) => {
                        this.notice = Some((
                            format!("发送 Kafka 消息失败：{}", error.user_message()),
                            true,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 使已提交的消息生产回调失效；底层写请求自然结束后不会回写旧视图。
    pub(super) fn invalidate_produce_operation(&mut self) {
        self.produce_operation_id = self.produce_operation_id.wrapping_add(1);
        self.producing = false;
    }

    pub(super) fn render_message_producer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let compact = kafka_main_content_width(window) < 760.0;
        let input_disabled = self.producing;
        let action_disabled = self.selected_cluster_id.is_none()
            || !self.read_only.allows_admin()
            || self.producing
            || self.loading_runtime
            || self.testing
            || self.saving
            || self.deleting
            || self.topic_operation
            || self.acl_operation
            || self.consumer_group_operation
            || self.updating_config;
        let target_row = h_flex()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_end()
            .gap(px(8.0))
            .when(compact, |row| row.flex_col().items_stretch())
            .child(field(
                "目标 Topic",
                Input::new(&self.produce_topic_input)
                    .small()
                    .disabled(input_disabled),
                if compact { 0.0 } else { 280.0 },
            ))
            .child(field(
                "Partition（可选）",
                Input::new(&self.produce_partition_input)
                    .small()
                    .disabled(input_disabled),
                if compact { 0.0 } else { 210.0 },
            ));
        let payload_row = h_flex()
            .w_full()
            .min_w_0()
            .items_end()
            .gap(px(8.0))
            .when(compact, |row| row.flex_col().items_stretch())
            .child(flexible_field(
                "Key（可选）",
                Input::new(&self.produce_key_input)
                    .small()
                    .disabled(input_disabled),
            ))
            .child(flexible_field(
                "Value（UTF-8）",
                Input::new(&self.produce_value_input)
                    .small()
                    .disabled(input_disabled),
            ));
        let mode_copy = if self.read_only.allows_admin() {
            "管理模式：点击发送后还需要确认；本阶段只提交一条 UTF-8 消息"
        } else {
            "只读模式：消息生产不可用"
        };
        v_flex()
            .id("kafka-message-producer")
            .debug_selector(|| "kafka-message-producer".into())
            .w_full()
            .flex_none()
            .gap(px(8.0))
            .p(px(12.0))
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .child(section_heading(
                "发送一条消息",
                "单条 Kafka Producer 工作流；成功后显示 Broker 返回的 Partition、Offset 和 Timestamp",
                &theme,
            ))
            .child(target_row)
            .child(payload_row)
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id("kafka-message-producer-mode")
                            .debug_selector(|| "kafka-message-producer-mode".into())
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(if self.read_only.allows_admin() {
                                theme.muted_foreground
                            } else {
                                theme.warning
                            })
                            .child(mode_copy),
                    )
                    .child(
                        ramag_ui::clickable_button("kafka-produce-message")
                            .debug_selector(|| "kafka-produce-message".into())
                            .primary()
                            .small()
                            .icon(IconName::Play)
                            .label("发送")
                            .loading(self.producing)
                            .disabled(action_disabled)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.begin_message_produce(window, cx);
                            })),
                    ),
            )
    }
}
