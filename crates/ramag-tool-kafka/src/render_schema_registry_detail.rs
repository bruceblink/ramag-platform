use super::*;

impl KafkaView {
    pub(super) fn render_schema_registry_detail(
        &self,
        compact: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let versions = self.render_schema_versions(&theme, compact, cx);
        let detail = self.render_schema_version_content(&theme);
        v_flex()
            .id("kafka-schema-registry-detail")
            .debug_selector(|| "kafka-schema-registry-detail".into())
            .min_h_0()
            .min_w_0()
            .p(px(12.0))
            .gap(px(10.0))
            .border_1()
            .border_color(theme.border)
            .rounded(px(6.0))
            .when(compact, |panel| panel.w_full().h(px(360.0)).flex_none())
            .when(!compact, |panel| panel.w(px(420.0)).flex_none())
            .child(section_heading(
                "Schema 版本详情",
                self.schema_selected_subject
                    .as_deref()
                    .unwrap_or("选择 Subject 查看版本"),
                &theme,
            ))
            .child(
                h_flex()
                    .id("kafka-schema-version-workspace")
                    .debug_selector(|| "kafka-schema-version-workspace".into())
                    .min_h_0()
                    .min_w_0()
                    .flex_1()
                    .items_stretch()
                    .gap(px(10.0))
                    .when(compact, |row| row.flex_col())
                    .child(versions)
                    .child(detail),
            )
    }

    fn render_schema_versions(
        &self,
        theme: &gpui_component::Theme,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut list = v_flex()
            .id("kafka-schema-version-list")
            .debug_selector(|| "kafka-schema-version-list".into())
            .min_h_0()
            .min_w_0()
            .gap(px(4.0))
            .when(compact, |panel| panel.w_full().h(px(108.0)).flex_none())
            .when(!compact, |panel| panel.w(px(128.0)).flex_none())
            .child(div().text_xs().text_color(theme.muted_foreground).child(
                if self.schema_selected_subject.is_some() {
                    "版本"
                } else {
                    "未选择 Subject"
                },
            ));

        let body = if self.schema_selected_subject.is_none() {
            div()
                .flex_1()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("选择 Subject")
                .into_any_element()
        } else if self.loading_schema_versions {
            div()
                .flex_1()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("正在读取版本…")
                .into_any_element()
        } else if let Some(error) = self.schema_versions_error.clone() {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .child(Icon::new(IconName::TriangleAlert).text_color(theme.danger))
                .child(div().text_xs().text_color(theme.danger).child(error))
                .child(
                    ramag_ui::clickable_button("kafka-schema-versions-retry")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Search)
                        .label("重试")
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            if let (Some(config), Some(subject)) =
                                (this.selected_config(), this.schema_selected_subject.clone())
                            {
                                this.load_schema_versions(config, subject, window, cx);
                            }
                        })),
                )
                .into_any_element()
        } else if self.schema_versions.is_empty() {
            div()
                .flex_1()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("没有可显示的版本")
                .into_any_element()
        } else {
            let mut rows = v_flex()
                .id("kafka-schema-version-list-content")
                .debug_selector(|| "kafka-schema-version-list-content".into())
                .w_full()
                .min_h_0()
                .gap(px(4.0));
            for version in &self.schema_versions {
                let version = *version;
                let selected = self.schema_selected_version == Some(version);
                let selector = format!("kafka-schema-version-{version}");
                rows = rows.child(
                    ramag_ui::clickable_button(SharedString::from(selector.clone()))
                        .debug_selector(move || selector.clone())
                        .ghost()
                        .small()
                        .icon(IconName::File)
                        .label(format!("Version {version}"))
                        .when(selected, |button| button.bg(theme.accent.opacity(0.15)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.select_schema_version(version, window, cx);
                        })),
                );
            }
            div()
                .id("kafka-schema-version-list-scroll")
                .debug_selector(|| "kafka-schema-version-list-scroll".into())
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.schema_versions_scroll)
                .child(rows)
                .into_any_element()
        };
        list = list.child(body);
        list
    }

    fn render_schema_version_content(&self, theme: &gpui_component::Theme) -> impl IntoElement {
        let mut panel = v_flex()
            .id("kafka-schema-version-content")
            .debug_selector(|| "kafka-schema-version-content".into())
            .flex_1()
            .min_h_0()
            .min_w_0()
            .gap(px(8.0));
        if self.loading_schema_version {
            return panel
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("正在读取 Schema 内容…");
        }
        if let Some(error) = self.schema_version_error.clone() {
            return panel
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.danger)
                .child(error);
        }
        let Some(detail) = self.schema_version_detail.as_ref() else {
            return panel
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("选择 Version 查看 Schema 内容");
        };
        panel = panel.child(
            h_flex()
                .w_full()
                .flex_wrap()
                .gap(px(8.0))
                .child(schema_meta("Version", detail.version.to_string(), theme))
                .child(schema_meta("ID", detail.id.to_string(), theme))
                .child(schema_meta(
                    "类型",
                    detail.schema_type.as_deref().unwrap_or("未声明").to_owned(),
                    theme,
                )),
        );
        panel.child(
            div()
                .id("kafka-schema-version-schema-scroll")
                .debug_selector(|| "kafka-schema-version-schema-scroll".into())
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_y_scroll()
                .track_scroll(&self.schema_version_detail_scroll)
                .p(px(8.0))
                .border_1()
                .border_color(theme.border)
                .rounded(px(4.0))
                .text_xs()
                .whitespace_normal()
                .child(detail.schema.clone()),
        )
    }
}

fn schema_meta(
    label: &'static str,
    value: String,
    theme: &gpui_component::Theme,
) -> impl IntoElement {
    v_flex()
        .min_w(px(72.0))
        .gap(px(2.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(div().text_xs().child(value))
}
