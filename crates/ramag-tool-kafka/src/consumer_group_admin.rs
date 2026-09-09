use super::*;

#[derive(Clone, Copy)]
pub(super) enum ConsumerGroupOffsetResetTarget {
    Earliest,
    Latest,
}

impl ConsumerGroupOffsetResetTarget {
    const fn label(self) -> &'static str {
        match self {
            Self::Earliest => "最早 Offset",
            Self::Latest => "当前末尾 Offset",
        }
    }

    const fn action_label(self) -> &'static str {
        match self {
            Self::Earliest => "重置到最早",
            Self::Latest => "重置到末尾",
        }
    }
}

impl KafkaView {
    /// 让已提交的消费者组 Offset 写入在切换集群或销毁页面后不能回写旧视图。
    pub(super) fn invalidate_consumer_group_operation(&mut self) {
        self.consumer_group_operation_id = self.consumer_group_operation_id.wrapping_add(1);
        self.consumer_group_operation = false;
    }

    pub(super) fn begin_reset_consumer_group_offsets(
        &mut self,
        target: ConsumerGroupOffsetResetTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.ensure_consumer_group_admin_ready(cx) {
            return;
        }
        let Some(group) = self.selected_consumer_group_snapshot() else {
            self.notice = Some(("请先选择要修改的消费者组".into(), true));
            cx.notify();
            return;
        };
        let config = match self.form_config(cx) {
            Ok(config) => config,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let request = match build_offset_reset_request(&group, target) {
            Ok(request) => request,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let group_state = group.state.as_deref().unwrap_or("未知");
        let view = cx.entity();
        let description = format!(
            "集群「{}」中的消费者组「{}」将修改 {} 个 Topic/Partition 的已提交 Offset，目标为{}。当前状态：{}。请先停止该组的消费者；确认后才会调用 Kafka Admin API，执行完成后页面会重新读取快照。",
            config.name,
            request.group_id,
            request.offsets.len(),
            target.label(),
            group_state,
        );
        ramag_ui::open_confirm(
            format!("{}？", target.action_label()),
            description,
            target.action_label(),
            true,
            move |window, app| {
                view.update(app, |this, cx| {
                    this.execute_reset_consumer_group_offsets(config, request, window, cx);
                });
            },
            window,
            cx,
        );
    }

    fn ensure_consumer_group_admin_ready(&mut self, cx: &mut Context<Self>) -> bool {
        if self.selected_cluster_id.is_none() {
            self.notice = Some(("请先选择已保存的 Kafka 集群".into(), true));
            cx.notify();
            return false;
        }
        if !self.read_only.allows_admin() {
            self.notice = Some((
                format!(
                    "{}；请先在配置页开启消费者组管理模式",
                    ramag_domain::error::READ_ONLY_MESSAGE
                ),
                true,
            ));
            cx.notify();
            return false;
        }
        if self.consumer_group_operation
            || self.loading_consumer_groups
            || self.loading_runtime
            || self.topic_operation
            || self.acl_operation
        {
            return false;
        }
        true
    }

    fn selected_consumer_group_snapshot(&self) -> Option<KafkaConsumerGroup> {
        self.selected_consumer_group.as_ref().and_then(|group_id| {
            self.consumer_groups
                .iter()
                .find(|group| &group.group_id == group_id)
                .cloned()
        })
    }

    fn execute_reset_consumer_group_offsets(
        &mut self,
        config: KafkaClusterConfig,
        request: KafkaConsumerGroupOffsetResetRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.consumer_group_operation {
            return;
        }
        self.consumer_group_operation_id = self.consumer_group_operation_id.wrapping_add(1);
        let operation_id = self.consumer_group_operation_id;
        let cluster_id = config.id.clone();
        let group_id = request.group_id.clone();
        let count = request.offsets.len();
        let service = self.service.clone();
        self.consumer_group_operation = true;
        self.notice = Some((
            format!("正在修改消费者组「{group_id}」的 {count} 个 Offset…"),
            false,
        ));
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .reset_consumer_group_offsets(&config, &request)
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.consumer_group_operation_id != operation_id
                    || this.selected_cluster_id.as_ref() != Some(&cluster_id)
                {
                    return;
                }
                this.consumer_group_operation = false;
                match result {
                    Ok(()) => {
                        this.notice = Some((
                            format!("消费者组「{group_id}」已修改 {count} 个 Offset，正在刷新快照"),
                            false,
                        ));
                        if let Some(config) = this.selected_config() {
                            this.load_consumer_groups(config, window, cx);
                        }
                    }
                    Err(error) => {
                        this.notice = Some((
                            format!("修改消费者组 Offset 失败：{}", error.user_message()),
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

    pub(super) fn render_consumer_group_reset_actions(
        &self,
        reset_disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        ramag_ui::responsive_toolbar()
            .child(
                ramag_ui::clickable_button("kafka-consumer-group-reset-earliest")
                    .debug_selector(|| "kafka-consumer-group-reset-earliest".into())
                    .outline()
                    .small()
                    .icon(IconName::ChevronLeft)
                    .label("重置到最早")
                    .disabled(reset_disabled)
                    .tooltip("把当前列出的消费者组 Offset 重置到最早位置")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.begin_reset_consumer_group_offsets(
                            ConsumerGroupOffsetResetTarget::Earliest,
                            window,
                            cx,
                        );
                    })),
            )
            .child(
                ramag_ui::clickable_button("kafka-consumer-group-reset-latest")
                    .debug_selector(|| "kafka-consumer-group-reset-latest".into())
                    .outline()
                    .small()
                    .icon(IconName::ChevronRight)
                    .label("重置到末尾")
                    .disabled(reset_disabled)
                    .tooltip("把当前列出的消费者组 Offset 重置到当前末尾")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.begin_reset_consumer_group_offsets(
                            ConsumerGroupOffsetResetTarget::Latest,
                            window,
                            cx,
                        );
                    })),
            )
    }
}

fn build_offset_reset_request(
    group: &KafkaConsumerGroup,
    target: ConsumerGroupOffsetResetTarget,
) -> std::result::Result<KafkaConsumerGroupOffsetResetRequest, String> {
    let offsets = group
        .offsets
        .iter()
        .map(|offset| {
            let target_offset = match target {
                ConsumerGroupOffsetResetTarget::Earliest => 0,
                ConsumerGroupOffsetResetTarget::Latest => offset.end_offset.ok_or_else(|| {
                    format!(
                        "{}/{} 缺少末尾 Offset，不能执行重置",
                        offset.topic, offset.partition
                    )
                })?,
            };
            Ok(KafkaConsumerGroupOffsetReset::new(
                offset.topic.clone(),
                offset.partition,
                target_offset,
            ))
        })
        .collect::<std::result::Result<Vec<_>, String>>()?;
    let request = KafkaConsumerGroupOffsetResetRequest::new(group.group_id.clone(), offsets);
    request.validate().map(|()| request)
}
