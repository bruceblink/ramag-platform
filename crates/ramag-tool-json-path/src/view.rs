//! JSON Path 的原生 GPUI 输入、执行和结果反馈。

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{Editor, EditorState, Input, InputState},
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Window, div, prelude::*, px,
};
use ramag_app::{
    MAX_PLUGIN_TASK_RESULT_BYTES, PluginTaskBudget, PluginTaskRunError, StaticPluginHost,
};
use ramag_ui::clickable_button;

use crate::{ENTRY_ID, JsonPathRequest};

const DEFAULT_JSON: &str = r#"{
  "users": [
    { "name": "Alice", "active": true },
    { "name": "Bob", "active": false }
  ],
  "meta": { "request-id": "abc-123" }
}"#;
const INPUT_BYTES: usize = 4 * 1024 * 1024;

/// JSON Path 原生工具面板；输入状态由 GPUI 管理，执行由插件宿主调度。
pub struct JsonPathView {
    host: Arc<StaticPluginHost>,
    raw_json: Entity<EditorState>,
    path: Entity<InputState>,
    output: String,
    error: Option<String>,
    running: bool,
}

impl JsonPathView {
    pub(crate) fn new(
        host: Arc<StaticPluginHost>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let raw_json = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .placeholder("输入 JSON 或 JSON5 文本")
                .default_value(DEFAULT_JSON)
        });
        let path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("例如 $.users[*].name")
                .default_value("$.users[*].name")
                .validate(|value, _| value.len() <= 1024)
        });
        Self {
            host,
            raw_json,
            path,
            output: String::new(),
            error: None,
            running: false,
        }
    }

    /// 读取当前编辑器值并通过宿主执行器提交一次有界入口任务。
    fn execute(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        let raw_json = self.raw_json.read(cx).value();
        if raw_json.len() > INPUT_BYTES {
            self.error = Some(format!("输入超过 {INPUT_BYTES} 字节"));
            self.output.clear();
            cx.notify();
            return;
        }
        let request = JsonPathRequest {
            raw_json: raw_json.to_string(),
            path: self.path.read(cx).value().trim().to_owned(),
        };
        let input = match serde_json::to_vec(&request) {
            Ok(input) => input,
            Err(error) => {
                self.error = Some(format!("构造 JSON Path 请求失败：{error}"));
                self.output.clear();
                cx.notify();
                return;
            }
        };
        let budget =
            match PluginTaskBudget::new(Duration::from_secs(10), MAX_PLUGIN_TASK_RESULT_BYTES) {
                Ok(budget) => budget,
                Err(error) => {
                    self.error = Some(error.to_string());
                    self.output.clear();
                    cx.notify();
                    return;
                }
            };
        let execution = match self
            .host
            .execute_entry(crate::PLUGIN_ID, ENTRY_ID, input, budget)
        {
            Ok(execution) => execution,
            Err(error) => {
                self.error = Some(error.to_string());
                self.output.clear();
                cx.notify();
                return;
            }
        };
        self.running = true;
        self.error = None;
        self.output.clear();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = execution.join().await;
            let _ = this.update(cx, |view, cx| {
                view.running = false;
                match result {
                    Ok(output) => match String::from_utf8(output) {
                        Ok(output) => view.output = output,
                        Err(_) => view.error = Some("入口返回了无效 UTF-8 文本".into()),
                    },
                    Err(error) => view.error = Some(format_task_error(error)),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn format_task_error(error: PluginTaskRunError) -> String {
    error.to_string()
}

impl Render for JsonPathView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (status, status_kind) = if self.running {
            ("正在执行…", ramag_ui::pulse_ui::PulseStatus::Warming)
        } else if self.error.is_some() {
            ("执行失败", ramag_ui::pulse_ui::PulseStatus::Failed)
        } else if self.output.is_empty() {
            ("等待执行", ramag_ui::pulse_ui::PulseStatus::Unavailable)
        } else {
            ("执行完成", ramag_ui::pulse_ui::PulseStatus::Current)
        };
        let error = self.error.clone();
        v_flex()
            .id("json-path-view")
            .debug_selector(|| "json-path-view".into())
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_y_scrollbar()
            .p(px(24.0))
            .gap(px(14.0))
            .child(
                ramag_ui::responsive_toolbar()
                    .id("json-path-page-header")
                    .debug_selector(|| "json-path-page-header".into())
                    .items_center()
                    .child(
                        ramag_ui::pulse_ui::pulse_page_title(
                            "JSON Path 提取器",
                            Some("原生 GPUI 入口 · JSON5 输入 · 本机优先执行"),
                            cx,
                        )
                        .flex_1()
                        .min_w_0(),
                    )
                    .child(
                        ramag_ui::pulse_ui::pulse_status_badge_with_label(status_kind, status, cx)
                            .id("json-path-status")
                            .debug_selector(|| "json-path-status".into()),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().text_sm().child("JSON Path"))
                    .child(Input::new(&self.path).flex_1().min_w_0())
                    .child(
                        clickable_button("json-path-run")
                            .debug_selector(|| "json-path-run".into())
                            .primary()
                            .label(if self.running { "执行中" } else { "提取" })
                            .disabled(self.running)
                            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                view.execute(cx);
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(5.0))
                    .child(div().text_sm().child("输入 JSON / JSON5"))
                    .child(Editor::new(&self.raw_json).h(px(260.0))),
            )
            .child(
                v_flex()
                    .gap(px(5.0))
                    .child(
                        h_flex()
                            .items_center()
                            .child(div().text_sm().child("提取结果"))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(status),
                            ),
                    )
                    .child(
                        div()
                            .id("json-path-output")
                            .debug_selector(|| "json-path-output".into())
                            .w_full()
                            .min_h(px(180.0))
                            .p(px(12.0))
                            .bg(theme.secondary)
                            .border_1()
                            .border_color(theme.border)
                            .rounded(px(6.0))
                            .text_sm()
                            .child(if self.output.is_empty() {
                                "执行后显示结果".to_owned()
                            } else {
                                self.output.clone()
                            }),
                    ),
            )
            .when_some(error, |this, error| {
                this.child(
                    div()
                        .id("json-path-error")
                        .debug_selector(|| "json-path-error".into())
                        .text_sm()
                        .text_color(theme.danger)
                        .child(error),
                )
            })
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
