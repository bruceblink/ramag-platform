use super::*;

/// ksqlDB 查询页的独立状态；请求代次和取消信号不与 Kafka 消息读取共享。
pub(super) struct KafkaKsqlDbState {
    pub(super) endpoint: Entity<InputState>,
    pub(super) query: Entity<InputState>,
    pub(super) result: Option<KafkaKsqlDbQueryResult>,
    pub(super) error: Option<String>,
    pub(super) loading: bool,
    pub(super) request_id: u64,
    pub(super) cancelled: Option<Arc<AtomicBool>>,
    pub(super) scroll: ScrollHandle,
}

impl KafkaKsqlDbState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<KafkaView>) -> Self {
        Self {
            endpoint: input(
                window,
                cx,
                MAX_KAFKA_KSQLDB_ENDPOINT_BYTES,
                "ksqlDB Server 地址（可选）",
                false,
                "",
            ),
            query: input(
                window,
                cx,
                MAX_KAFKA_KSQLDB_QUERY_BYTES,
                "SELECT ... EMIT CHANGES LIMIT 20",
                false,
                "SELECT * FROM RAMAG_STREAM EMIT CHANGES LIMIT 20;",
            ),
            result: None,
            error: None,
            loading: false,
            request_id: 0,
            cancelled: None,
            scroll: ScrollHandle::new(),
        }
    }
}

impl KafkaView {
    pub(super) fn invalidate_ksqldb_request(&mut self) {
        self.ksqldb.request_id = self.ksqldb.request_id.wrapping_add(1);
        self.ksqldb.loading = false;
        if let Some(cancelled) = self.ksqldb.cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
    }

    pub(super) fn reset_ksqldb_query(&mut self) {
        self.invalidate_ksqldb_request();
        self.ksqldb.result = None;
        self.ksqldb.error = None;
    }

    /// 执行当前保存集群上的只读 ksqlDB 查询；切换集群后拒绝迟到结果。
    pub(super) fn execute_ksqldb_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ksqldb.loading {
            return;
        }
        let Some(config) = self.selected_config() else {
            self.ksqldb.error = Some("请先选择已保存的 Kafka 集群".into());
            cx.notify();
            return;
        };
        let query = KafkaKsqlDbQuery::new(value(&self.ksqldb.query, cx));
        if let Err(error) = query.validate() {
            self.ksqldb.error = Some(error);
            self.ksqldb.result = None;
            cx.notify();
            return;
        }
        self.ksqldb.request_id = self.ksqldb.request_id.wrapping_add(1);
        let request_id = self.ksqldb.request_id;
        let context_cluster_id = self.selected_cluster_id.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.ksqldb.cancelled = Some(cancelled.clone());
        self.ksqldb.loading = true;
        self.ksqldb.error = None;
        self.ksqldb.result = None;
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .execute_ksqldb_query_with_cancel(&config, &query, cancelled)
                .await;
            let _ = this.update_in(cx, |this, _window, cx| {
                if this.ksqldb.request_id != request_id
                    || this.selected_cluster_id.as_ref() != context_cluster_id.as_ref()
                {
                    return;
                }
                this.ksqldb.loading = false;
                this.ksqldb.cancelled = None;
                match result {
                    Ok(result) => {
                        this.ksqldb.error = None;
                        this.ksqldb.result = Some(result);
                    }
                    Err(error) => {
                        this.ksqldb.result = None;
                        this.ksqldb.error = Some(error.user_message());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn cancel_ksqldb_query(&mut self, cx: &mut Context<Self>) {
        if !self.ksqldb.loading {
            return;
        }
        self.invalidate_ksqldb_request();
        self.ksqldb.error = Some("ksqlDB 查询已取消".into());
        cx.notify();
    }

    pub(super) fn render_ksqldb_query(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let compact = kafka_main_content_width(window) < 900.0;
        let result = if self.ksqldb.loading {
            v_flex()
                .id("kafka-ksqldb-result-loading")
                .debug_selector(|| "kafka-ksqldb-result-loading".into())
                .h(px(92.0))
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("正在执行 ksqlDB 查询…")
                .into_any_element()
        } else if let Some(error) = &self.ksqldb.error {
            v_flex()
                .id("kafka-ksqldb-result-error")
                .debug_selector(|| "kafka-ksqldb-result-error".into())
                .min_h(px(52.0))
                .justify_center()
                .text_xs()
                .text_color(theme.danger)
                .child(error.clone())
                .into_any_element()
        } else if let Some(snapshot) = &self.ksqldb.result {
            self.render_ksqldb_result(snapshot, &theme)
                .into_any_element()
        } else {
            v_flex()
                .id("kafka-ksqldb-result-empty")
                .debug_selector(|| "kafka-ksqldb-result-empty".into())
                .h(px(52.0))
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("尚未执行查询")
                .into_any_element()
        };
        v_flex()
            .id("kafka-ksqldb-query")
            .debug_selector(|| "kafka-ksqldb-query".into())
            .w_full()
            .min_w_0()
            .gap(px(10.0))
            .p(px(12.0))
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .child(section_heading(
                "ksqlDB 只读查询",
                "只接受 SELECT；结果、行数和响应正文均受限",
                &theme,
            ))
            .child(
                h_flex()
                    .id("kafka-ksqldb-query-controls")
                    .debug_selector(|| "kafka-ksqldb-query-controls".into())
                    .w_full()
                    .items_end()
                    .gap(px(8.0))
                    .when(compact, |row| row.flex_col().items_stretch())
                    .child(
                        field("SQL", Input::new(&self.ksqldb.query).small(), 0.0)
                            .flex_1()
                            .min_w_0()
                            .debug_selector(|| "kafka-ksqldb-query-input".into()),
                    )
                    .child(
                        ramag_ui::clickable_button("kafka-ksqldb-query-run")
                            .debug_selector(|| "kafka-ksqldb-query-run".into())
                            .primary()
                            .small()
                            .icon(IconName::Search)
                            .label("执行")
                            .loading(self.ksqldb.loading)
                            .disabled(self.ksqldb.loading || self.loading_runtime || self.saving)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.execute_ksqldb_query(window, cx);
                            })),
                    )
                    .when(self.ksqldb.loading, |row| {
                        row.child(
                            ramag_ui::clickable_button("kafka-ksqldb-query-cancel")
                                .debug_selector(|| "kafka-ksqldb-query-cancel".into())
                                .outline()
                                .small()
                                .icon(IconName::Close)
                                .label("取消")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.cancel_ksqldb_query(cx);
                                })),
                        )
                    }),
            )
            .child(result)
    }

    fn render_ksqldb_result(
        &self,
        result: &KafkaKsqlDbQueryResult,
        theme: &gpui_component::Theme,
    ) -> impl IntoElement {
        let columns = if result.columns.is_empty() {
            vec!["结果".to_owned()]
        } else {
            result.columns.clone()
        };
        let rows = result.rows.iter().map(|row| {
            h_flex()
                .w_full()
                .min_w_0()
                .gap(px(8.0))
                .px(px(8.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(theme.border)
                .children(columns.iter().enumerate().map(|(index, _)| {
                    div()
                        .flex_1()
                        .min_w(px(120.0))
                        .text_xs()
                        .whitespace_normal()
                        .child(row.get(index).cloned().unwrap_or_default())
                }))
        });
        v_flex()
            .id("kafka-ksqldb-result")
            .debug_selector(|| "kafka-ksqldb-result".into())
            .w_full()
            .min_w_0()
            .max_h(px(280.0))
            .overflow_y_scroll()
            .track_scroll(&self.ksqldb.scroll)
            .border_1()
            .border_color(theme.border)
            .rounded(px(4.0))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(6.0))
                    .bg(theme.secondary)
                    .children(columns.iter().map(|column| {
                        div()
                            .flex_1()
                            .min_w(px(120.0))
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(column.clone())
                    })),
            )
            .children(rows)
            .when(result.truncated, |panel| {
                panel.child(
                    div()
                        .px(px(8.0))
                        .py(px(6.0))
                        .text_xs()
                        .text_color(theme.warning)
                        .child("结果已截断"),
                )
            })
    }
}
