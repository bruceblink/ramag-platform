//! 数据同步全屏阻塞层。

mod result;

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _, button::ButtonVariants as _,
    h_flex, spinner::Spinner, v_flex,
};
use gpui_kit::{
    ClickEvent, Context, FocusHandle, IntoElement, KeyDownEvent, ParentElement, Render,
    SharedString, Styled, Window, div, prelude::*, px, relative,
};
use ramag_app::{DataSyncGate, DataSyncGatePhase, DataSyncGateSnapshot};
use ramag_domain::entities::{DataSyncStage, DataSyncTaskId, format_bytes};

use result::{format_count, format_elapsed_ms, render_result_summary};

pub struct DataSyncOverlay {
    gate: Arc<DataSyncGate>,
    cancel_confirmation: bool,
    focused_task: Option<DataSyncTaskId>,
    focus_handle: FocusHandle,
}

impl DataSyncOverlay {
    pub fn new(gate: Arc<DataSyncGate>, cx: &mut Context<Self>) -> Self {
        let gate_for_ticker = gate.clone();
        cx.spawn(async move |this, cx| {
            let mut previous = None;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                let snapshot = gate_for_ticker.snapshot();
                let signature = snapshot.as_ref().map(|snapshot| {
                    (
                        snapshot.task_id.clone(),
                        snapshot.phase,
                        snapshot.progress.clone(),
                    )
                });
                if signature == previous {
                    continue;
                }
                previous = signature;
                if this
                    .update(cx, |this, cx| {
                        if gate_for_ticker.snapshot().is_none() {
                            this.cancel_confirmation = false;
                            this.focused_task = None;
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            gate,
            cancel_confirmation: false,
            focused_task: None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn render_card(
        &mut self,
        snapshot: DataSyncGateSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let terminal = snapshot.phase.terminal();
        let cancelling = snapshot.phase == DataSyncGatePhase::Cancelling;
        let progress = &snapshot.progress;
        let title = match snapshot.phase {
            DataSyncGatePhase::Running => "正在同步数据",
            DataSyncGatePhase::Cancelling => "正在安全停止",
            DataSyncGatePhase::Completed => "数据同步完成",
            DataSyncGatePhase::Failed => "数据同步失败",
            DataSyncGatePhase::Cancelled => "数据同步已取消",
        };
        let stage = match progress.stage {
            DataSyncStage::Preparing => "准备",
            DataSyncStage::VerifyingTarget => "复核目标",
            DataSyncStage::CreatingStructure => "创建结构",
            DataSyncStage::Scanning => "扫描源数据",
            DataSyncStage::Writing => "写入缺失数据",
            DataSyncStage::Finalizing => "恢复结构与收尾",
            DataSyncStage::Cancelling => "等待安全停止点",
        };
        let theme = cx.theme();
        let (background, border, muted, foreground, success, warning, danger) = (
            theme.background,
            theme.border,
            theme.muted_foreground,
            theme.foreground,
            theme.success,
            theme.warning,
            theme.danger,
        );
        let context = format!(
            "{} / {}  →  {} / {}",
            snapshot.context.source_connection,
            snapshot.context.source_scope,
            snapshot.context.target_connection,
            snapshot.context.target_scope
        );
        let object_progress = progress.objects_total.map_or_else(
            || progress.objects_done.to_string(),
            |total| format!("{} / {total}", progress.objects_done),
        );
        let counts = format!(
            "对象 {object_progress}  ·  扫描 {}  ·  新增 {}  ·  跳过 {}  ·  失败 {}",
            format_count(progress.scanned),
            format_count(progress.inserted),
            format_count(progress.skipped),
            format_count(progress.failed)
        );
        let details = format!(
            "传输 {}  ·  警告 {}  ·  已用时 {}",
            format_bytes(progress.bytes),
            format_count(progress.warnings),
            format_elapsed_ms(progress.elapsed_ms)
        );
        let progress_percent = sync_progress_percent(progress.objects_done, progress.objects_total);
        let summary = snapshot.summary.clone();
        let summary_missing = terminal && summary.is_none();
        let objects_total = progress.objects_total;
        let task_for_cancel = snapshot.task_id.clone();
        let task_for_ack = snapshot.task_id.clone();
        let mut shade = gpui_kit::black();
        shade.a = 0.58;

        div()
            .id("data-sync-overlay")
            .debug_selector(|| "data-sync-overlay".into())
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(shade)
            .occlude()
            .track_focus(&self.focus_handle)
            .key_context("DataSyncBlocking")
            .on_key_down(
                cx.listener(|_this, _event: &KeyDownEvent, _window, cx| cx.stop_propagation()),
            )
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation()
            })
            .child(
                v_flex()
                    .id("data-sync-card")
                    .debug_selector(|| "data-sync-card".into())
                    .w_full()
                    .max_w(px(620.0))
                    .max_w_full()
                    .max_h_full()
                    .overflow_y_scroll()
                    .gap(px(14.0))
                    .p(px(24.0))
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(border)
                    .bg(background)
                    .text_color(foreground)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(10.0))
                            .when(!terminal, |row| row.child(Spinner::new().small()))
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child(title),
                            ),
                    )
                    .child(div().text_sm().text_color(muted).child(context))
                    .when(!terminal, |card| {
                        card.child(
                            v_flex()
                                .id("sync-running-progress")
                                .debug_selector(|| "sync-running-progress".into())
                                .gap(px(8.0))
                                .child(
                                    h_flex()
                                        .flex_wrap()
                                        .text_sm()
                                        .child(format!("阶段：{stage}"))
                                        .when(!progress.object.is_empty(), |line| {
                                            line.child(format!("  ·  当前：{}", progress.object))
                                        }),
                                )
                                .child(
                                    h_flex()
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .text_xs()
                                        .text_color(muted)
                                        .child("对象进度")
                                        .child(progress_percent.map_or_else(
                                            || "正在统计".to_string(),
                                            |percent| format!("{percent}%"),
                                        )),
                                )
                                .child(
                                    div()
                                        .id("sync-progress-track")
                                        .debug_selector(|| "sync-progress-track".into())
                                        .w_full()
                                        .h(px(6.0))
                                        .rounded(px(3.0))
                                        .bg(border.opacity(0.38))
                                        .when_some(progress_percent, |track, percent| {
                                            track.child(
                                                div()
                                                    .id("sync-progress-fill")
                                                    .debug_selector(|| "sync-progress-fill".into())
                                                    .h_full()
                                                    .rounded(px(3.0))
                                                    .bg(success)
                                                    .w(relative(percent as f32 / 100.0)),
                                            )
                                        }),
                                )
                                .child(div().text_sm().child(counts))
                                .child(div().text_sm().text_color(muted).child(details)),
                        )
                    })
                    .when_some(summary, |card, summary| {
                        card.child(render_result_summary(
                            &summary,
                            objects_total,
                            border,
                            muted,
                            foreground,
                            success,
                            warning,
                            danger,
                        ))
                    })
                    .when(summary_missing, |card| {
                        card.child(render_sync_feedback_notice(
                            "sync-summary-missing",
                            "sync-summary-missing-message",
                            "结果摘要不可用",
                            "同步已结束，但结果摘要不可用。",
                            danger,
                            muted,
                        ))
                    })
                    .when_some(snapshot.error.clone(), |card, error| {
                        card.child(render_sync_feedback_notice(
                            "sync-error-notice",
                            "sync-error-scroll",
                            "同步失败",
                            error,
                            danger,
                            muted,
                        ))
                    })
                    .when(self.cancel_confirmation && !terminal, |card| {
                        card.child(
                            v_flex()
                                .gap(px(8.0))
                                .p(px(10.0))
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(danger)
                                .child(
                                    div()
                                        .text_sm()
                                        .child("取消会等待当前批次或结构创建安全收尾。"),
                                )
                                .child(
                                    h_flex()
                                        .flex_wrap()
                                        .justify_end()
                                        .gap(px(8.0))
                                        .child(
                                            crate::clickable_button("sync-cancel-back")
                                                .ghost()
                                                .small()
                                                .label("继续同步")
                                                .on_click(cx.listener(
                                                    |this, _: &ClickEvent, _, cx| {
                                                        this.cancel_confirmation = false;
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .child(
                                            div()
                                                .debug_selector(|| "sync-cancel-confirm".into())
                                                .child(
                                                    crate::clickable_button("sync-cancel-confirm")
                                                        .danger()
                                                        .small()
                                                        .label("取消")
                                                        .on_click(cx.listener(
                                                            move |this, _: &ClickEvent, _, cx| {
                                                                this.gate.request_cancel_current(
                                                                    &task_for_cancel,
                                                                );
                                                                this.cancel_confirmation = false;
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
                                        ),
                                ),
                        )
                    })
                    .when(!terminal && !self.cancel_confirmation, |card| {
                        card.child(
                            h_flex().flex_wrap().justify_end().child(
                                div().debug_selector(|| "sync-cancel".into()).child(
                                    crate::clickable_button("sync-cancel")
                                        .danger()
                                        .small()
                                        .icon(IconName::Close)
                                        .label(if cancelling {
                                            "正在取消…"
                                        } else {
                                            "取消"
                                        })
                                        .disabled(cancelling)
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.cancel_confirmation = true;
                                            cx.notify();
                                        })),
                                ),
                            ),
                        )
                    })
                    .when(terminal, |card| {
                        card.child(
                            h_flex().flex_wrap().justify_end().child(
                                div().debug_selector(|| "sync-result-ack".into()).child(
                                    crate::clickable_button("sync-result-ack")
                                        .primary()
                                        .small()
                                        .label("关闭")
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _, cx| {
                                                this.gate.acknowledge_current(&task_for_ack);
                                                this.cancel_confirmation = false;
                                                cx.notify();
                                            },
                                        )),
                                ),
                            ),
                        )
                    }),
            )
    }
}

/// 将同步对象计数转换为有界百分比；未知或零总量不显示虚假进度。
fn sync_progress_percent(objects_done: u64, objects_total: Option<u64>) -> Option<u8> {
    let total = objects_total.filter(|total| *total > 0)?;
    let completed = objects_done.min(total);
    Some(((u128::from(completed) * 100) / u128::from(total)) as u8)
}

/// 终态同步反馈使用标题、图标和可滚动原因，避免长错误挤压确认操作。
fn render_sync_feedback_notice(
    id: &'static str,
    message_id: &'static str,
    title: &'static str,
    message: impl Into<SharedString>,
    danger: gpui_kit::Hsla,
    muted: gpui_kit::Hsla,
) -> impl IntoElement {
    let message = message.into();
    let mut background = danger;
    background.a = 0.10;
    h_flex()
        .id(id)
        .debug_selector(move || id.into())
        .w_full()
        .min_w_0()
        .items_start()
        .gap(px(8.0))
        .p(px(10.0))
        .border_1()
        .border_color(danger.opacity(0.28))
        .rounded(px(6.0))
        .bg(background)
        .child(Icon::new(IconName::CircleX).small().text_color(danger))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(danger)
                        .child(title),
                )
                .child(
                    div()
                        .id(message_id)
                        .debug_selector(move || message_id.into())
                        .max_h(px(160.0))
                        .overflow_y_scroll()
                        .text_xs()
                        .text_color(muted)
                        .child(message),
                ),
        )
}

impl Render for DataSyncOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.gate.snapshot();
        let active = snapshot.is_some();
        if let Some(snapshot) = snapshot.as_ref()
            && self.focused_task.as_ref() != Some(&snapshot.task_id)
        {
            self.focused_task = Some(snapshot.task_id.clone());
            window.focus(&self.focus_handle, cx);
        }
        div()
            .when(active, |root| root.absolute().inset_0())
            .when_some(snapshot, |root, snapshot| {
                root.child(self.render_card(snapshot, cx))
            })
    }
}

#[cfg(test)]
#[path = "data_sync_overlay_tests.rs"]
mod tests;
