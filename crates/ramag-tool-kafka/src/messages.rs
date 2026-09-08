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

impl KafkaView {
    /// 启动一次有界消息读取；新任务会使旧任务结果失效，取消只更新 UI 代次并丢弃迟到结果。
    pub(super) fn read_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading_runtime || self.loading_messages {
            return;
        }
        let Some(config) = self.form_config(cx).ok() else {
            self.notice = Some(("请先完成有效的集群配置".into(), true));
            cx.notify();
            return;
        };
        let topic = value(&self.topic_input, cx);
        let partitions = match parse_partition_list(&value(&self.partition_input, cx)) {
            Ok(value) => value,
            Err(error) => {
                self.notice = Some((error, true));
                cx.notify();
                return;
            }
        };
        let scan = match self.range_mode {
            KafkaRangeMode::Offset => {
                let start_offset =
                    match parse_i64_input(&self.start_offset_input, cx, "起始 Offset") {
                        Ok(value) => value,
                        Err(error) => {
                            self.notice = Some((error, true));
                            cx.notify();
                            return;
                        }
                    };
                let end_offset = match optional_i64_input(&self.end_offset_input, cx, "结束 Offset")
                {
                    Ok(value) => value,
                    Err(error) => {
                        self.notice = Some((error, true));
                        cx.notify();
                        return;
                    }
                };
                KafkaMessageQuery::by_offset(
                    topic.clone(),
                    partitions.clone(),
                    start_offset,
                    end_offset,
                )
            }
            KafkaRangeMode::Time => {
                let start_time = match parse_datetime_input(&self.start_time_input, cx, "起始时间")
                {
                    Ok(Some(value)) => value,
                    Ok(None) => {
                        self.notice = Some(("起始时间不能为空".into(), true));
                        cx.notify();
                        return;
                    }
                    Err(error) => {
                        self.notice = Some((error, true));
                        cx.notify();
                        return;
                    }
                };
                let end_time = match parse_datetime_input(&self.end_time_input, cx, "结束时间")
                {
                    Ok(value) => value,
                    Err(error) => {
                        self.notice = Some((error, true));
                        cx.notify();
                        return;
                    }
                };
                KafkaMessageQuery::by_time(topic, partitions, start_time, end_time)
            }
        }
        .with_limits(
            match parse_usize_input(&self.max_records_input, cx, "最多读取条数") {
                Ok(value) => value,
                Err(error) => {
                    self.notice = Some((error, true));
                    cx.notify();
                    return;
                }
            },
            DEFAULT_KAFKA_MAX_BYTES,
            DEFAULT_KAFKA_MAX_SCAN_SECONDS,
            DEFAULT_KAFKA_MAX_CONCURRENT_PARTITIONS,
        );
        if let Err(error) = scan.validate() {
            self.notice = Some((error, true));
            cx.notify();
            return;
        }
        let search_text = value(&self.message_search, cx);
        let search_fields = self.selected_search_fields();
        if !search_text.is_empty() && search_fields.is_empty() {
            self.notice = Some(("至少选择一个搜索字段".into(), true));
            cx.notify();
            return;
        }
        self.message_request_id = self.message_request_id.wrapping_add(1);
        let request_id = self.message_request_id;
        self.message_page = None;
        self.selected_message = None;
        self.reset_message_paging();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.message_read_cancelled = Some(cancelled.clone());
        self.loading_messages = true;
        self.notice = Some((
            if search_text.is_empty() {
                "正在按范围读取消息…".into()
            } else {
                "正在按范围扫描并搜索消息…".into()
            },
            false,
        ));
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = if search_text.is_empty() {
                service
                    .read_messages_with_cancel(&config, &scan, cancelled.clone())
                    .await
            } else {
                let query =
                    KafkaMessageSearchQuery::new(search_text, scan).with_fields(search_fields);
                service
                    .search_messages_with_cancel(&config, &query, cancelled.clone())
                    .await
            };
            let _ = this.update_in(cx, |this, _window, cx| {
                if this.message_request_id != request_id {
                    return;
                }
                this.loading_messages = false;
                this.message_read_cancelled = None;
                match result {
                    Ok(page) => match page.validate() {
                        Ok(()) => {
                            let count = page.records.len();
                            let truncated = page.truncated;
                            this.message_page = Some(page);
                            this.notice = Some((
                                format!(
                                    "读取完成：返回 {count} 条，{}",
                                    if truncated {
                                        "已触达扫描预算"
                                    } else {
                                        "未触达扫描预算"
                                    }
                                ),
                                false,
                            ));
                        }
                        Err(error) => {
                            this.notice = Some((format!("消息结果校验失败：{error}"), true));
                        }
                    },
                    Err(error) => {
                        this.mark_runtime_failure("读取消息", &error);
                        this.notice =
                            Some((format!("读取消息失败：{}", error.user_message()), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 使当前消息任务失效并通知底层扫描线程退出；迟到结果不会回写页面。
    pub(super) fn cancel_message_read(&mut self, cx: &mut Context<Self>) {
        if !self.loading_messages {
            return;
        }
        self.invalidate_message_request();
        self.notice = Some(("消息读取已取消；迟到结果不会写入当前页面".into(), false));
        cx.notify();
    }

    pub(super) fn invalidate_message_request(&mut self) {
        self.message_request_id = self.message_request_id.wrapping_add(1);
        self.loading_messages = false;
        if let Some(cancelled) = self.message_read_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
        self.invalidate_message_tail();
    }

    /// 回到已加载消息的第一页，并把虚拟列表滚动位置归零，避免新查询沿用旧视口。
    pub(super) fn reset_message_paging(&mut self) {
        self.message_page_index = 0;
        self.selected_message = None;
        self.message_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
        self.message_page_scroll
            .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
    }

    /// 返回当前已加载结果的页数；分页只切分内存中的有界结果，不扩大 Kafka 扫描范围。
    pub(super) fn message_page_count(&self) -> usize {
        self.message_page.as_ref().map_or(0, |page| {
            page.records.len().div_ceil(self.message_page_size)
        })
    }

    /// 切换已加载结果页并清理详情选择，保证详情不会指向上一页的记录。
    pub(super) fn set_message_page(&mut self, page_index: usize, cx: &mut Context<Self>) {
        let page_count = self.message_page_count();
        if page_index >= page_count || page_index == self.message_page_index {
            return;
        }
        self.message_page_index = page_index;
        self.selected_message = None;
        self.message_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
        cx.notify();
    }

    pub(super) fn selected_search_fields(&self) -> Vec<KafkaMessageSearchField> {
        KafkaMessageSearchField::all()
            .into_iter()
            .enumerate()
            .filter_map(|(index, field)| self.search_fields[index].then_some(field))
            .collect()
    }

    /// 将选中的消息写成可回放的 JSON 包络；字节字段使用 Base64，避免 UTF-8 失败导致丢数据。
    pub(super) fn export_message(
        &mut self,
        record: KafkaMessageRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exporting {
            return;
        }
        if record.retained_bytes() > MAX_KAFKA_EXPORT_BYTES {
            self.notice = Some((
                format!(
                    "消息过大，单条导出最多支持 {} MB",
                    MAX_KAFKA_EXPORT_BYTES / (1024 * 1024)
                ),
                true,
            ));
            cx.notify();
            return;
        }
        let file_name = suggested_message_file_name(&record);
        self.exporting = true;
        self.notice = Some(("正在准备消息导出…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let outcome: std::result::Result<Option<String>, String> = async {
                let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_file_name(&file_name)
                    .add_filter("Kafka JSON", &["json"])
                    .save_file()
                    .await
                else {
                    return Ok(None);
                };
                let path = handle.path().to_path_buf();
                let content = serde_json::to_string_pretty(&KafkaMessageExport::from(&record))
                    .map_err(|error| format!("生成消息 JSON 失败：{error}"))?;
                let write_path = path.clone();
                ramag_app::run_blocking(move || {
                    ramag_app::usecases::export::write_atomic(&write_path, &content)
                })
                .await
                .map_err(|error| format!("写入消息导出失败：{error}"))?;
                Ok(Some(path.display().to_string()))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.exporting = false;
                match outcome {
                    Ok(None) => {}
                    Ok(Some(path)) => {
                        this.notice = Some((format!("消息已导出到 {path}"), false));
                    }
                    Err(error) => {
                        this.notice = Some((error, true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    // 选择 Topic 只更新当前筛选和详情状态；切换到消息页由详情区的明确按钮负责。
    pub(super) fn select_topic(
        &mut self,
        topic: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_message_request();
        self.reset_message_paging();
        self.clear_message_tail(cx);
        self.selected_topic = Some(topic.clone());
        set_value(&self.topic_input, topic, window, cx);
        if self.config_resource_type == KafkaConfigResourceType::Topic {
            self.invalidate_config_request();
            set_value(
                &self.config_resource_name,
                self.selected_topic.clone().unwrap_or_default(),
                window,
                cx,
            );
            set_value(&self.config_value, "", window, cx);
        }
        let target_partitions = self
            .selected_topic
            .as_ref()
            .and_then(|name| self.topics.iter().find(|candidate| &candidate.name == name))
            .map(|topic| topic.partitions.len().saturating_add(1).to_string())
            .unwrap_or_default();
        set_value(&self.topic_target_partitions, target_partitions, window, cx);
        self.message_page = None;
        self.selected_message = None;
        self.notice = None;
        cx.notify();
    }
}
