//! Process filtering and presentation; force-quit remains explicit and identity-bound.

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, button::ButtonVariants as _, h_flex,
    input::Input, v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, Window, div,
    px,
};
use ramag_infra_system::ProcessRow;

use super::super::{SystemSection, SystemView, helpers};
use crate::{MAX_VISIBLE_PROCESSES, MonitorSnapshot, ProcessSort};

impl SystemView {
    pub(super) fn render_processes(
        &mut self,
        snapshot: &MonitorSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let query = self.process_search.read(cx).value().to_lowercase();
        let mut rows = snapshot
            .host
            .processes
            .iter()
            .filter(|row| matches_process(row, &query))
            .collect::<Vec<_>>();
        let sort = self.monitor.process_sort();
        rows.sort_by(|left, right| {
            let (left_value, right_value) = match sort {
                ProcessSort::Cpu => (
                    snapshot.process_value(&left.cpu_percent),
                    snapshot.process_value(&right.cpu_percent),
                ),
                ProcessSort::Memory => (
                    snapshot.process_value(&left.memory_bytes),
                    snapshot.process_value(&right.memory_bytes),
                ),
            };
            compare_optional_desc(left_value, right_value)
        });

        let compact = f32::from(window.viewport_size().width) < 560.0;
        let mut controls = h_flex()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(8.0));
        controls = controls.child(Input::new(&self.process_search).small().w(px(if compact {
            210.0
        } else {
            300.0
        })));
        for sort in [ProcessSort::Cpu, ProcessSort::Memory] {
            let selected = sort == self.monitor.process_sort();
            let button =
                ramag_ui::clickable_button(format!("system-process-sort-{}", sort.label()))
                    .xsmall()
                    .label(format!("按 {} 排序", sort.label()));
            controls = controls.child(
                if selected {
                    button.primary()
                } else {
                    button.ghost()
                }
                .on_click(cx.listener(move |this, _, _, cx| this.select_process_sort(sort, cx))),
            );
        }

        let theme = cx.theme();
        let mut table = v_flex()
            .debug_selector(|| "system-process-table".into())
            .w_full()
            .min_w_0()
            .gap(px(0.0));
        let (pid_width, cpu_width, memory_width, action_width, gap, padding) = if compact {
            (42.0, 48.0, 62.0, 44.0, 4.0, 6.0)
        } else {
            (56.0, 62.0, 88.0, 56.0, 8.0, 9.0)
        };
        table = table.child(
            h_flex()
                .w_full()
                .min_h(px(34.0))
                .items_center()
                .gap(px(gap))
                .px(px(padding))
                .border_b_1()
                .border_color(theme.border)
                .child(col("PID", pid_width, theme))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("进程 / 用户"),
                )
                .child(col("CPU", cpu_width, theme))
                .child(col("内存", memory_width, theme))
                .child(col("操作", action_width, theme)),
        );
        if rows.is_empty() {
            table = table.child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                if query.is_empty() {
                    "当前没有进程数据"
                } else {
                    "没有匹配的进程"
                },
                cx,
            ));
        }
        for row in rows.iter().take(MAX_VISIBLE_PROCESSES) {
            table = table.child(self.process_row(row, snapshot, compact, cx));
        }
        if rows.len() > MAX_VISIBLE_PROCESSES {
            table = table.child(
                div()
                    .px(px(10.0))
                    .py(px(7.0))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!(
                        "结果过多，仅显示前 {MAX_VISIBLE_PROCESSES} 行；请继续缩小搜索范围。"
                    )),
            );
        }
        let description = format!(
            "{} 个匹配进程 · 按 {:?} 排序 · 结束操作会再次核对进程身份",
            rows.len(),
            sort
        );
        let mut page = v_flex()
            .debug_selector(|| "system-page-processes".into())
            .w_full()
            .min_w_0()
            .gap(px(12.0))
            .p(px(if compact { 12.0 } else { 20.0 }))
            .child(helpers::page_title(
                SystemSection::Processes,
                "搜索、比较和管理当前进程",
                cx,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(description),
            )
            .child(controls)
            .child(
                ramag_ui::pulse_ui::pulse_panel(cx)
                    .p(px(0.0))
                    .overflow_hidden()
                    .child(table),
            );
        if snapshot.collection_stale {
            page = page.child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Stale,
                "采集器没有返回新快照；进程列表保留最近结果，指标不再标记为当前。",
                cx,
            ));
        }
        page.into_any_element()
    }

    fn process_row(
        &self,
        process: &ProcessRow,
        snapshot: &MonitorSnapshot,
        compact: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let pid = process.identity.pid;
        let identity = process.identity.clone();
        let name = process.name.clone();
        let unsafe_identity = pid <= 1 || identity.start_time_ticks == 0;
        let self_process = pid == std::process::id();
        let disabled = unsafe_identity || self_process || self.termination_in_progress;
        let button = ramag_ui::clickable_button(format!("system-kill-{pid}"))
            .debug_selector(|| format!("system-kill-{pid}"))
            .xsmall()
            .label("结束")
            .danger()
            .disabled(disabled)
            .tooltip(if self_process {
                "不能结束当前 Ramag 进程"
            } else {
                "强制结束此进程"
            });
        let button = if disabled {
            button
        } else {
            button.on_click(cx.listener(move |this, _, _, cx| {
                this.prepare_termination(identity.clone(), name.clone(), cx);
            }))
        };
        let theme = cx.theme();
        let process_label = process.user.as_ref().map_or_else(
            || process.name.clone(),
            |user| format!("{} · {}", process.name, user),
        );
        let (pid_width, cpu_width, memory_width, action_width, gap, padding) = if compact {
            (42.0, 48.0, 62.0, 44.0, 4.0, 6.0)
        } else {
            (56.0, 62.0, 88.0, 56.0, 8.0, 9.0)
        };
        let row = h_flex()
            .w_full()
            .min_h(px(38.0))
            .items_center()
            .gap(px(gap))
            .px(px(padding))
            .border_b_1()
            .border_color(theme.border.opacity(0.55))
            .child(col(&pid.to_string(), pid_width, theme))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .child(process_label),
            )
            .child(col(
                &display(
                    snapshot.process_value(&process.cpu_percent),
                    &ramag_infra_system::Unit::Percent,
                ),
                cpu_width,
                theme,
            ))
            .child(col(
                &display(
                    snapshot.process_value(&process.memory_bytes),
                    &ramag_infra_system::Unit::Bytes,
                ),
                memory_width,
                theme,
            ))
            .child(div().w(px(action_width)).child(button));
        row.into_any_element()
    }
}

fn matches_process(process: &ProcessRow, query: &str) -> bool {
    query.is_empty()
        || process.name.to_lowercase().contains(query)
        || process.identity.pid.to_string().contains(query)
        || process
            .user
            .as_deref()
            .is_some_and(|user| user.to_lowercase().contains(query))
}

fn compare_optional_desc(left: Option<f64>, right: Option<f64>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.total_cmp(&left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn display(value: Option<f64>, unit: &ramag_infra_system::Unit) -> String {
    value
        .map(|value| helpers::format_value(value, unit))
        .unwrap_or_else(|| "—".into())
}

fn col(value: &str, width: f32, theme: &gpui_kit::component::theme::Theme) -> impl IntoElement {
    div()
        .w(px(width))
        .min_w_0()
        .overflow_hidden()
        .text_ellipsis()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(value.to_owned())
}
