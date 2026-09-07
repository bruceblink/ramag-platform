use super::*;

impl KafkaView {
    /// 渲染持续消费的启动参数、运行状态和显式生命周期按钮；历史扫描控件保持独立。
    pub(super) fn render_message_tail_controls(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let compact = f32::from(window.viewport_size().width) < 900.0;
        let start_modes = KafkaTailStartMode::ALL.into_iter().fold(
            h_flex()
                .id("kafka-tail-start-modes")
                .debug_selector(|| "kafka-tail-start-modes".into())
                .gap(px(4.0)),
            |row, mode| {
                let selected = self.message_tail_start_mode == mode;
                row.child(
                    ramag_ui::clickable_button(SharedString::from(format!(
                        "kafka-tail-start-{}",
                        mode.label().to_lowercase()
                    )))
                    .small()
                    .label(mode.label())
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .disabled(self.message_tail_running)
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.message_tail_start_mode = mode;
                            cx.notify();
                        },
                    )),
                )
            },
        );
        let status = if !self.message_tail_running {
            ("未运行", theme.muted_foreground)
        } else if self.message_tail_paused {
            ("已暂停展示", theme.warning)
        } else if self.message_tail_connected {
            ("实时连接中", theme.success)
        } else {
            ("重连中", theme.warning)
        };
        let status_text = if self.message_tail_reconnect_attempt == 0 {
            status.0.to_owned()
        } else {
            format!(
                "{} · 第 {} 次重连",
                status.0, self.message_tail_reconnect_attempt
            )
        };
        let window_text = format!(
            "窗口 {} 条 / {} bytes · 已保留 {} 条 / {} bytes",
            self.message_tail_window_messages,
            self.message_tail_window_bytes,
            self.message_tail_records.len(),
            self.message_tail_bytes
        );
        let stats_text = format!(
            "通道丢弃 {} 条 / {} bytes · 窗口淘汰 {} 条 / {} bytes",
            self.message_tail_dropped_records,
            self.message_tail_dropped_bytes,
            self.message_tail_evicted_records,
            self.message_tail_evicted_bytes
        );
        let actions = h_flex()
            .id("kafka-tail-actions")
            .debug_selector(|| "kafka-tail-actions".into())
            .flex_none()
            .items_end()
            .gap(px(6.0))
            .child(
                ramag_ui::clickable_button("kafka-tail-start")
                    .debug_selector(|| "kafka-tail-start".into())
                    .primary()
                    .small()
                    .icon(IconName::Play)
                    .label("开始")
                    .disabled(
                        self.message_tail_running
                            || self.loading_runtime
                            || self.loading_messages
                            || self.testing
                            || self.saving
                            || self.deleting,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.start_message_tail(window, cx);
                    })),
            )
            .when(self.message_tail_running, |row| {
                row.child(
                    ramag_ui::clickable_button("kafka-tail-pause")
                        .debug_selector(|| "kafka-tail-pause".into())
                        .outline()
                        .small()
                        .icon(if self.message_tail_paused {
                            IconName::Play
                        } else {
                            IconName::Pause
                        })
                        .label(if self.message_tail_paused {
                            "恢复展示"
                        } else {
                            "暂停展示"
                        })
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.toggle_message_tail_pause(cx);
                        })),
                )
                .child(
                    ramag_ui::clickable_button("kafka-tail-stop")
                        .debug_selector(|| "kafka-tail-stop".into())
                        .outline()
                        .small()
                        .icon(IconName::Close)
                        .label("停止")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.stop_message_tail(cx);
                        })),
                )
            })
            .child(
                ramag_ui::clickable_button("kafka-tail-clear")
                    .debug_selector(|| "kafka-tail-clear".into())
                    .ghost()
                    .small()
                    .icon(IconName::Delete)
                    .label("清空窗口")
                    .disabled(self.message_tail_records.is_empty())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.clear_message_tail(cx);
                    })),
            );
        let start_inputs = h_flex()
            .debug_selector(|| "kafka-tail-start-inputs".into())
            .items_end()
            .gap(px(8.0))
            .when(compact, |row| row.flex_col().items_stretch().w_full())
            .child(field(
                "起始位置",
                start_modes,
                if compact { 0.0 } else { 230.0 },
            ))
            .when(
                self.message_tail_start_mode == KafkaTailStartMode::Offset,
                |row| {
                    row.child(field(
                        "Offset",
                        Input::new(&self.message_tail_offset_input).small(),
                        if compact { 0.0 } else { 120.0 },
                    ))
                },
            );
        let limits = h_flex()
            .id("kafka-tail-limits")
            .debug_selector(|| "kafka-tail-limits".into())
            .items_end()
            .gap(px(8.0))
            .flex_wrap()
            .when(compact, |row| row.w_full())
            .child(field(
                "窗口条数",
                Input::new(&self.message_tail_window_messages_input).small(),
                100.0,
            ))
            .child(field(
                "窗口字节数",
                Input::new(&self.message_tail_window_bytes_input).small(),
                120.0,
            ))
            .child(field(
                "单条最大字节数",
                Input::new(&self.message_tail_max_message_bytes_input).small(),
                130.0,
            ))
            .child(field(
                "轮询毫秒",
                Input::new(&self.message_tail_poll_timeout_input).small(),
                90.0,
            ));
        v_flex()
            .id("kafka-message-tail-controls")
            .debug_selector(|| "kafka-message-tail-controls".into())
            .w_full()
            .flex_none()
            .gap(px(8.0))
            .p(px(12.0))
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .bg(theme.secondary.opacity(0.22))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.0))
                    .when(compact, |row| row.flex_col().items_stretch())
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                h_flex()
                                    .gap(px(8.0))
                                    .child(div().size(px(8.0)).rounded_full().bg(status.1))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child("持续实时消费"),
                                    )
                                    .child(div().text_xs().text_color(status.1).child(status_text)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(format!("{} · {}", window_text, stats_text)),
                            ),
                    )
                    .child(actions),
            )
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_end()
                    .gap(px(8.0))
                    .when(compact, |row| row.flex_col().items_stretch())
                    .child(start_inputs)
                    .child(limits),
            )
    }

    /// 实时窗口使用独立有限列表，不与历史查询分页结果混用。
    pub(super) fn render_message_tail_panel(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let records = self
            .message_tail_records
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let show_panel = self.message_tail_running
            || !records.is_empty()
            || self.message_tail_dropped_records > 0
            || self.message_tail_evicted_records > 0;
        let rows = if records.is_empty() {
            v_flex()
                .h_full()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(if self.message_tail_running {
                    "等待实时消息…"
                } else {
                    "开始后，实时消息会显示在这里"
                })
                .into_any_element()
        } else {
            let body = uniform_list(
                "kafka-message-tail-list",
                records.len(),
                cx.processor(move |this, range: Range<usize>, _window, cx| {
                    range
                        .map(|index| {
                            let record = records[index].clone();
                            this.render_tail_message_row(
                                index,
                                record,
                                this.selected_tail_message == Some(index),
                                cx,
                            )
                            .into_any_element()
                        })
                        .collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.message_tail_scroll)
            .w_full()
            .min_w_0()
            .flex_1();
            v_flex().h_full().min_h_0().child(body).into_any_element()
        };
        let mut panel = v_flex()
            .id("kafka-message-tail")
            .debug_selector(|| "kafka-message-tail".into())
            .w_full()
            .flex_none()
            .min_h_0();
        if show_panel {
            panel = panel
                .h(px(MESSAGE_TAIL_RESULTS_HEIGHT))
                .border_1()
                .border_color(theme.border)
                .rounded(px(6.0))
                .child(
                    h_flex()
                        .h(px(34.0))
                        .flex_none()
                        .items_center()
                        .gap(px(8.0))
                        .px(px(12.0))
                        .border_b_1()
                        .border_color(theme.border)
                        .bg(theme.secondary)
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("实时窗口")
                        .child(div().flex_1().min_w_0())
                        .child(format!("{} 条", self.message_tail_records.len())),
                )
                .child(rows);
        }
        panel
    }

    fn render_tail_message_row(
        &self,
        index: usize,
        record: KafkaMessageRecord,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        h_flex()
            .id(SharedString::from(format!(
                "kafka-message-tail-row-{index}"
            )))
            .debug_selector(move || format!("kafka-message-tail-row-{index}"))
            .w_full()
            .items_center()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(7.0))
            .border_b_1()
            .border_color(theme.border)
            .when(selected, |row| row.bg(theme.accent.opacity(0.1)))
            .when(!selected, |row| {
                row.hover(|row| row.bg(theme.muted.opacity(0.5)))
            })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.selected_tail_message = Some(index);
                cx.notify();
            }))
            .child(
                div()
                    .w(px(54.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("P{}", record.partition)),
            )
            .child(
                div()
                    .w(px(88.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(record.offset.to_string()),
            )
            .child(
                div()
                    .w(px(148.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(format_timestamp(record.timestamp)),
            )
            .child(
                div()
                    .w(px(100.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(
                        record
                            .key_preview(96)
                            .map_or_else(|| "<null>".into(), |preview| preview.text),
                    ),
            )
            .child(
                div().flex_1().min_w_0().text_xs().truncate().child(
                    record
                        .value_preview(MESSAGE_PREVIEW_BYTES)
                        .map_or_else(|| "<null>".into(), |preview| preview.text),
                ),
            )
    }

    /// 显示已加载消息的分页状态；翻页只切换内存中的有界结果，不会隐式扩大 Broker 扫描。
    pub(super) fn render_message_pagination(
        &self,
        total_records: usize,
        current_page: usize,
        page_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let has_previous = current_page > 0;
        let has_next = current_page.saturating_add(1) < page_count;
        let previous_page = current_page.saturating_sub(1);
        let next_page = current_page.saturating_add(1);
        h_flex()
            .id("kafka-message-pagination")
            .debug_selector(|| "kafka-message-pagination".into())
            .w_full()
            .h(px(38.0))
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.background)
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(format!("已加载 {total_records} 条消息"))
            .child(div().flex_1().min_w_0())
            .child(
                ramag_ui::clickable_button("kafka-message-page-previous")
                    .debug_selector(|| "kafka-message-page-previous".into())
                    .ghost()
                    .small()
                    .icon(IconName::ChevronLeft)
                    .label("上页")
                    .disabled(!has_previous)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_message_page(previous_page, cx);
                    })),
            )
            .child(
                div()
                    .id("kafka-message-page-indicator")
                    .debug_selector(|| "kafka-message-page-indicator".into())
                    .flex_none()
                    .child(if page_count == 0 {
                        "第 0 / 0 页".to_string()
                    } else {
                        format!("第 {} / {} 页", current_page + 1, page_count)
                    }),
            )
            .child(
                ramag_ui::clickable_button("kafka-message-page-next")
                    .debug_selector(|| "kafka-message-page-next".into())
                    .ghost()
                    .small()
                    .icon(IconName::ChevronRight)
                    .label("下页")
                    .disabled(!has_next)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_message_page(next_page, cx);
                    })),
            )
    }

    pub(super) fn render_message_row(
        &self,
        index: usize,
        record: KafkaMessageRecord,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        h_flex()
            .id(SharedString::from(format!("kafka-message-row-{index}")))
            .debug_selector(move || format!("kafka-message-row-{index}"))
            .w_full()
            .items_center()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(9.0))
            .border_b_1()
            .border_color(theme.border)
            .when(selected, |row| row.bg(theme.accent.opacity(0.1)))
            .when(!selected, |row| {
                row.hover(|row| row.bg(theme.muted.opacity(0.5)))
            })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.selected_message = Some(index);
                cx.notify();
            }))
            .child(
                div()
                    .w(px(56.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("P{}", record.partition)),
            )
            .child(
                div()
                    .w(px(90.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(record.offset.to_string()),
            )
            .child(
                div()
                    .w(px(150.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(format_timestamp(record.timestamp)),
            )
            .child(
                div()
                    .w(px(100.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(
                        record
                            .key_preview(96)
                            .map_or_else(|| "<null>".into(), |preview| preview.text),
                    ),
            )
            .child(
                div().flex_1().min_w_0().text_sm().truncate().child(
                    record
                        .value_preview(MESSAGE_PREVIEW_BYTES)
                        .map_or_else(|| "<null>".into(), |preview| preview.text),
                ),
            )
            .child(
                div()
                    .w(px(70.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("{} headers", record.headers.len())),
            )
    }
}
