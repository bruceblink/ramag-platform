//! Identity-bound process details, resolved afresh from the current monitor snapshot.

use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use ramag_infra_system::Unit;

use super::super::super::SystemView;
use super::super::super::process_selection::{SelectedProcess, SelectedProcessState};
use super::cells::metric_display;
use crate::MonitorSnapshot;

const DETAIL_ROW_GAP: f32 = 8.0;

impl SystemView {
    /// Resolves the captured identity against all current rows and renders only live metrics.
    /// The selected value owns identity and name only; every metric and field reason is borrowed
    /// from this snapshot. Closing the fixed header control clears selection without an OS action.
    pub(super) fn render_process_details(
        &self,
        snapshot: &MonitorSnapshot,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(selected) = self.selected_process.as_ref() else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let viewport_height = f32::from(window.viewport_size().height);
        let max_body_height = (viewport_height * 0.4).clamp(0.0, 260.0);
        let close = ramag_ui::clickable_button("system-process-details-close")
            .debug_selector(|| "system-process-details-close".into())
            .xsmall()
            .ghost()
            .icon(Icon::new(IconName::Close))
            .tooltip("关闭进程详情")
            .on_click(cx.listener(|this, _, _, cx| this.close_process_details(cx)));
        let header = h_flex()
            .id("system-process-details-header")
            .debug_selector(|| "system-process-details-header".into())
            .w_full()
            .min_w_0()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child("进程详情"),
            )
            .child(close);

        let body = match selected.resolve(snapshot) {
            SelectedProcessState::Current(process) => {
                let mut body = v_flex()
                    .id("system-process-details-body")
                    .debug_selector(|| "system-process-details-body".into())
                    .w_full()
                    .min_w_0()
                    .max_h(px(max_body_height))
                    .gap(px(DETAIL_ROW_GAP))
                    .overflow_y_scroll()
                    .track_scroll(&self.process_detail_scroll);
                if snapshot.collection_stale {
                    body = body.child(
                        div()
                            .id("system-process-detail-stale")
                            .debug_selector(|| "system-process-detail-stale".into())
                            .w_full()
                            .min_w_0()
                            .whitespace_normal()
                            .text_xs()
                            .text_color(theme.warning)
                            .child("采集快照已过期；下方指标已标记为过期，不代表当前读数。"),
                    );
                }
                body = body
                    .child(detail_field("name", "名称", &process.name, None, cx))
                    .child(detail_field(
                        "pid",
                        "PID",
                        &process.identity.pid.to_string(),
                        None,
                        cx,
                    ));
                let user = process.user.as_deref().unwrap_or("不可用");
                body = body.child(detail_field(
                    "user",
                    "用户",
                    user,
                    process.user_reason.as_deref(),
                    cx,
                ));
                body = body
                    .child(metric_field(
                        snapshot,
                        "cpu",
                        "CPU",
                        &process.cpu_percent,
                        &Unit::Percent,
                        cx,
                    ))
                    .child(metric_field(
                        snapshot,
                        "memory",
                        "内存",
                        &process.memory_bytes,
                        &Unit::Bytes,
                        cx,
                    ))
                    .child(metric_field(
                        snapshot,
                        "read",
                        "读取速率",
                        &process.read_bytes_per_second,
                        &Unit::BytesPerSecond,
                        cx,
                    ))
                    .child(metric_field(
                        snapshot,
                        "write",
                        "写入速率",
                        &process.write_bytes_per_second,
                        &Unit::BytesPerSecond,
                        cx,
                    ));
                let (mut thread_value, thread_reason) =
                    metric_display(snapshot, &process.threads, &Unit::Count);
                if let Some(value) = snapshot.process_value(&process.threads) {
                    thread_value = format!("{value:.0} 个线程");
                } else {
                    thread_value = thread_value.replace("count", "个线程");
                }
                body = body.child(detail_field(
                    "threads",
                    "线程数",
                    &thread_value,
                    thread_reason.as_deref(),
                    cx,
                ));
                let sample_at = process_capture_time(snapshot);
                body.child(detail_field(
                    "sample-time",
                    "采样时间（采集器单调时钟）",
                    &sample_at,
                    None,
                    cx,
                ))
                .into_any_element()
            }
            SelectedProcessState::Missing => self
                .identity_status_body(
                    "missing",
                    "本次快照未包含此进程，可能已退出或无法读取",
                    selected,
                    max_body_height,
                    cx,
                )
                .into_any_element(),
            SelectedProcessState::Reused => self
                .identity_status_body(
                    "reused",
                    "此 PID 已被复用；详情仍绑定原进程，未显示新进程指标",
                    selected,
                    max_body_height,
                    cx,
                )
                .into_any_element(),
        };

        v_flex()
            .id("system-process-details")
            .debug_selector(|| "system-process-details".into())
            .w_full()
            .min_w_0()
            .gap(px(6.0))
            .pb(px(8.0))
            .border_b_1()
            .border_color(theme.border)
            .child(header)
            .child(body)
            .into_any_element()
    }

    /// Builds a bounded scroll body for missing and reused identities without carrying old metrics.
    fn identity_status_body(
        &self,
        kind: &'static str,
        message: &'static str,
        selected: &SelectedProcess,
        max_height: f32,
        cx: &Context<Self>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let selector = format!("system-process-detail-{kind}");
        v_flex()
            .id("system-process-details-body")
            .debug_selector(|| "system-process-details-body".into())
            .w_full()
            .min_w_0()
            .max_h(px(max_height))
            .gap(px(DETAIL_ROW_GAP))
            .overflow_y_scroll()
            .track_scroll(&self.process_detail_scroll)
            .child(
                v_flex()
                    .id(selector.clone())
                    .debug_selector(move || selector.clone())
                    .w_full()
                    .min_w_0()
                    .gap(px(4.0))
                    .child(detail_text("name", "名称", &selected.name, cx))
                    .child(detail_text(
                        "pid",
                        "PID",
                        &selected.identity.pid.to_string(),
                        cx,
                    ))
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .whitespace_normal()
                            .text_sm()
                            .text_color(cx.theme().warning)
                            .child(soft_wrap(message)),
                    ),
            )
    }
}

/// Renders one live metric with its physical state and failure reason visible in the panel.
fn metric_field(
    snapshot: &MonitorSnapshot,
    key: &'static str,
    label: &'static str,
    reading: &ramag_infra_system::Reading,
    unit: &Unit,
    cx: &Context<SystemView>,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (value, reason) = metric_display(snapshot, reading, unit);
    detail_field(key, label, &value, reason.as_deref(), cx)
}

/// Keeps complete text inspectable and exposes a stable reason selector when a read failed.
fn detail_field(
    key: &'static str,
    label: &'static str,
    value: &str,
    reason: Option<&str>,
    cx: &Context<SystemView>,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let theme = cx.theme();
    let selector = format!("system-process-detail-{key}");
    let mut field = v_flex()
        .id(selector.clone())
        .debug_selector(move || selector.clone())
        .w_full()
        .min_w_0()
        .gap(px(2.0))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .items_start()
                .gap(px(8.0))
                .child(
                    div()
                        .w(px(100.0))
                        .flex_none()
                        .whitespace_normal()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .whitespace_normal()
                        .text_sm()
                        .text_color(theme.foreground)
                        .child(soft_wrap(value)),
                ),
        );
    if let Some(reason) = reason.filter(|reason| !reason.is_empty()) {
        let reason_selector = format!("system-process-detail-reason-{key}");
        field = field.child(
            div()
                .id(reason_selector.clone())
                .debug_selector(move || reason_selector.clone())
                .w_full()
                .min_w_0()
                .whitespace_normal()
                .text_xs()
                .text_color(theme.warning)
                .child(soft_wrap(reason)),
        );
    }
    field
}

/// Provides a line break opportunity inside otherwise unbroken names and backend error strings.
/// This changes only the rendered copy; the captured process name and diagnostic remain intact.
fn soft_wrap(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut run = 0usize;
    for character in value.chars() {
        result.push(character);
        if character.is_whitespace() || character == '\u{200b}' {
            run = 0;
        } else {
            run += 1;
            if run == 8 {
                result.push('\u{200b}');
                run = 0;
            }
        }
    }
    result
}

/// Formats collector-relative monotonic time without implying wall-clock time.
fn process_capture_time(snapshot: &MonitorSnapshot) -> String {
    let seconds = snapshot.host.capture_finished_ns as f64 / 1_000_000_000.0;
    format!("{seconds:.3} 秒 · 采样序号 {}", snapshot.host.sequence)
}

fn detail_text(
    key: &'static str,
    label: &'static str,
    value: &str,
    cx: &Context<SystemView>,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let theme = cx.theme();
    let selector = format!("system-process-detail-{key}");
    h_flex()
        .id(selector.clone())
        .debug_selector(move || selector.clone())
        .w_full()
        .min_w_0()
        .items_start()
        .gap(px(8.0))
        .child(
            div()
                .w(px(100.0))
                .flex_none()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_normal()
                .text_sm()
                .text_color(theme.foreground)
                .child(soft_wrap(value)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Display wrapping must retain every source character and must never change captured identity text.
    #[test]
    fn long_ascii_and_chinese_display_text_preserves_original_content() {
        let source = "long-running-worker-数据库后台任务.exe".repeat(8);
        let wrapped = soft_wrap(&source);
        assert!(wrapped.contains('\u{200b}'));
        assert_eq!(wrapped.replace('\u{200b}', ""), source);
        assert_eq!(soft_wrap("short"), "short");
    }

    #[test]
    fn capture_time_is_monotonic_source_time_and_sequence() {
        let mut snapshot = MonitorSnapshot::default();
        snapshot.host.capture_finished_ns = 1_250_000_000;
        snapshot.host.sequence = 12;
        assert_eq!(process_capture_time(&snapshot), "1.250 秒 · 采样序号 12");
    }
}
