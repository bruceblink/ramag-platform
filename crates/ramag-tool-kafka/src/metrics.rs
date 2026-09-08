use std::sync::atomic::Ordering;
use std::time::Duration;

use super::*;
use ramag_domain::error::Result as DomainResult;

/// 只接受同一刷新代次和同一集群的指标结果，避免切换集群后写入旧快照。
pub(super) fn metrics_result_matches<T: PartialEq>(
    current_generation: u64,
    generation: u64,
    current_cluster: Option<&T>,
    result_cluster: Option<&T>,
) -> bool {
    current_generation == generation && current_cluster == result_cluster
}

impl KafkaView {
    pub(super) fn clear_metrics_snapshot(&mut self) {
        self.metrics_snapshot = None;
        self.metrics_error = None;
        self.broker_metrics_snapshot = None;
        self.broker_metrics_error = None;
        self.metrics_loading = false;
    }

    /// 停止当前集群的指标刷新器；已返回的旧快照不自动写入新集群。
    pub(super) fn invalidate_metrics_refresh(&mut self) {
        self.metrics_refresh_generation = self.metrics_refresh_generation.wrapping_add(1);
        if let Some(cancelled) = self.metrics_refresh_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
        self.metrics_loading = false;
    }

    /// 启动按集群隔离的指标刷新任务；每次只保留一条刷新链，迟到结果按代次丢弃。
    pub(super) fn start_metrics_refresh(
        &mut self,
        config: KafkaClusterConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let capabilities = self.service.transport_capabilities();
        if !capabilities.metrics_snapshot && config.broker_metrics.endpoint.is_none() {
            self.metrics_loading = false;
            return;
        }
        let interval = match parse_metrics_refresh_seconds(&self.metrics_refresh_seconds_input, cx)
        {
            Ok(interval) => interval,
            Err(error) => {
                self.metrics_error = Some(error);
                self.metrics_loading = false;
                return;
            }
        };
        self.invalidate_metrics_refresh();
        self.metrics_refresh_generation = self.metrics_refresh_generation.wrapping_add(1);
        let generation = self.metrics_refresh_generation;
        let cluster_id = config.id.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.metrics_refresh_cancelled = Some(cancelled.clone());
        self.metrics_loading = true;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, async_cx| {
            loop {
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                let (result, broker_result) = futures::join!(
                    service.metrics_snapshot(&config),
                    service.broker_metrics_snapshot(&config),
                );
                let result_cancelled = cancelled.clone();
                let result_cluster_id = cluster_id.clone();
                let keep_running = this.update_in(async_cx, move |this, _window, cx| {
                    if !metrics_result_matches(
                        this.metrics_refresh_generation,
                        generation,
                        this.selected_cluster_id.as_ref(),
                        Some(&result_cluster_id),
                    ) || result_cancelled.load(Ordering::Acquire)
                    {
                        return false;
                    }
                    this.metrics_loading = false;
                    this.apply_metrics_result(result);
                    this.apply_broker_metrics_result(broker_result);
                    cx.notify();
                    true
                });
                if !matches!(keep_running, Ok(true)) {
                    break;
                }
                async_cx
                    .background_executor()
                    .timer(Duration::from_secs(u64::from(interval)))
                    .await;
            }
        })
        .detach();
    }

    pub(super) fn refresh_metrics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.metrics_loading {
            return;
        }
        let Some(config) = self.selected_config() else {
            self.metrics_error = Some("请先选择已保存的 Kafka 集群".into());
            cx.notify();
            return;
        };
        if let Err(error) = parse_metrics_refresh_seconds(&self.metrics_refresh_seconds_input, cx) {
            self.metrics_error = Some(error);
            cx.notify();
            return;
        }
        self.start_metrics_refresh(config, window, cx);
        self.notice = Some(("正在采集 Kafka 指标快照…".into(), false));
        cx.notify();
    }

    fn apply_metrics_result(&mut self, result: DomainResult<KafkaMetricsSnapshot>) {
        match result {
            Ok(snapshot) => {
                let previous = self.metrics_snapshot.take();
                let snapshot = snapshot.with_high_watermark_rates(previous.as_ref());
                drop(previous);
                self.metrics_snapshot = Some(snapshot);
                self.metrics_error = None;
            }
            Err(error) => {
                tracing::warn!(operation = "kafka_metrics_snapshot", error = %error, "Kafka 指标快照采集失败");
                self.metrics_error = Some(error.user_message());
            }
        }
    }

    fn apply_broker_metrics_result(&mut self, result: DomainResult<KafkaBrokerMetricsSnapshot>) {
        match result {
            Ok(snapshot) => {
                self.broker_metrics_snapshot = Some(snapshot);
                self.broker_metrics_error = None;
            }
            Err(error) => {
                tracing::warn!(
                    operation = "kafka_broker_metrics_snapshot",
                    error = %error,
                    "外部 Broker 指标快照采集失败"
                );
                self.broker_metrics_error = Some(error.user_message());
            }
        }
    }
}

fn parse_metrics_refresh_seconds(
    field: &Entity<InputState>,
    cx: &App,
) -> std::result::Result<u32, String> {
    let text = value(field, cx);
    let seconds = text
        .parse::<u32>()
        .map_err(|_| format!("指标刷新间隔必须是 {MIN_KAFKA_METRICS_REFRESH_SECONDS} - {MAX_KAFKA_METRICS_REFRESH_SECONDS} 秒"))?;
    if !(MIN_KAFKA_METRICS_REFRESH_SECONDS..=MAX_KAFKA_METRICS_REFRESH_SECONDS).contains(&seconds) {
        return Err(format!(
            "指标刷新间隔必须是 {MIN_KAFKA_METRICS_REFRESH_SECONDS} - {MAX_KAFKA_METRICS_REFRESH_SECONDS} 秒"
        ));
    }
    Ok(seconds)
}
