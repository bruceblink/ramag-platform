use super::*;
use ramag_ui::RestrictScrollToAxisExt as _;

impl KafkaView {
    /// 渲染消息查询页；窄屏把结果区放入页面滚动范围，保证查询控件不会挤掉表格和详情。
    pub(super) fn render_messages(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let stacked_root = kafka_main_content_width(window) < 900.0;
        let compact = f32::from(window.viewport_size().width) < 1280.0;
        let page = self.message_page.as_ref();
        let page_count = self.message_page_count();
        let current_page = self.message_page_index.min(page_count.saturating_sub(1));
        let selected_record = page.and_then(|page| {
            self.selected_message
                .and_then(|index| page.records.get(index))
        });
        let rows = if self.loading_messages {
            let table_content = v_flex()
                .w_full()
                .min_w(px(MESSAGE_TABLE_MIN_WIDTH))
                .h_full()
                .child(message_table_header(&theme))
                .child(skeleton_table(
                    &theme,
                    8,
                    &[
                        Some(56.0),
                        Some(90.0),
                        Some(150.0),
                        Some(100.0),
                        None,
                        Some(70.0),
                    ],
                ));
            let table = div()
                .id("kafka-message-table")
                .debug_selector(|| "kafka-message-table".into())
                .relative()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(
                    div()
                        .id("kafka-message-h-scroll")
                        .debug_selector(|| "kafka-message-h-scroll".into())
                        .size_full()
                        .overflow_x_scroll()
                        .restrict_scroll_to_axis()
                        .track_scroll(&self.message_horizontal_scroll)
                        .child(loading_transition(
                            table_content,
                            "kafka-message-loading-transition",
                        )),
                )
                .child(
                    div()
                        .id("kafka-message-v-scrollbar")
                        .debug_selector(|| "kafka-message-v-scrollbar".into())
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_0()
                        .w(px(16.0))
                        .bg(theme.scrollbar)
                        .child(
                            Scrollbar::vertical(&self.message_scroll)
                                .id("kafka-message-v-scrollbar-control")
                                .scrollbar_show(ScrollbarShow::Always),
                        ),
                );
            let horizontal_scrollbar = div()
                .id("kafka-message-h-scrollbar")
                .debug_selector(|| "kafka-message-h-scrollbar".into())
                .flex_none()
                .w_full()
                .h(px(16.0))
                .relative()
                .bg(theme.scrollbar)
                .child(
                    Scrollbar::horizontal(&self.message_horizontal_scroll)
                        .id("kafka-message-h-scrollbar-control")
                        .scroll_size(gpui::size(px(MESSAGE_TABLE_MIN_WIDTH), px(16.0)))
                        .scrollbar_show(ScrollbarShow::Always),
                );
            v_flex()
                .id("kafka-message-loading")
                .debug_selector(|| "kafka-message-loading".into())
                .flex_1()
                .min_h_0()
                .child(table)
                .child(horizontal_scrollbar)
                .into_any_element()
        } else if let Some(page) = page {
            if page.records.is_empty() {
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("扫描范围内没有消息"),
                    )
                    .into_any_element()
            } else {
                let page_start = current_page.saturating_mul(self.message_page_size);
                let page_end = page_start
                    .saturating_add(self.message_page_size)
                    .min(page.records.len());
                let page_len = page_end.saturating_sub(page_start);
                let header = message_table_header(&theme);
                let body = uniform_list(
                    "kafka-message-list",
                    page_len,
                    cx.processor(move |this, range: Range<usize>, _window, cx| {
                        range
                            .filter_map(|index| {
                                let record_index = page_start.saturating_add(index);
                                let record = this
                                    .message_page
                                    .as_ref()?
                                    .records
                                    .get(record_index)?
                                    .clone();
                                let selected = this.selected_message == Some(record_index);
                                Some(
                                    this.render_message_row(record_index, record, selected, cx)
                                        .into_any_element(),
                                )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&self.message_scroll)
                .w_full()
                .h_full()
                .min_w(px(MESSAGE_TABLE_MIN_WIDTH))
                .flex_1();
                let table_content = v_flex()
                    .w_full()
                    .min_w(px(MESSAGE_TABLE_MIN_WIDTH))
                    .h_full()
                    .child(header)
                    .child(body);
                let table = div()
                    .id("kafka-message-table")
                    .debug_selector(|| "kafka-message-table".into())
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(
                        div()
                            .id("kafka-message-h-scroll")
                            .debug_selector(|| "kafka-message-h-scroll".into())
                            .size_full()
                            .overflow_x_scroll()
                            .restrict_scroll_to_axis()
                            .track_scroll(&self.message_horizontal_scroll)
                            .child(table_content),
                    )
                    .child(
                        div()
                            .id("kafka-message-v-scrollbar")
                            .debug_selector(|| "kafka-message-v-scrollbar".into())
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(px(16.0))
                            .bg(theme.scrollbar)
                            .child(
                                Scrollbar::vertical(&self.message_scroll)
                                    .id("kafka-message-v-scrollbar-control")
                                    .scrollbar_show(ScrollbarShow::Always),
                            ),
                    );
                let horizontal_scrollbar = div()
                    .id("kafka-message-h-scrollbar")
                    .debug_selector(|| "kafka-message-h-scrollbar".into())
                    .flex_none()
                    .w_full()
                    .h(px(16.0))
                    .relative()
                    .bg(theme.scrollbar)
                    .child(
                        Scrollbar::horizontal(&self.message_horizontal_scroll)
                            .id("kafka-message-h-scrollbar-control")
                            .scroll_size(gpui::size(px(MESSAGE_TABLE_MIN_WIDTH), px(16.0)))
                            .scrollbar_show(ScrollbarShow::Always),
                    );
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(table)
                    .child(horizontal_scrollbar)
                    .into_any_element()
            }
        } else {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .child(Icon::new(IconName::Search).text_color(theme.muted_foreground))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("设置范围后读取消息"),
                )
                .into_any_element()
        };
        let detail = if self.loading_messages {
            let skeleton = skeleton_detail_panel(&theme, 118.0, 210.0, 6, &[None, Some(128.0)]);
            v_flex()
                .id("kafka-message-detail-loading")
                .debug_selector(|| "kafka-message-detail-loading".into())
                .when(compact, |view| view.w_full().flex_1().min_w_0())
                .when(!compact, |view| view.w(px(360.0)).flex_none())
                .min_h_0()
                .child(loading_transition(
                    skeleton,
                    "kafka-message-detail-loading-transition",
                ))
                .into_any_element()
        } else {
            selected_record
                .map(|record| {
                    self.render_message_detail(record, compact, cx)
                        .into_any_element()
                })
                .unwrap_or_else(|| {
                    v_flex()
                        .when(compact, |view| view.w_full().flex_1().min_w_0())
                        .when(!compact, |view| view.w(px(360.0)).flex_none())
                        .min_h_0()
                        .items_center()
                        .justify_center()
                        .px(px(20.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("选择一条消息查看完整预览"),
                        )
                        .into_any_element()
                })
        };
        let pagination = page.map(|page| {
            self.render_message_pagination(page.records.len(), current_page, page_count, cx)
        });
        v_flex()
            .id("kafka-messages")
            .debug_selector(|| "kafka-messages".into())
            .flex_1()
            .min_w_0()
            .min_h_0()
            .when(stacked_root, |page| {
                page.overflow_y_scroll()
                    .track_scroll(&self.message_page_scroll)
            })
            .p(px(18.0))
            .gap(px(12.0))
            .child(self.render_message_controls(window, cx))
            .child(self.render_message_producer(window, cx))
            .when(
                self.selected_topic.is_some() || !value(&self.topic_input, cx).is_empty(),
                |page| page.child(self.render_message_tail_panel(window, cx)),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .when(compact, |row| row.flex_col())
                     .when(stacked_root, |row| {
                        row.h(px(COMPACT_MESSAGE_RESULTS_HEIGHT))
                    })
                    .gap(px(14.0))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                         .border_1()
                         .border_color(theme.border)
                         .rounded(px(6.0))
                         .when(stacked_root, |panel| panel.min_h(px(260.0)))
                         .child(rows)
                            .when_some(pagination, |panel, pagination| panel.child(pagination)),
                    )
                    .child(detail),
            )
    }

    pub(super) fn render_message_controls(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let compact = f32::from(window.viewport_size().width) < 1080.0;
        let range_modes = [KafkaRangeMode::Offset, KafkaRangeMode::Time]
            .into_iter()
            .fold(
                h_flex()
                    .debug_selector(|| "kafka-range-mode".into())
                    .gap(px(4.0)),
                |row, mode| {
                    let selected = self.range_mode == mode;
                    row.child(
                        ramag_ui::clickable_button(SharedString::from(format!(
                            "kafka-range-{}",
                            mode.label()
                        )))
                        .small()
                        .label(mode.label())
                        .when(selected, |button| button.primary())
                        .when(!selected, |button| button.ghost())
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                this.range_mode = mode;
                                cx.notify();
                            },
                        )),
                    )
                },
            );
        let range_inputs = match self.range_mode {
            KafkaRangeMode::Offset => h_flex()
                .debug_selector(|| "kafka-range-inputs".into())
                .flex_none()
                .items_end()
                .gap(px(8.0))
                .child(
                    field(
                        "起始 Offset",
                        Input::new(&self.start_offset_input).small(),
                        if compact { 0.0 } else { 130.0 },
                    )
                    .debug_selector(|| "kafka-range-start-field".into()),
                )
                .child(
                    field(
                        "结束 Offset",
                        Input::new(&self.end_offset_input).small(),
                        if compact { 0.0 } else { 130.0 },
                    )
                    .debug_selector(|| "kafka-range-end-field".into()),
                )
                .when(compact, |inputs| inputs.w_full().flex_col().items_stretch())
                .into_any_element(),
            KafkaRangeMode::Time => h_flex()
                .debug_selector(|| "kafka-range-inputs".into())
                .flex_none()
                .items_end()
                .gap(px(8.0))
                .child(
                    field(
                        "起始时间",
                        Input::new(&self.start_time_input).small(),
                        if compact { 0.0 } else { 230.0 },
                    )
                    .debug_selector(|| "kafka-range-start-field".into()),
                )
                .child(
                    field(
                        "结束时间",
                        Input::new(&self.end_time_input).small(),
                        if compact { 0.0 } else { 230.0 },
                    )
                    .debug_selector(|| "kafka-range-end-field".into()),
                )
                .when(compact, |inputs| inputs.w_full().flex_col().items_stretch())
                .into_any_element(),
        };
        let message_actions = h_flex()
            .debug_selector(|| "kafka-message-actions".into())
            .flex_none()
            .items_end()
            .gap(px(8.0))
            .child(field(
                "Limit",
                Input::new(&self.max_records_input).small(),
                90.0,
            ))
            .child(
                ramag_ui::clickable_button("kafka-read-messages")
                    .debug_selector(|| "kafka-read-messages".into())
                    .primary()
                    .small()
                    .icon(IconName::Search)
                    .label("读取")
                    .loading(self.loading_messages)
                    .disabled(
                        self.loading_runtime
                            || self.loading_messages
                            || self.testing
                            || self.saving,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.read_messages(window, cx);
                    })),
            )
            .when(self.loading_messages, |row| {
                row.child(
                    ramag_ui::clickable_button("kafka-cancel-messages")
                        .debug_selector(|| "kafka-cancel-messages".into())
                        .outline()
                        .small()
                        .icon(IconName::Close)
                        .label("取消")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.cancel_message_read(cx);
                        })),
                )
            });
        let message_actions =
            message_actions.when(compact, |row| row.w_full().flex_wrap().items_end());
        let search_fields = KafkaMessageSearchField::all().into_iter().enumerate().fold(
            h_flex().gap(px(4.0)),
            |row, (index, field)| {
                let selected = self.search_fields[index];
                row.child(
                    ramag_ui::clickable_button(SharedString::from(format!(
                        "kafka-search-field-{index}"
                    )))
                    .small()
                    .label(search_field_label(field))
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.search_fields[index] = !this.search_fields[index];
                            cx.notify();
                        },
                    )),
                )
            },
        );
        // 将查询条件和操作控件分成可收缩的布局组，避免固定宽度控件把搜索区域推出窗口。
        let message_query = h_flex()
            .debug_selector(|| "kafka-message-query".into())
            .flex_1()
            .min_w_0()
            .flex_wrap()
            .items_end()
            .gap(px(8.0))
            .child(field(
                "Topic",
                Input::new(&self.topic_input).small(),
                if compact { 0.0 } else { 190.0 },
            ))
            .child(field(
                "Partition",
                Input::new(&self.partition_input).small(),
                if compact { 0.0 } else { 130.0 },
            ))
            .child(field(
                "范围",
                range_modes,
                if compact { 0.0 } else { 134.0 },
            ))
            .child(range_inputs)
            .when(compact, |query| query.w_full().flex_col().items_stretch());
        let message_search = v_flex()
            .debug_selector(|| "kafka-message-search".into())
            .when(compact, |search| search.w_full())
            .when(!compact, |search| search.w(px(260.0)))
            .flex_none()
            .gap(px(5.0))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("搜索内容（可选）"),
            )
            .child(
                ramag_ui::cleanable_input(
                    &self.message_search,
                    "kafka-message-search-clear",
                    false,
                    cx,
                )
                .small()
                .prefix(
                    Icon::new(IconName::Search)
                        .small()
                        .text_color(theme.muted_foreground),
                ),
            );
        let message_search_fields = v_flex()
            .debug_selector(|| "kafka-message-search-fields".into())
            .when(compact, |fields| fields.w_full())
            .when(!compact, |fields| fields.w(px(208.0)))
            .flex_none()
            .gap(px(5.0))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("搜索字段"),
            )
            .child(search_fields);
        let show_tail_controls =
            self.selected_topic.is_some() || !value(&self.topic_input, cx).is_empty();
        v_flex()
            .w_full()
            .flex_none()
            .gap(px(8.0))
            .when(show_tail_controls, |view| {
                view.child(self.render_message_tail_controls(window, cx))
            })
            .child(
                h_flex()
                    .debug_selector(|| "kafka-message-query-row".into())
                    .w_full()
                    .min_w_0()
                    .items_end()
                    .gap(px(8.0))
                    .when(compact, |row| row.flex_col().items_stretch())
                    .child(message_query)
                    .child(message_actions),
            )
            .child(
                h_flex()
                    .debug_selector(|| "kafka-message-search-row".into())
                    .w_full()
                    .min_w_0()
                    .items_end()
                    .gap(px(8.0))
                    .when(compact, |row| row.flex_col().items_stretch())
                    .child(message_search)
                    .child(message_search_fields)
                    .child(
                        div()
                            .debug_selector(|| "kafka-message-search-note".into())
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .text_right()
                            .truncate()
                            .child("只读扫描 · 不提交 Offset"),
                    ),
            )
    }
}
