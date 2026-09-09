use super::*;

impl KafkaView {
    /// 根据窗口宽度排列概览内容，避免高侧栏把后续 Topic 预览推入无意义的空白区。
    pub(super) fn render_overview(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let content_width = kafka_main_content_width(window);
        // Keep the overview split in step with the Topic workspace. The width
        // is already measured after the sidebar, so 900px is the usable-pane
        // boundary rather than the outer window width.
        let compact = content_width < 900.0;
        let narrow = content_width < 700.0;
        let metadata = self.metadata.as_ref();
        let topic_count = self.topics.len();
        let partition_count = self
            .topics
            .iter()
            .map(|topic| topic.partitions.len())
            .sum::<usize>();
        let content = if self.loading_runtime {
            self.render_overview_loading(&theme, compact, narrow)
        } else {
            match metadata {
                None => v_flex()
                    .flex_1()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .child(Icon::new(IconName::Network).text_color(theme.muted_foreground))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("尚未建立连接"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("请先在配置页保存集群并测试连接"),
                    )
                    .into_any_element(),
                Some(metadata) => {
                    let metrics = h_flex()
                        .id("kafka-overview-metrics")
                        .debug_selector(|| "kafka-overview-metrics".into())
                        .w_full()
                        .min_w_0()
                        .flex_none()
                        .gap(px(12.0))
                        .when(narrow, |row| row.flex_col().items_stretch().gap(px(8.0)))
                        .child(metric_card("Brokers", metadata.brokers.len(), &theme))
                        .child(metric_card("Topics", topic_count, &theme))
                        .child(metric_card("Partitions", partition_count, &theme));
                    let broker_section = v_flex()
                        .id("kafka-overview-broker")
                        .debug_selector(|| "kafka-overview-broker".into())
                        .w_full()
                        .min_w_0()
                        .flex_none()
                        .gap(px(10.0))
                        .child(section_heading(
                            "Broker 健康与元数据",
                            "协议可达性来自 Kafka Metadata API；运行指标单独接入",
                            &theme,
                        ))
                        .child(self.render_broker_health(metadata, cx))
                        .child(self.render_broker_table(metadata, cx));
                    let topic_section = v_flex()
                        .id("kafka-overview-topic")
                        .debug_selector(|| "kafka-overview-topic".into())
                        .w_full()
                        .min_w_0()
                        .flex_none()
                        .gap(px(10.0))
                        .child(section_heading(
                            "Topic 预览",
                            "仅展示已从 Broker 读取的数据",
                            &theme,
                        ))
                        .child(self.render_topic_preview(cx));
                    let primary_sections = v_flex()
                        .id("kafka-overview-primary")
                        .debug_selector(|| "kafka-overview-primary".into())
                        .flex_1()
                        .min_w_0()
                        .gap(px(18.0))
                        .when(compact, |column| column.w_full().flex_initial())
                        .child(broker_section)
                        .child(topic_section);
                    let cluster_section = v_flex()
                        .id("kafka-overview-cluster")
                        .debug_selector(|| "kafka-overview-cluster".into())
                        .min_w_0()
                        .gap(px(10.0))
                        .when(compact, |panel| panel.w_full().flex_none())
                        .when(!compact, |panel| panel.w(px(320.0)).flex_none())
                        .child(section_heading("集群信息", "连接状态和协议摘要", &theme))
                        .child(self.render_cluster_summary(metadata, cx));
                    let sections = h_flex()
                        .id("kafka-overview-sections")
                        .debug_selector(|| "kafka-overview-sections".into())
                        .w_full()
                        .min_w_0()
                        .flex_none()
                        .items_stretch()
                        .gap(px(18.0))
                        .when(compact, |row| row.flex_col().items_stretch())
                        .child(primary_sections)
                        .child(cluster_section);
                    let overview_content = v_flex()
                        .id("kafka-overview-scroll")
                        .debug_selector(|| "kafka-overview-scroll".into())
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .items_stretch()
                        .overflow_y_scroll()
                        .track_scroll(&self.overview_scroll)
                        .p(px(22.0))
                        .gap(px(18.0))
                        .child(metrics)
                        .child(self.render_metrics_snapshot(window, cx))
                        .child(sections)
                        .into_any_element();
                    h_flex()
                        .id("kafka-overview-scroll-viewport")
                        .debug_selector(|| "kafka-overview-scroll-viewport".into())
                        .flex_1()
                        .w_full()
                        .min_w_0()
                        .min_h_0()
                        .items_stretch()
                        .child(overview_content)
                        .child(
                            div()
                                .id("kafka-overview-v-scrollbar")
                                .debug_selector(|| "kafka-overview-v-scrollbar".into())
                                .h_full()
                                .w(px(16.0))
                                .flex_none()
                                .bg(theme.scrollbar)
                                .child(
                                    Scrollbar::vertical(&self.overview_scroll)
                                        .id("kafka-overview-v-scrollbar-control")
                                        .scrollbar_show(ScrollbarShow::Always),
                                ),
                        )
                        .into_any_element()
                }
            }
        };
        let runtime_error = self.runtime_error.clone();
        v_flex()
            .size_full()
            .w_full()
            .min_h_0()
            .items_stretch()
            .when_some(runtime_error, |view, error| {
                view.child(
                    h_flex()
                        .id("kafka-runtime-error")
                        .debug_selector(|| "kafka-runtime-error".into())
                        .w_full()
                        .flex_wrap()
                        .items_center()
                        .gap(px(10.0))
                        .px(px(22.0))
                        .py(px(10.0))
                        .bg(theme.danger.opacity(0.08))
                        .border_b_1()
                        .border_color(theme.danger.opacity(0.25))
                        .child(Icon::new(IconName::TriangleAlert).text_color(theme.danger))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .whitespace_normal()
                                .text_xs()
                                .text_color(theme.danger)
                                .debug_selector(|| "kafka-runtime-error-message".into())
                                .child(error),
                        )
                        .child(
                            ramag_ui::clickable_button("kafka-runtime-retry")
                                .debug_selector(|| "kafka-runtime-retry".into())
                                .flex_none()
                                .outline()
                                .small()
                                .icon(ramag_ui::icons::refresh_cw())
                                .label("重试")
                                .disabled(
                                    self.loading_runtime
                                        || self.selected_cluster_id.is_none()
                                        || self.testing
                                        || self.saving
                                        || self.deleting
                                        || self.topic_operation
                                        || self.acl_operation,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.retry_runtime(window, cx);
                                })),
                        ),
                )
            })
            .child(content)
            .id("kafka-overview")
            .debug_selector(|| "kafka-overview".into())
    }

    /// 加载期间保留概览页的滚动容器、列结构和固定行高，避免空快照先触发布局塌缩。
    fn render_overview_loading(
        &self,
        theme: &gpui_component::Theme,
        compact: bool,
        narrow: bool,
    ) -> gpui::AnyElement {
        let metrics = h_flex()
            .id("kafka-overview-loading-metrics")
            .debug_selector(|| "kafka-overview-loading-metrics".into())
            .w_full()
            .min_w_0()
            .flex_none()
            .gap(px(12.0))
            .when(narrow, |row| row.flex_col().items_stretch().gap(px(8.0)))
            .child(skeleton_metric_card(theme))
            .child(skeleton_metric_card(theme))
            .child(skeleton_metric_card(theme));
        let snapshot_cluster_columns = if narrow {
            vec![Some(60.0), Some(64.0), Some(74.0), None]
        } else {
            vec![Some(84.0), Some(88.0), Some(104.0), None]
        };
        let snapshot_detail_columns = if narrow {
            vec![Some(72.0), Some(80.0), Some(70.0), None]
        } else {
            vec![Some(112.0), Some(100.0), Some(96.0), None]
        };
        let broker_columns = if narrow {
            vec![Some(78.0), None, Some(76.0)]
        } else {
            vec![Some(112.0), None, Some(96.0), Some(118.0)]
        };

        let snapshot = v_flex()
            .id("kafka-overview-metrics-snapshot")
            .debug_selector(|| "kafka-overview-metrics-snapshot".into())
            .w_full()
            .min_w_0()
            .flex_none()
            .gap(px(10.0))
            .p(px(14.0))
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(12.0))
                    .when(compact, |row| row.flex_col().items_stretch())
                    .child(
                        skeleton_section_heading(theme, 132.0, 248.0)
                            .flex_1()
                            .min_w_0(),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap(px(8.0))
                            .when(compact, |row| row.w_full())
                            .child(skeleton_bar(theme, 96.0, 28.0))
                            .child(skeleton_bar(theme, 72.0, 28.0)),
                    ),
            )
            .child(
                h_flex()
                    .id("kafka-overview-loading-status")
                    .debug_selector(|| "kafka-overview-loading-status".into())
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(div().size(px(8.0)).rounded_full().bg(theme.warning))
                    .child(skeleton_bar(theme, 168.0, 8.0))
                    .child(div().flex_1().min_w_0())
                    .child(skeleton_bar(theme, 240.0, 8.0)),
            )
            .child(skeleton_table(theme, 3, &snapshot_cluster_columns))
            .child(skeleton_section_heading(theme, 116.0, 250.0))
            .child(skeleton_table(theme, 2, &snapshot_detail_columns));

        let broker_section = v_flex()
            .id("kafka-overview-loading-broker")
            .debug_selector(|| "kafka-overview-loading-broker".into())
            .w_full()
            .min_w_0()
            .flex_none()
            .gap(px(10.0))
            .child(skeleton_section_heading(theme, 154.0, 286.0))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().size(px(8.0)).rounded_full().bg(theme.warning))
                    .child(skeleton_bar(theme, 152.0, 8.0)),
            )
            .child(skeleton_table(theme, 5, &broker_columns));
        let topic_section = v_flex()
            .id("kafka-overview-loading-topic")
            .debug_selector(|| "kafka-overview-loading-topic".into())
            .w_full()
            .min_w_0()
            .flex_none()
            .gap(px(10.0))
            .child(skeleton_section_heading(theme, 96.0, 222.0))
            .child(skeleton_table(theme, 5, &[None, Some(104.0)]));
        let primary_sections = v_flex()
            .id("kafka-overview-loading-primary")
            .debug_selector(|| "kafka-overview-loading-primary".into())
            .flex_1()
            .min_w_0()
            .gap(px(18.0))
            .when(compact, |column| column.w_full().flex_initial())
            .child(broker_section)
            .child(topic_section);
        let cluster_section = v_flex()
            .id("kafka-overview-loading-cluster")
            .debug_selector(|| "kafka-overview-loading-cluster".into())
            .min_w_0()
            .gap(px(10.0))
            .when(compact, |panel| panel.w_full().flex_none())
            .when(!compact, |panel| panel.w(px(320.0)).flex_none())
            .child(skeleton_section_heading(theme, 94.0, 180.0))
            .child(
                v_flex()
                    .w_full()
                    .gap(px(8.0))
                    .p(px(14.0))
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(6.0))
                    .child(skeleton_table(theme, 4, &[Some(92.0), None])),
            );
        let sections = h_flex()
            .id("kafka-overview-loading-sections")
            .debug_selector(|| "kafka-overview-loading-sections".into())
            .w_full()
            .min_w_0()
            .flex_none()
            .items_stretch()
            .gap(px(18.0))
            .when(compact, |row| row.flex_col().items_stretch())
            .child(primary_sections)
            .child(cluster_section);
        let overview_content = v_flex()
            .id("kafka-overview-scroll")
            .debug_selector(|| "kafka-overview-scroll".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .items_stretch()
            .overflow_y_scroll()
            .track_scroll(&self.overview_scroll)
            .p(px(22.0))
            .gap(px(18.0))
            .child(metrics)
            .child(snapshot)
            .child(sections);
        let overview_content =
            loading_transition(overview_content, "kafka-overview-loading-transition");

        h_flex()
            .id("kafka-overview-scroll-viewport")
            .debug_selector(|| "kafka-overview-scroll-viewport".into())
            .flex_1()
            .w_full()
            .min_w_0()
            .min_h_0()
            .items_stretch()
            .child(overview_content)
            .child(
                div()
                    .id("kafka-overview-v-scrollbar")
                    .debug_selector(|| "kafka-overview-v-scrollbar".into())
                    .h_full()
                    .w(px(16.0))
                    .flex_none()
                    .bg(theme.scrollbar)
                    .child(
                        Scrollbar::vertical(&self.overview_scroll)
                            .id("kafka-overview-v-scrollbar-control")
                            .scrollbar_show(ScrollbarShow::Always),
                    ),
            )
            .into_any_element()
    }
}
