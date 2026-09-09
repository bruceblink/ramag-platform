use super::*;
use async_channel::{TrySendError, bounded};

impl KafkaView {
    /// 启动持续消费：Transport 只把事件写入有界 channel，UI 任务负责逐项收取并维护窗口。
    pub(super) fn start_message_tail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.message_tail_running {
            return;
        }
        let Some(config) = self.form_config(cx).ok() else {
            self.notice = Some(("请先完成有效的集群配置".into(), true));
            cx.notify();
            return;
        };
        let topic = value(&self.topic_input, cx);
        let partitions = match parse_partition_list(&value(&self.partition_input, cx)) {
            Ok(partitions) => partitions,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let start = match self.message_tail_start_mode {
            KafkaTailStartMode::Latest => KafkaMessageTailStart::Latest,
            KafkaTailStartMode::Earliest => KafkaMessageTailStart::Earliest,
            KafkaTailStartMode::Offset => {
                match parse_tail_i64(&self.message_tail_offset_input, cx, "实时起始 Offset") {
                    Ok(offset) => KafkaMessageTailStart::Offset(offset),
                    Err(error) => {
                        self.notice = Some((error, true));
                        cx.notify();
                        return;
                    }
                }
            }
        };
        let limits = match (
            parse_tail_usize(&self.message_tail_window_messages_input, cx, "实时窗口条数"),
            parse_tail_u64(&self.message_tail_window_bytes_input, cx, "实时窗口字节数"),
            parse_tail_usize(
                &self.message_tail_max_message_bytes_input,
                cx,
                "实时单条最大字节数",
            ),
            parse_tail_u32(&self.message_tail_poll_timeout_input, cx, "实时轮询毫秒"),
        ) {
            (Ok(messages), Ok(bytes), Ok(max_message), Ok(timeout)) => {
                (messages, bytes, max_message, timeout)
            }
            (Err(error), _, _, _)
            | (_, Err(error), _, _)
            | (_, _, Err(error), _)
            | (_, _, _, Err(error)) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let request = KafkaMessageTailRequest::new(topic.clone(), partitions, start)
            .with_limits(limits.0, limits.1, limits.2, limits.3);
        if let Err(error) = request.validate() {
            self.notice = Some((error, true));
            cx.notify();
            return;
        }

        self.invalidate_message_tail();
        self.message_tail_records.clear();
        self.message_tail_bytes = 0;
        self.message_tail_dropped_records = 0;
        self.message_tail_dropped_bytes = 0;
        self.message_tail_evicted_records = 0;
        self.message_tail_evicted_bytes = 0;
        self.selected_tail_message = None;
        self.message_tail_window_messages = request.max_window_messages;
        self.message_tail_window_bytes = request.max_window_bytes;
        self.message_tail_running = true;
        self.message_tail_connected = true;
        self.message_tail_paused = false;
        self.message_tail_reconnect_attempt = 0;
        self.message_tail_request_id = self.message_tail_request_id.wrapping_add(1);
        let request_id = self.message_tail_request_id;
        let cancelled = Arc::new(AtomicBool::new(false));
        self.message_tail_cancelled = Some(cancelled.clone());
        let (sender, receiver) = bounded(MESSAGE_TAIL_CHANNEL_CAPACITY);
        let sink: KafkaMessageTailSink = Arc::new(move |event| match sender.try_send(event) {
            Ok(()) => KafkaMessageTailSinkResult::Accepted,
            Err(TrySendError::Full(_)) => KafkaMessageTailSinkResult::Backpressured,
            Err(TrySendError::Closed(_)) => KafkaMessageTailSinkResult::Closed,
        });

        let service = self.service.clone();
        let consumer_cancelled = cancelled.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .tail_messages(&config, &request, sink, consumer_cancelled)
                .await;
            let _ = this.update_in(cx, |this, _window, cx| {
                if this.message_tail_request_id != request_id {
                    return;
                }
                this.message_tail_running = false;
                this.message_tail_connected = false;
                this.message_tail_cancelled = None;
                if let Err(error) = result {
                    this.mark_runtime_failure("持续读取消息", &error);
                    this.notice =
                        Some((format!("持续读取消息失败：{}", error.user_message()), true));
                } else {
                    this.notice = Some(("持续读取消息已停止".into(), false));
                }
                cx.notify();
            });
        })
        .detach();

        cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                let _ = this.update_in(cx, |this, _window, cx| {
                    if this.message_tail_request_id != request_id {
                        return;
                    }
                    this.apply_message_tail_event(event);
                    cx.notify();
                });
            }
        })
        .detach();
        self.notice = Some((format!("已开始持续读取 Topic「{topic}」"), false));
        cx.notify();
    }

    pub(super) fn stop_message_tail(&mut self, cx: &mut Context<Self>) {
        if !self.message_tail_running {
            return;
        }
        self.invalidate_message_tail();
        self.notice = Some(("持续读取消息已停止；当前窗口仍保留".into(), false));
        cx.notify();
    }

    pub(super) fn toggle_message_tail_pause(&mut self, cx: &mut Context<Self>) {
        if !self.message_tail_running {
            return;
        }
        self.message_tail_paused = !self.message_tail_paused;
        self.notice = Some((
            if self.message_tail_paused {
                "已暂停展示；后台仍受有界通道和大小预算限制"
            } else {
                "已恢复展示"
            }
            .into(),
            false,
        ));
        cx.notify();
    }

    pub(super) fn clear_message_tail(&mut self, cx: &mut Context<Self>) {
        self.message_tail_records.clear();
        self.message_tail_bytes = 0;
        self.message_tail_dropped_records = 0;
        self.message_tail_dropped_bytes = 0;
        self.message_tail_evicted_records = 0;
        self.message_tail_evicted_bytes = 0;
        self.selected_tail_message = None;
        self.message_tail_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
        cx.notify();
    }

    /// 使实时消费者失效并发出取消信号；native poll 最迟在下一次有界超时后退出。
    pub(super) fn invalidate_message_tail(&mut self) {
        self.message_tail_request_id = self.message_tail_request_id.wrapping_add(1);
        if let Some(cancelled) = self.message_tail_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
        self.message_tail_running = false;
        self.message_tail_connected = false;
        self.message_tail_paused = false;
    }

    fn apply_message_tail_event(&mut self, event: KafkaMessageTailEvent) {
        match event {
            KafkaMessageTailEvent::Message(record) => {
                let bytes = record.retained_bytes();
                if self.message_tail_paused {
                    self.message_tail_dropped_records =
                        self.message_tail_dropped_records.saturating_add(1);
                    self.message_tail_dropped_bytes =
                        self.message_tail_dropped_bytes.saturating_add(bytes);
                    return;
                }
                if bytes > self.message_tail_window_bytes {
                    self.message_tail_dropped_records =
                        self.message_tail_dropped_records.saturating_add(1);
                    self.message_tail_dropped_bytes =
                        self.message_tail_dropped_bytes.saturating_add(bytes);
                    return;
                }
                while self.message_tail_records.len() >= self.message_tail_window_messages
                    || self.message_tail_bytes.saturating_add(bytes)
                        > self.message_tail_window_bytes
                {
                    let Some(oldest) = self.message_tail_records.pop_front() else {
                        break;
                    };
                    let oldest_bytes = oldest.retained_bytes();
                    self.message_tail_bytes = self.message_tail_bytes.saturating_sub(oldest_bytes);
                    self.message_tail_evicted_records =
                        self.message_tail_evicted_records.saturating_add(1);
                    self.message_tail_evicted_bytes =
                        self.message_tail_evicted_bytes.saturating_add(oldest_bytes);
                    if let Some(index) = self.selected_tail_message {
                        self.selected_tail_message = index.checked_sub(1);
                    }
                }
                self.message_tail_bytes = self.message_tail_bytes.saturating_add(bytes);
                self.message_tail_records.push_back(record);
            }
            KafkaMessageTailEvent::Dropped { records, bytes } => {
                self.message_tail_dropped_records =
                    self.message_tail_dropped_records.saturating_add(records);
                self.message_tail_dropped_bytes =
                    self.message_tail_dropped_bytes.saturating_add(bytes);
            }
            KafkaMessageTailEvent::Reconnecting { attempt } => {
                self.message_tail_connected = false;
                self.message_tail_reconnect_attempt = attempt;
            }
            KafkaMessageTailEvent::Connected { attempt } => {
                self.message_tail_connected = true;
                self.message_tail_reconnect_attempt = attempt;
            }
        }
    }
}

fn parse_tail_i64(field: &Entity<InputState>, cx: &App, label: &str) -> Result<i64, String> {
    let text = value(field, cx);
    text.parse::<i64>()
        .map_err(|_| format!("{label} 必须是非负整数"))
        .and_then(|value| {
            (value >= 0)
                .then_some(value)
                .ok_or_else(|| format!("{label} 必须是非负整数"))
        })
}

fn parse_tail_usize(field: &Entity<InputState>, cx: &App, label: &str) -> Result<usize, String> {
    value(field, cx)
        .parse::<usize>()
        .map_err(|_| format!("{label} 必须是正整数"))
}

fn parse_tail_u64(field: &Entity<InputState>, cx: &App, label: &str) -> Result<u64, String> {
    value(field, cx)
        .parse::<u64>()
        .map_err(|_| format!("{label} 必须是正整数"))
}

fn parse_tail_u32(field: &Entity<InputState>, cx: &App, label: &str) -> Result<u32, String> {
    value(field, cx)
        .parse::<u32>()
        .map_err(|_| format!("{label} 必须是正整数"))
}
