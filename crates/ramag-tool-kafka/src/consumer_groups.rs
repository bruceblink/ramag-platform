use super::*;

impl KafkaView {
    /// 触发一次有界消费者组快照读取；旧请求的结果不会覆盖当前集群。
    pub(super) fn load_consumer_groups(
        &mut self,
        config: KafkaClusterConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let restart_metrics = self.metrics_refresh_cancelled.is_some();
        self.load_consumer_groups_with_metrics(config, window, cx, restart_metrics);
    }

    /// 在运行时快照之后先读取详细消费者组数据，再启动指标刷新，避免两条大查询链重叠。
    pub(super) fn load_consumer_groups_with_metrics(
        &mut self,
        config: KafkaClusterConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
        restart_metrics: bool,
    ) {
        if self.loading_consumer_groups || self.selected_cluster_id.as_ref() != Some(&config.id) {
            return;
        }
        if restart_metrics {
            self.invalidate_metrics_refresh();
        }
        self.consumer_group_request_id = self.consumer_group_request_id.wrapping_add(1);
        let request_id = self.consumer_group_request_id;
        let cluster_id = config.id.clone();
        let metrics_config = config.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let service = self.service.clone();
        self.consumer_group_cancelled = Some(cancelled.clone());
        self.loading_consumer_groups = true;
        self.consumer_group_error = None;
        self.notice = Some(("正在读取 Kafka 消费者组…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .list_consumer_groups_with_cancel(&config, cancelled)
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.consumer_group_request_id != request_id
                    || this.selected_cluster_id.as_ref() != Some(&cluster_id)
                {
                    return;
                }
                this.loading_consumer_groups = false;
                this.consumer_group_cancelled = None;
                match result {
                    Ok(groups) => {
                        let selected = this.selected_consumer_group.clone().filter(|group_id| {
                            groups.iter().any(|group| &group.group_id == group_id)
                        });
                        let count = groups.len();
                        this.consumer_groups = groups;
                        this.selected_consumer_group = selected.or_else(|| {
                            this.consumer_groups
                                .first()
                                .map(|group| group.group_id.clone())
                        });
                        this.consumer_group_error = None;
                        this.notice = Some((format!("已读取 {count} 个消费者组"), false));
                    }
                    Err(error) => {
                        this.consumer_groups.clear();
                        this.selected_consumer_group = None;
                        this.consumer_group_error = Some(error.user_message());
                        this.mark_runtime_failure("读取消费者组", &error);
                        this.notice =
                            Some((format!("读取消费者组失败：{}", error.user_message()), true));
                    }
                }
                if restart_metrics {
                    this.start_metrics_refresh(metrics_config.clone(), window, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 使当前消费者组任务失效，刷新或切换集群时丢弃迟到结果。
    pub(super) fn invalidate_consumer_group_request(&mut self) {
        self.consumer_group_request_id = self.consumer_group_request_id.wrapping_add(1);
        self.loading_consumer_groups = false;
        if let Some(cancelled) = self.consumer_group_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
    }

    pub(super) fn select_consumer_group(&mut self, group_id: String, cx: &mut Context<Self>) {
        if self
            .consumer_groups
            .iter()
            .any(|group| group.group_id == group_id)
        {
            self.selected_consumer_group = Some(group_id);
            cx.notify();
        }
    }
}
