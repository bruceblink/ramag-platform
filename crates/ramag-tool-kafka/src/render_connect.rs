use super::*;

pub(super) fn matching_connectors(connectors: &[KafkaConnectConnector], query: &str) -> Vec<usize> {
    let query = query.to_lowercase();
    connectors
        .iter()
        .enumerate()
        .filter_map(|(index, connector)| {
            let matches = query.is_empty()
                || connector.name.to_lowercase().contains(&query)
                || connector
                    .connector_type
                    .as_deref()
                    .is_some_and(|connector_type| connector_type.to_lowercase().contains(&query))
                || connector.state.to_lowercase().contains(&query);
            matches.then_some(index)
        })
        .collect()
}

impl KafkaView {
    pub(super) fn invalidate_connect_request(&mut self) {
        self.connect_request_id = self.connect_request_id.wrapping_add(1);
        self.loading_connectors = false;
        if let Some(cancelled) = self.connect_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
    }

    pub(super) fn clear_connect_snapshot(&mut self) {
        self.invalidate_connect_request();
        self.connect_connectors.clear();
        self.connectors_loaded = false;
        self.connect_error = None;
        self.connect_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
    }

    /// 读取 Kafka Connect 的只读状态；未配置端点时不发起网络请求。
    pub(super) fn load_connectors(
        &mut self,
        config: KafkaClusterConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading_connectors
            || self.loading_runtime
            || self.saving
            || self.deleting
            || self.selected_cluster_id.as_ref() != Some(&config.id)
        {
            return;
        }
        if config.connect.endpoint.is_none() {
            self.clear_connect_snapshot();
            self.connectors_loaded = true;
            cx.notify();
            return;
        }

        self.connect_request_id = self.connect_request_id.wrapping_add(1);
        let request_id = self.connect_request_id;
        let cluster_id = config.id.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let service = self.service.clone();
        self.connect_cancelled = Some(cancelled.clone());
        self.loading_connectors = true;
        self.connectors_loaded = false;
        self.connect_error = None;
        self.notice = Some(("正在读取 Kafka Connect 连接器状态…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .list_connectors_with_cancel(&config, cancelled)
                .await;
            let _ = this.update_in(cx, |this, _window, cx| {
                if this.connect_request_id != request_id
                    || this.selected_cluster_id.as_ref() != Some(&cluster_id)
                {
                    return;
                }
                this.loading_connectors = false;
                this.connect_cancelled = None;
                this.connectors_loaded = true;
                match result {
                    Ok(connectors) => {
                        let count = connectors.len();
                        this.connect_connectors = connectors;
                        this.connect_error = None;
                        this.notice =
                            Some((format!("已读取 {count} 个 Kafka Connect 连接器"), false));
                    }
                    Err(error) => {
                        this.connect_connectors.clear();
                        this.connect_error = Some(error.user_message());
                        this.notice = Some((
                            format!("读取 Kafka Connect 失败：{}", error.user_message()),
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

    pub(super) fn render_connect(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let compact = kafka_main_content_width(window) < 700.0;
        let config = self.selected_config();
        let configured = config
            .as_ref()
            .is_some_and(|config| config.connect.endpoint.is_some());
        let query = value(&self.connect_search, cx);
        let visible_indices = matching_connectors(&self.connect_connectors, &query);
        let visible_count = visible_indices.len();
        let list = if !configured {
            connector_empty_state(
                "未配置 Kafka Connect",
                "请在配置页填写 Kafka Connect 地址并保存",
                &theme,
            )
            .into_any_element()
        } else if self.loading_connectors {
            h_flex()
                .id("kafka-connect-table")
                .debug_selector(|| "kafka-connect-table".into())
                .size_full()
                .items_stretch()
                .child(
                    v_flex()
                        .id("kafka-connect-list-content")
                        .debug_selector(|| "kafka-connect-list-content".into())
                        .h_full()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .child(loading_transition(
                            skeleton_table(&theme, 7, &[Some(190.0), Some(84.0), Some(150.0)]),
                            "kafka-connect-loading-transition",
                        )),
                )
                .child(connect_scrollbar(&self.connect_scroll, &theme))
                .into_any_element()
        } else if let Some(error) = self.connect_error.clone() {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .child(Icon::new(IconName::TriangleAlert).text_color(theme.danger))
                .child(div().text_xs().text_color(theme.danger).child(error))
                .child(
                    ramag_ui::clickable_button("kafka-connect-retry")
                        .outline()
                        .small()
                        .icon(IconName::Search)
                        .label("重试")
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            if let Some(config) = this.selected_config() {
                                this.clear_connect_snapshot();
                                this.load_connectors(config, window, cx);
                            }
                        })),
                )
                .into_any_element()
        } else if visible_indices.is_empty() {
            connector_empty_state(
                if self.connect_connectors.is_empty() {
                    "Kafka Connect 没有连接器"
                } else {
                    "没有匹配的连接器"
                },
                "可修改筛选条件或刷新列表",
                &theme,
            )
            .into_any_element()
        } else {
            let visible_indices = Arc::new(visible_indices);
            let row_theme = theme.clone();
            let rows = uniform_list(
                "kafka-connect-list",
                visible_count,
                cx.processor(move |this, range: Range<usize>, _window, _cx| {
                    range
                        .filter_map(|index| {
                            let connector_index = *visible_indices.get(index)?;
                            let connector = this.connect_connectors.get(connector_index)?;
                            Some(connector_row(index, connector, &row_theme))
                        })
                        .collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.connect_scroll)
            .flex_1()
            .min_h_0()
            .min_w_0();
            h_flex()
                .id("kafka-connect-table")
                .debug_selector(|| "kafka-connect-table".into())
                .size_full()
                .items_stretch()
                .child(
                    v_flex()
                        .id("kafka-connect-list-content")
                        .debug_selector(|| "kafka-connect-list-content".into())
                        .h_full()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .child(rows),
                )
                .child(connect_scrollbar(&self.connect_scroll, &theme))
                .into_any_element()
        };
        v_flex()
            .id("kafka-connect")
            .debug_selector(|| "kafka-connect".into())
            .size_full()
            .min_w_0()
            .min_h_0()
            .p(px(18.0))
            .gap(px(12.0))
            .child(
                h_flex()
                    .id("kafka-connect-header")
                    .debug_selector(|| "kafka-connect-header".into())
                    .w_full()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .when(compact, |row| row.flex_col().items_stretch())
                    .child(section_heading(
                        "Kafka Connect",
                        "只读取连接器和 Task 状态；不执行暂停、恢复、重启或删除",
                        &theme,
                    ))
                    .child(
                        h_flex()
                            .id("kafka-connect-actions")
                            .debug_selector(|| "kafka-connect-actions".into())
                            .flex_none()
                            .items_center()
                            .gap(px(8.0))
                            .when(compact, |row| row.w_full())
                            .child(
                                div().min_w_0().when(compact, |label| label.flex_1()).child(
                                    ramag_ui::cleanable_input(
                                        &self.connect_search,
                                        "kafka-connect-search-clear",
                                        false,
                                        cx,
                                    )
                                    .small()
                                    .prefix(
                                        Icon::new(IconName::Search)
                                            .small()
                                            .text_color(theme.muted_foreground),
                                    ),
                                ),
                            )
                            .when(configured, |row| {
                                row.child(
                                    ramag_ui::clickable_button("kafka-connect-refresh")
                                        .outline()
                                        .small()
                                        .icon(IconName::Search)
                                        .label("刷新")
                                        .loading(self.loading_connectors)
                                        .disabled(
                                            self.loading_connectors
                                                || self.loading_runtime
                                                || self.saving
                                                || self.deleting,
                                        )
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, window, cx| {
                                                if let Some(config) = this.selected_config() {
                                                    this.clear_connect_snapshot();
                                                    this.load_connectors(config, window, cx);
                                                }
                                            },
                                        )),
                                )
                            }),
                    ),
            )
            .child(
                v_flex()
                    .id("kafka-connect-list-panel")
                    .debug_selector(|| "kafka-connect-list-panel".into())
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(6.0))
                    .child(list),
            )
    }
}

fn connect_scrollbar(
    scroll: &UniformListScrollHandle,
    theme: &gpui_component::Theme,
) -> impl IntoElement {
    div()
        .id("kafka-connect-v-scrollbar")
        .debug_selector(|| "kafka-connect-v-scrollbar".into())
        .h_full()
        .w(px(KAFKA_SCHEMA_SUBJECT_SCROLLBAR_WIDTH))
        .flex_none()
        .bg(theme.scrollbar)
        .child(
            Scrollbar::vertical(scroll)
                .id("kafka-connect-v-scrollbar-control")
                .scrollbar_show(ScrollbarShow::Always),
        )
}

fn connector_empty_state(
    title: &'static str,
    detail: &'static str,
    theme: &gpui_component::Theme,
) -> gpui::Div {
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(title),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(detail),
        )
}

fn connector_row(
    index: usize,
    connector: &KafkaConnectConnector,
    theme: &gpui_component::Theme,
) -> gpui::AnyElement {
    let state_color = match connector.state.as_str() {
        "RUNNING" => theme.success,
        "PAUSED" => theme.warning,
        "FAILED" => theme.danger,
        _ => theme.muted_foreground,
    };
    h_flex()
        .id(SharedString::from(format!("kafka-connect-row-{index}")))
        .debug_selector(move || format!("kafka-connect-row-{index}"))
        .w_full()
        .min_w_0()
        .items_center()
        .gap(px(12.0))
        .px(px(12.0))
        .py(px(10.0))
        .border_b_1()
        .border_color(theme.border)
        .child(
            v_flex()
                .min_w_0()
                .flex_1()
                .gap(px(2.0))
                .child(div().text_sm().truncate().child(connector.name.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(
                            connector
                                .connector_type
                                .clone()
                                .unwrap_or_else(|| "连接器类型未知".into()),
                        ),
                ),
        )
        .child(
            div()
                .w(px(86.0))
                .flex_none()
                .text_xs()
                .text_color(state_color)
                .child(connector.state.clone()),
        )
        .child(
            div()
                .w(px(96.0))
                .flex_none()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!("Task {}", connector.tasks.len())),
        )
        .child(
            div()
                .w(px(180.0))
                .flex_none()
                .truncate()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(
                    connector
                        .worker_id
                        .clone()
                        .unwrap_or_else(|| "Worker 未知".into()),
                ),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .text_xs()
                .text_color(theme.danger)
                .child(connector.error.clone().unwrap_or_default()),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_search_matches_name_type_and_state() {
        let connectors = vec![KafkaConnectConnector {
            name: "orders-sink".into(),
            connector_type: Some("sink".into()),
            state: "FAILED".into(),
            worker_id: None,
            tasks: Vec::new(),
            error: None,
        }];
        assert_eq!(matching_connectors(&connectors, "ORDERS"), vec![0]);
        assert_eq!(matching_connectors(&connectors, "sink"), vec![0]);
        assert_eq!(matching_connectors(&connectors, "failed"), vec![0]);
    }
}
