use std::collections::HashSet;

use gpui_component::WindowExt as _;
use serde_json::Value;

const MAX_MESSAGE_VIEW_BYTES: usize = 2 * 1024 * 1024;
const MAX_JSON_TREE_NODES: usize = 2048;
const MAX_JSON_TREE_DEPTH: usize = 32;
const MAX_JSON_LABEL_BYTES: usize = 512;
const MESSAGE_VIEW_MIN_HEIGHT: f32 = 160.0;
const MESSAGE_VIEW_MAX_HEIGHT: f32 = 520.0;

struct MqttMessageViewer {
    message: MqttMessage,
    format: MqttPayloadFormat,
    json_tree: Option<JsonTree>,
    json_tree_error: Option<String>,
}

struct JsonTree {
    value: Value,
    expanded: HashSet<Vec<usize>>,
}

impl MqttView {
    fn open_message_viewer(
        &mut self,
        message: MqttMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let topic = message.topic.clone();
        let format = self.receive_payload_format;
        let (json_tree, json_tree_error) = if format == MqttPayloadFormat::Json {
            JsonTree::from_payload(&message.payload)
        } else {
            (None, None)
        };
        let viewer = cx.new(|_| MqttMessageViewer {
            message,
            format,
            json_tree,
            json_tree_error,
        });
        window.open_dialog(cx, move |dialog, window, _| {
            let viewer_for_content = viewer.clone();
            dialog
                .title(ramag_ui::closable_dialog_title(
                    "mqtt-message-viewer-close",
                    format!("查看消息 · {topic}"),
                    |_, _| {},
                ))
                .close_button(false)
                .width(ramag_ui::responsive_dialog_width(window, 960.0))
                .max_h(ramag_ui::responsive_dialog_max_height(window))
                .margin_top(ramag_ui::responsive_dialog_top(window))
                .content(move |content, _, _| content.child(viewer_for_content.clone()))
        });
    }
}

impl MqttMessageViewer {
    fn set_format(&mut self, format: MqttPayloadFormat) {
        self.format = format;
        if format == MqttPayloadFormat::Json {
            (self.json_tree, self.json_tree_error) =
                JsonTree::from_payload(&self.message.payload);
        } else {
            self.json_tree = None;
            self.json_tree_error = None;
        }
    }

    fn toggle_json_path(&mut self, path: Vec<usize>, cx: &mut Context<Self>) {
        let Some(tree) = self.json_tree.as_mut() else {
            return;
        };
        if !tree.expanded.remove(&path) {
            tree.expanded.insert(path);
        }
        cx.notify();
    }

    fn copy_topic(&self, window: &mut Window, cx: &mut Context<Self>) {
        ramag_ui::copy_text_with_notification(self.message.topic.clone(), window, cx);
    }

    fn copy_payload(&self, window: &mut Window, cx: &mut Context<Self>) {
        let (text, _) = bounded_message_view_text(self.format, &self.message.payload);
        ramag_ui::copy_text_with_notification(text, window, cx);
    }

    fn render_format_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut toolbar = h_flex()
            .id("mqtt-message-viewer-formats")
            .debug_selector(|| "mqtt-message-viewer-formats".into())
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(5.0))
            .child(div().text_xs().child("显示格式"));
        for format in MqttPayloadFormat::ALL {
            let label = format.label();
            let selector = format!("mqtt-message-viewer-format-{label}");
            let mut button = ramag_ui::clickable_button(selector.clone())
                .debug_selector(move || selector.clone())
                .xsmall()
                .label(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.set_format(format);
                    cx.notify();
                }));
            button = if self.format == format {
                button.primary()
            } else {
                button.ghost()
            };
            toolbar = toolbar.child(button);
        }
        toolbar
    }

    fn render_metadata(&self, theme: &gpui_component::Theme) -> gpui::AnyElement {
        let mut metadata = h_flex()
            .id("mqtt-message-viewer-meta")
            .debug_selector(|| "mqtt-message-viewer-meta".into())
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(10.0))
            .text_xs()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(self.message.topic.clone()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child(format!("QoS {} · {}", self.message.qos.as_u8(), self.message.received_at)),
            );
        if self.message.retain {
            metadata = metadata.child(message_badge("mqtt-message-viewer-retain", "Retain", theme));
        }
        if self.message.duplicate {
            metadata = metadata.child(message_badge("mqtt-message-viewer-duplicate", "Dup", theme));
        }
        if !self.message.user_properties.is_empty() {
            metadata = metadata.child(message_badge(
                "mqtt-message-viewer-properties",
                format!("属性 {}", self.message.user_properties.len()),
                theme,
            ));
        }
        metadata.into_any_element()
    }

    fn render_json_payload(
        &self,
        theme: &gpui_component::Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(tree) = &self.json_tree else {
            return div()
                .w_full()
                .min_w_0()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(
                    self.json_tree_error
                        .clone()
                        .unwrap_or_else(|| "JSON 树不可用".into()),
                )
                .into_any_element();
        };
        render_json_tree_node(
            &tree.value,
            &tree.expanded,
            Vec::new(),
            "$".into(),
            0,
            theme,
            cx,
        )
    }

    fn render_payload(
        &self,
        text: String,
        theme: &gpui_component::Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if self.format != MqttPayloadFormat::Json {
            return ramag_ui::SelectableText::new("mqtt-message-viewer-content", text)
                .w_full()
                .min_w_0()
                .text_color(theme.foreground)
                .into_any_element();
        }
        if self.json_tree.is_some() {
            return self.render_json_payload(theme, cx);
        }
        v_flex()
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.warning)
                    .child(
                        self.json_tree_error
                            .clone()
                            .unwrap_or_else(|| "JSON 树不可用".into()),
                    ),
            )
            .child(
                ramag_ui::SelectableText::new("mqtt-message-viewer-content", text)
                    .w_full()
                    .min_w_0()
                    .text_color(theme.foreground),
            )
            .into_any_element()
    }
}

impl Render for MqttMessageViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (text, truncated) = bounded_message_view_text(self.format, &self.message.payload);
        let viewport = window.viewport_size();
        let content_height = (f32::from(viewport.height) - 260.0)
            .clamp(MESSAGE_VIEW_MIN_HEIGHT, MESSAGE_VIEW_MAX_HEIGHT);
        let status = if truncated {
            format!("仅显示前 {} MiB", MAX_MESSAGE_VIEW_BYTES / 1024 / 1024)
        } else {
            format!("{} 字节", self.message.payload.len())
        };
        let copy_topic = ramag_ui::clickable_button("mqtt-message-viewer-copy-topic")
            .debug_selector(|| "mqtt-message-viewer-copy-topic".into())
            .ghost()
            .small()
            .icon(IconName::Copy)
            .label("复制 Topic")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.copy_topic(window, cx);
            }));
        let copy_payload = ramag_ui::clickable_button("mqtt-message-viewer-copy-payload")
            .debug_selector(|| "mqtt-message-viewer-copy-payload".into())
            .ghost()
            .small()
            .icon(IconName::Copy)
            .label("复制当前格式")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.copy_payload(window, cx);
            }));
        v_flex()
            .id("mqtt-message-viewer")
            .debug_selector(|| "mqtt-message-viewer".into())
            .w_full()
            .min_w_0()
            .gap(px(8.0))
            .child(self.render_metadata(&theme))
            .child(
                div()
                    .text_xs()
                    .text_color(if truncated {
                        theme.warning
                    } else {
                        theme.muted_foreground
                    })
                    .child(status),
            )
            .child(self.render_format_toolbar(cx))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .gap(px(6.0))
                    .child(copy_topic)
                    .child(copy_payload),
            )
            .child(
                div()
                    .id("mqtt-message-viewer-content-frame")
                    .debug_selector(|| "mqtt-message-viewer-content-frame".into())
                    .w_full()
                    .min_w_0()
                    .h(px(content_height))
                    .overflow_y_scroll()
                    .p(px(12.0))
                    .bg(theme.background)
                    .child(self.render_payload(text, &theme, cx)),
            )
    }
}

fn bounded_message_view_text(format: MqttPayloadFormat, payload: &[u8]) -> (String, bool) {
    let rendered = format_received_payload(format, payload);
    if rendered.len() <= MAX_MESSAGE_VIEW_BYTES {
        return (rendered, false);
    }
    const TRUNCATION_PREFIX: &str = "\n\n[消息内容已截断，原始显示结果字节数：";
    let suffix = format!("{TRUNCATION_PREFIX}{}]", rendered.len());
    let mut end = MAX_MESSAGE_VIEW_BYTES.saturating_sub(suffix.len());
    while end > 0 && !rendered.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}{}", &rendered[..end], suffix), true)
}

impl JsonTree {
    fn from_payload(payload: &[u8]) -> (Option<Self>, Option<String>) {
        if payload.len() > MAX_MESSAGE_VIEW_BYTES {
            return (
                None,
                Some("JSON 树查看仅支持不超过 2 MiB 的载荷，请切换格式查看有界文本".into()),
            );
        }
        let value = match serde_json::from_slice::<Value>(payload) {
            Ok(value) => value,
            Err(error) => return (None, Some(format!("JSON 解析失败：{error}"))),
        };
        let mut nodes = 0;
        if let Err(error) = validate_json_tree(&value, 0, &mut nodes) {
            return (None, Some(error));
        }
        (
            Some(Self {
                value,
                expanded: HashSet::from([Vec::new()]),
            }),
            None,
        )
    }
}

fn validate_json_tree(
    value: &Value,
    depth: usize,
    nodes: &mut usize,
) -> std::result::Result<(), String> {
    *nodes = nodes.saturating_add(1);
    if *nodes > MAX_JSON_TREE_NODES {
        return Err(format!(
            "JSON 树节点超过上限 {MAX_JSON_TREE_NODES}，请切换格式查看文本"
        ));
    }
    if depth > MAX_JSON_TREE_DEPTH {
        return Err(format!(
            "JSON 树嵌套超过 {MAX_JSON_TREE_DEPTH} 层，请切换格式查看文本"
        ));
    }
    match value {
        Value::Array(items) => {
            for item in items {
                validate_json_tree(item, depth + 1, nodes)?;
            }
        }
        Value::Object(fields) => {
            for item in fields.values() {
                validate_json_tree(item, depth + 1, nodes)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn render_json_tree_node(
    value: &Value,
    expanded: &HashSet<Vec<usize>>,
    path: Vec<usize>,
    label: String,
    depth: usize,
    theme: &gpui_component::Theme,
    cx: &mut Context<MqttMessageViewer>,
) -> gpui::AnyElement {
    let children = match value {
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, item)| (format!("[{index}]"), item, index))
            .collect::<Vec<_>>(),
        Value::Object(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, (key, item))| (truncate_json_label(key), item, index))
            .collect::<Vec<_>>(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Vec::new(),
    };
    let has_children = !children.is_empty();
    let is_expanded = expanded.contains(&path);
    let selector = json_tree_selector(&path);
    let path_for_click = path.clone();
    let mut row = h_flex()
        .id(selector.clone())
        .debug_selector(move || selector.to_string())
        .w_full()
        .min_w_0()
        .items_center()
        .gap(px(4.0))
        .pl(px((depth * 16) as f32))
        .py(px(2.0))
        .rounded(px(3.0));
    if has_children {
        row = row
            .cursor_pointer()
            .hover({
                let muted = theme.muted;
                move |row| row.bg(muted)
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.toggle_json_path(path_for_click.clone(), cx);
            }));
    }
    row = row.child(
        div().flex_none().w(px(14.0)).child(
            Icon::new(if is_expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .xsmall()
            .text_color(if has_children {
                theme.muted_foreground
            } else {
                theme.background
            }),
        ),
    );
    row = row.child(
        div()
            .flex_none()
            .text_xs()
            .text_color(theme.foreground)
            .child(label),
    );
    row = row.child(
        div()
            .flex_1()
            .min_w_0()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(if has_children {
                json_container_summary(value)
            } else {
                json_scalar_preview(value)
            }),
    );
    let mut node = v_flex().w_full().min_w_0().child(row);
    if has_children && is_expanded {
        for (child_label, child, index) in children {
            let mut child_path = path.clone();
            child_path.push(index);
            node = node.child(render_json_tree_node(
                child,
                expanded,
                child_path,
                child_label,
                depth + 1,
                theme,
                cx,
            ));
        }
    }
    node.into_any_element()
}

fn json_tree_selector(path: &[usize]) -> SharedString {
    if path.is_empty() {
        return "mqtt-message-viewer-json-node-root".into();
    }
    format!(
        "mqtt-message-viewer-json-node-{}",
        path.iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("-")
    )
    .into()
}

fn json_container_summary(value: &Value) -> String {
    match value {
        Value::Array(items) => format!("数组 · {} 项", items.len()),
        Value::Object(fields) => format!("对象 · {} 个字段", fields.len()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => String::new(),
    }
}

fn json_scalar_preview(value: &Value) -> String {
    truncate_json_label(&value.to_string())
}

fn truncate_json_label(value: &str) -> String {
    if value.len() <= MAX_JSON_LABEL_BYTES {
        return value.to_string();
    }
    let suffix = "…";
    let mut end = MAX_JSON_LABEL_BYTES.saturating_sub(suffix.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &value[..end], suffix)
}
