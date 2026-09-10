use super::*;

/// 只保存匹配结果的源索引；列表重绘时再读取当前 Subject 快照。
pub(super) fn matching_schema_subject_indices(
    subjects: &[KafkaSchemaRegistrySubject],
    query: &str,
) -> Vec<usize> {
    let query = query.to_lowercase();
    subjects
        .iter()
        .enumerate()
        .filter_map(|(index, subject)| {
            (query.is_empty() || subject.name.to_lowercase().contains(&query)).then_some(index)
        })
        .collect()
}

impl KafkaView {
    pub(super) fn invalidate_schema_registry_request(&mut self) {
        self.schema_registry_request_id = self.schema_registry_request_id.wrapping_add(1);
        self.loading_schema_subjects = false;
        if let Some(cancelled) = self.schema_registry_cancelled.take() {
            cancelled.store(true, Ordering::Release);
        }
        self.invalidate_schema_version_requests();
    }

    pub(super) fn clear_schema_registry_snapshot(&mut self) {
        self.invalidate_schema_registry_request();
        self.schema_subjects.clear();
        self.schema_subjects_loaded = false;
        self.schema_subject_error = None;
        self.schema_subject_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
        self.clear_schema_version_snapshot();
    }

    /// 读取当前集群配置的 Subject 列表；未配置端点时不发起网络请求。
    pub(super) fn load_schema_subjects(
        &mut self,
        config: KafkaClusterConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading_schema_subjects
            || self.loading_runtime
            || self.saving
            || self.deleting
            || self.selected_cluster_id.as_ref() != Some(&config.id)
        {
            return;
        }
        if config.schema_registry.endpoint.is_none() {
            self.clear_schema_registry_snapshot();
            self.schema_subjects_loaded = true;
            cx.notify();
            return;
        }

        self.schema_registry_request_id = self.schema_registry_request_id.wrapping_add(1);
        let request_id = self.schema_registry_request_id;
        let cluster_id = config.id.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let service = self.service.clone();
        self.schema_registry_cancelled = Some(cancelled.clone());
        self.loading_schema_subjects = true;
        self.schema_subjects_loaded = false;
        self.schema_subject_error = None;
        self.notice = Some(("正在读取 Schema Registry Subject…".into(), false));
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .list_schema_subjects_with_cancel(&config, cancelled)
                .await;
            let _ = this.update_in(cx, |this, _window, cx| {
                if this.schema_registry_request_id != request_id
                    || this.selected_cluster_id.as_ref() != Some(&cluster_id)
                {
                    return;
                }
                this.loading_schema_subjects = false;
                this.schema_registry_cancelled = None;
                this.schema_subjects_loaded = true;
                match result {
                    Ok(subjects) => {
                        let count = subjects.len();
                        this.schema_subjects = subjects;
                        this.schema_subject_error = None;
                        this.notice = Some((format!("已读取 {count} 个 Schema Subject"), false));
                    }
                    Err(error) => {
                        this.schema_subjects.clear();
                        this.schema_subject_error = Some(error.user_message());
                        this.notice = Some((
                            format!(
                                "读取 Schema Registry Subject 失败：{}",
                                error.user_message()
                            ),
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

    pub(super) fn render_schema_registry(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let compact = kafka_main_content_width(window) < 700.0;
        let config = self.selected_config();
        let configured = config
            .as_ref()
            .is_some_and(|config| config.schema_registry.endpoint.is_some());
        let query = value(&self.schema_subject_search, cx).to_lowercase();
        let visible_indices = matching_schema_subject_indices(&self.schema_subjects, &query);
        let visible_count = visible_indices.len();
        let list = if self.loading_runtime {
            h_flex()
                .id("kafka-schema-subject-table")
                .debug_selector(|| "kafka-schema-subject-table".into())
                .size_full()
                .items_stretch()
                .child(
                    v_flex()
                        .id("kafka-schema-subject-list-content")
                        .debug_selector(|| "kafka-schema-subject-list-content".into())
                        .h_full()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .child(loading_transition(
                            skeleton_table(&theme, 7, &[None]),
                            "kafka-schema-subject-loading-transition",
                        )),
                )
                .child(
                    div()
                        .id("kafka-schema-subject-v-scrollbar")
                        .debug_selector(|| "kafka-schema-subject-v-scrollbar".into())
                        .h_full()
                        .w(px(KAFKA_SCHEMA_SUBJECT_SCROLLBAR_WIDTH))
                        .flex_none()
                        .bg(theme.scrollbar)
                        .child(
                            Scrollbar::vertical(&self.schema_subject_scroll)
                                .id("kafka-schema-subject-v-scrollbar-control")
                                .scrollbar_show(ScrollbarShow::Always),
                        ),
                )
                .into_any_element()
        } else if !configured {
            empty_state(
                "未配置 Schema Registry",
                "请在配置页填写 Schema Registry 地址并保存",
                &theme,
            )
            .into_any_element()
        } else if self.loading_schema_subjects {
            h_flex()
                .id("kafka-schema-subject-table")
                .debug_selector(|| "kafka-schema-subject-table".into())
                .size_full()
                .items_stretch()
                .child(
                    v_flex()
                        .id("kafka-schema-subject-list-content")
                        .debug_selector(|| "kafka-schema-subject-list-content".into())
                        .h_full()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .child(loading_transition(
                            skeleton_table(&theme, 7, &[None]),
                            "kafka-schema-subject-loading-transition",
                        )),
                )
                .child(
                    div()
                        .id("kafka-schema-subject-v-scrollbar")
                        .debug_selector(|| "kafka-schema-subject-v-scrollbar".into())
                        .h_full()
                        .w(px(KAFKA_SCHEMA_SUBJECT_SCROLLBAR_WIDTH))
                        .flex_none()
                        .bg(theme.scrollbar)
                        .child(
                            Scrollbar::vertical(&self.schema_subject_scroll)
                                .id("kafka-schema-subject-v-scrollbar-control")
                                .scrollbar_show(ScrollbarShow::Always),
                        ),
                )
                .into_any_element()
        } else if let Some(error) = self.schema_subject_error.clone() {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .child(Icon::new(IconName::TriangleAlert).text_color(theme.danger))
                .child(div().text_xs().text_color(theme.danger).child(error))
                .child(
                    ramag_ui::clickable_button("kafka-schema-registry-retry")
                        .outline()
                        .small()
                        .icon(IconName::Search)
                        .label("重试")
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            if let Some(config) = this.selected_config() {
                                this.clear_schema_registry_snapshot();
                                this.load_schema_subjects(config, window, cx);
                            }
                        })),
                )
                .into_any_element()
        } else if visible_indices.is_empty() {
            empty_state(
                if self.schema_subjects.is_empty() {
                    "Schema Registry 没有 Subject"
                } else {
                    "没有匹配的 Subject"
                },
                "可修改筛选条件或刷新列表",
                &theme,
            )
            .into_any_element()
        } else {
            let visible_indices = Arc::new(visible_indices);
            let row_theme = theme.clone();
            let rows = uniform_list(
                "kafka-schema-subject-list",
                visible_count,
                cx.processor(move |this, range: Range<usize>, _window, cx| {
                    range
                        .filter_map(|index| {
                            let subject_index = *visible_indices.get(index)?;
                            let subject = this.schema_subjects.get(subject_index)?;
                            let selected =
                                this.schema_selected_subject.as_ref() == Some(&subject.name);
                            Some(
                                this.render_schema_subject_row(
                                    index,
                                    subject.name.clone(),
                                    selected,
                                    &row_theme,
                                    cx,
                                )
                                .into_any_element(),
                            )
                        })
                        .collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.schema_subject_scroll)
            .flex_1()
            .min_h_0()
            .min_w_0();
            h_flex()
                .id("kafka-schema-subject-table")
                .debug_selector(|| "kafka-schema-subject-table".into())
                .size_full()
                .items_stretch()
                .child(
                    v_flex()
                        .id("kafka-schema-subject-list-content")
                        .debug_selector(|| "kafka-schema-subject-list-content".into())
                        .h_full()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .child(rows),
                )
                .child(
                    div()
                        .id("kafka-schema-subject-v-scrollbar")
                        .debug_selector(|| "kafka-schema-subject-v-scrollbar".into())
                        .h_full()
                        .w(px(KAFKA_SCHEMA_SUBJECT_SCROLLBAR_WIDTH))
                        .flex_none()
                        .bg(theme.scrollbar)
                        .child(
                            Scrollbar::vertical(&self.schema_subject_scroll)
                                .id("kafka-schema-subject-v-scrollbar-control")
                                .scrollbar_show(ScrollbarShow::Always),
                        ),
                )
                .into_any_element()
        };
        v_flex()
            .id("kafka-schema-registry")
            .debug_selector(|| "kafka-schema-registry".into())
            .size_full()
            .min_w_0()
            .min_h_0()
            .p(px(18.0))
            .gap(px(12.0))
            .child(
                h_flex()
                    .id("kafka-schema-registry-header")
                    .debug_selector(|| "kafka-schema-registry-header".into())
                    .w_full()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .when(compact, |row| row.flex_col().items_stretch())
                    .child(section_heading(
                        "Schema Registry",
                        "只读取 Subject、版本和 Schema 内容；不执行 Registry 写操作",
                        &theme,
                    ))
                    .child(
                        h_flex()
                            .id("kafka-schema-registry-actions")
                            .debug_selector(|| "kafka-schema-registry-actions".into())
                            .flex_none()
                            .items_center()
                            .gap(px(8.0))
                            .when(compact, |row| row.w_full())
                            .child(
                                div().min_w_0().when(compact, |label| label.flex_1()).child(
                                    ramag_ui::cleanable_input(
                                        &self.schema_subject_search,
                                        "kafka-schema-subject-search-clear",
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
                                    ramag_ui::clickable_button("kafka-schema-registry-refresh")
                                        .outline()
                                        .small()
                                        .icon(IconName::Search)
                                        .label("刷新")
                                        .loading(self.loading_schema_subjects)
                                        .disabled(
                                            self.loading_schema_subjects
                                                || self.loading_runtime
                                                || self.saving
                                                || self.deleting,
                                        )
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, window, cx| {
                                                if let Some(config) = this.selected_config() {
                                                    this.clear_schema_registry_snapshot();
                                                    this.load_schema_subjects(config, window, cx);
                                                }
                                            },
                                        )),
                                )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .id("kafka-schema-registry-content")
                    .debug_selector(|| "kafka-schema-registry-content".into())
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .items_stretch()
                    .gap(px(12.0))
                    .when(compact, |row| row.flex_col())
                    .child(
                        v_flex()
                            .id("kafka-schema-registry-list-panel")
                            .debug_selector(|| "kafka-schema-registry-list-panel".into())
                            .min_h_0()
                            .min_w_0()
                            .border_1()
                            .border_color(theme.border)
                            .rounded(px(6.0))
                            .when(compact, |panel| panel.w_full().h(px(300.0)).flex_none())
                            .when(!compact, |panel| panel.flex_1())
                            .child(list),
                    )
                    .child(self.render_schema_registry_detail(compact, window, cx)),
            )
    }
}

fn empty_state(
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

impl KafkaView {
    fn render_schema_subject_row(
        &self,
        index: usize,
        name: String,
        selected: bool,
        theme: &gpui_component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let name_for_click = name.clone();
        h_flex()
            .id(SharedString::from(format!(
                "kafka-schema-subject-row-{index}"
            )))
            .debug_selector(move || format!("kafka-schema-subject-row-{index}"))
            .w_full()
            .min_w_0()
            .items_center()
            .px(px(12.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(theme.border)
            .when(selected, |row| row.bg(theme.accent.opacity(0.1)))
            .when(!selected, |row| {
                row.hover(|row| row.bg(theme.muted.opacity(0.5)))
            })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.select_schema_subject(name_for_click.clone(), window, cx);
            }))
            .child(div().min_w_0().flex_1().truncate().text_sm().child(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_search_is_case_insensitive_and_preserves_source_order() {
        let subjects = vec![
            KafkaSchemaRegistrySubject {
                name: "orders-value".into(),
            },
            KafkaSchemaRegistrySubject {
                name: "Payments-Value".into(),
            },
            KafkaSchemaRegistrySubject {
                name: "orders-key".into(),
            },
        ];

        assert_eq!(
            matching_schema_subject_indices(&subjects, "ORDERS"),
            vec![0, 2]
        );
    }
}
