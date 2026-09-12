impl KafkaView {
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
        let search_mode = [
            KafkaMessageSearchMode::Literal,
            KafkaMessageSearchMode::Regex,
        ]
        .into_iter()
        .fold(
            h_flex()
                .debug_selector(|| "kafka-message-search-mode".into())
                .gap(px(4.0)),
            |row, mode| {
                let selected = self.search_mode == mode;
                row.child(
                    ramag_ui::clickable_button(SharedString::from(format!(
                        "kafka-search-mode-{}",
                        mode.label()
                    )))
                    .small()
                    .label(mode.label())
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.search_mode = mode;
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
        let message_search_mode_field = v_flex()
            .debug_selector(|| "kafka-message-search-mode-field".into())
            .flex_none()
            .gap(px(5.0))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("匹配模式"),
            )
            .child(search_mode);
        let message_search_options = h_flex()
            .debug_selector(|| "kafka-message-search-options".into())
            .min_w_0()
            .items_end()
            .gap(px(10.0))
            .when(compact, |options| options.w_full().flex_wrap())
            .child(message_search_fields)
            .child(message_search_mode_field);
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
                    .child(message_search_options)
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
