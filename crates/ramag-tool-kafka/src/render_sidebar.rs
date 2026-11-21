use super::*;
use gpui_kit::component::Selectable as _;

pub(super) fn matching_cluster_indices(clusters: &[KafkaClusterConfig], query: &str) -> Vec<usize> {
    clusters
        .iter()
        .enumerate()
        .filter(|(_, cluster)| {
            query.is_empty()
                || cluster.name.to_lowercase().contains(query)
                || cluster
                    .bootstrap_servers
                    .iter()
                    .any(|server| server.to_lowercase().contains(query))
        })
        .map(|(index, _)| index)
        .collect()
}

impl KafkaView {
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>, dialog: bool) -> impl IntoElement {
        let theme = cx.theme().clone();
        let query = value(&self.cluster_search, cx).to_lowercase();
        let visible_indices = matching_cluster_indices(&self.clusters, &query);
        let rows = if self.loading_clusters {
            v_flex()
                .id("kafka-cluster-loading")
                .debug_selector(|| "kafka-cluster-loading".into())
                .flex_1()
                .min_h_0()
                .child(loading_transition(
                    skeleton_table(&theme, if dialog { 2 } else { 4 }, &[Some(8.0), None]),
                    "kafka-cluster-loading-transition",
                ))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("正在加载本地配置…"),
                )
                .into_any_element()
        } else if self.clusters.is_empty() {
            let load_error = self.cluster_load_error.clone();
            let message = load_error
                .as_deref()
                .map_or("还没有保存的集群配置".to_owned(), |error| {
                    format!("加载配置失败：{error}")
                });
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .px(px(20.0))
                .child(Icon::new(IconName::Network).text_color(theme.muted_foreground))
                .child(
                    div()
                        .text_xs()
                        .text_center()
                        .text_color(if load_error.is_some() {
                            theme.danger
                        } else {
                            theme.muted_foreground
                        })
                        .child(message),
                )
                .when(load_error.is_some(), |view| {
                    view.child(
                        ramag_ui::clickable_button("kafka-retry-clusters")
                            .debug_selector(|| "kafka-retry-clusters".into())
                            .outline()
                            .small()
                            .icon(ramag_ui::icons::refresh_cw())
                            .label("重试")
                            .disabled(self.loading_clusters)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.retry_cluster_load(window, cx);
                            })),
                    )
                })
                .into_any_element()
        } else if visible_indices.is_empty() {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("没有匹配的集群"),
                )
                .into_any_element()
        } else {
            uniform_list(
                "kafka-cluster-list",
                visible_indices.len(),
                cx.processor(move |this, range: Range<usize>, _window, cx| {
                    range
                        .filter_map(|row| {
                            let cluster_index = *visible_indices.get(row)?;
                            let cluster = this.clusters.get(cluster_index)?.clone();
                            let selected = this.selected_cluster_id.as_ref() == Some(&cluster.id);
                            Some(
                                this.render_cluster_row(cluster, selected, cx)
                                    .into_any_element(),
                            )
                        })
                        .collect::<Vec<_>>()
                }),
            )
            .flex_1()
            .into_any_element()
        };

        v_flex()
            .id("kafka-sidebar")
            .debug_selector(|| "kafka-sidebar".into())
            .w(px(KAFKA_SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .min_h_0()
            .when(dialog, |panel| panel.w_full())
            .when(!dialog, |panel| {
                panel.border_r_1().border_color(theme.border)
            })
            .bg(ramag_ui::pulse_ui::pulse_palette(cx).surface)
            .child(
                ramag_ui::pulse_ui::pulse_home_toolbar(cx)
                    .debug_selector(|| "kafka-sidebar-header".into())
                    .min_h(px(58.0))
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.border)
                    .when(!dialog, |header| {
                        header.child(
                            h_flex()
                                .gap(px(8.0))
                                .child(Icon::new(IconName::Network).text_color(theme.accent))
                                .child(
                                    v_flex()
                                        .gap(px(1.0))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                .child("Kafka"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child("消息流工作区"),
                                        ),
                                ),
                        )
                    })
                    .child(
                        h_flex().gap(px(4.0)).child(
                            ramag_ui::clickable_button("kafka-add-profile")
                                .debug_selector(|| "kafka-add-profile".into())
                                .primary()
                                .small()
                                .icon(IconName::Plus)
                                .label("新建")
                                .tooltip("新建集群配置")
                                .disabled(self.saving || self.testing || self.deleting)
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.close_cluster_dialog(window, cx);
                                    this.new_profile(window, cx);
                                })),
                        ),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "kafka-sidebar-search".into())
                    .px(px(12.0))
                    .py(px(10.0))
                    .child(
                        ramag_ui::cleanable_input(
                            &self.cluster_search,
                            "kafka-cluster-search-clear",
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
            .child(rows)
            .child(
                div()
                    .flex_none()
                    .px(px(12.0))
                    .py(px(12.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(div().text_xs().text_color(theme.muted_foreground).child(
                        if self.loading_clusters {
                            "正在加载本地配置…".to_owned()
                        } else {
                            format!("{} 个本地配置", self.clusters.len())
                        },
                    )),
            )
    }

    pub(super) fn render_cluster_row(
        &self,
        cluster: KafkaClusterConfig,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let palette = ramag_ui::pulse_ui::pulse_palette(cx);
        let row_id = SharedString::from(format!("kafka-cluster-row-{}", cluster.id));
        let id = cluster.id.clone();
        let profile_operation_active = self.saving || self.testing || self.deleting;
        let endpoints = cluster.bootstrap_servers.join(", ");
        ramag_ui::clickable_button(row_id)
            .debug_selector(|| "kafka-cluster-row".into())
            .ghost()
            .selected(selected)
            .disabled(profile_operation_active)
            .w_full()
            .min_w_0()
            .h(px(52.0))
            .px(px(14.0))
            .when(selected, |row| row.bg(palette.selected))
            .accessibility_label(format!("{}，{}", cluster.name, endpoints))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.close_cluster_dialog(window, cx);
                this.select_cluster(id.clone(), window, cx);
            }))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(9.0))
                    .child(
                        div()
                            .size(px(8.0))
                            .flex_none()
                            .rounded_full()
                            .bg(if selected {
                                theme.accent
                            } else {
                                theme.muted_foreground
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(div().text_sm().truncate().child(cluster.name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(endpoints),
                            ),
                    ),
            )
    }
}
