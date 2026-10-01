//! Process filtering and presentation; force-quit remains explicit and identity-bound.

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, button::ButtonVariants as _,
    h_flex, input::Input, scroll::ScrollableElement as _, v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};
use ramag_infra_system::ProcessRow;
use ramag_ui::PointerDropdownMenu as _;

mod cells;
mod details;
use super::super::{SystemSection, SystemView, helpers};
use crate::{MAX_VISIBLE_PROCESSES, MonitorSnapshot, ProcessSort, ProcessSortDirection};
use cells::{cell, col, metric_cell, metric_columns, metric_display};

// Reserve readable identity/metric tracks and the action column; medium windows scroll them.
const DESKTOP_WIDTH: f32 = 878.0;
const DESKTOP_COLUMNS: [(ProcessSort, &str, f32); 7] = [
    (ProcessSort::Pid, "PID", 56.0),
    (ProcessSort::Name, "名称", 0.0),
    (ProcessSort::User, "用户", 96.0),
    (ProcessSort::Cpu, "CPU", 74.0),
    (ProcessSort::Memory, "内存", 90.0),
    (ProcessSort::Read, "读取", 100.0),
    (ProcessSort::Write, "写入", 100.0),
];
const ACTION_WIDTH: f32 = 84.0;

impl SystemView {
    /// Filters and sorts the snapshot once, then bounds rendered rows after ordering.
    pub(super) fn render_processes(
        &mut self,
        snapshot: &MonitorSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let query = self.process_search.read(cx).value().to_lowercase();
        let sort = self.monitor.process_sort();
        let direction = self.monitor.process_sort_direction();
        let rows = snapshot.sorted_processes(&query, sort, direction);
        let compact = f32::from(window.viewport_size().width) < 560.0;
        let theme = cx.theme();
        let controls = self.process_controls(compact, sort, direction, cx);
        let table = if compact {
            self.compact_process_table(&rows, snapshot, &query, cx)
        } else {
            self.desktop_process_table(&rows, snapshot, &query, cx)
        };
        let mut table = v_flex().w_full().min_w_0().child(table);
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
            "{} 个匹配进程 · 按 {}{}排序",
            rows.len(),
            sort.label(),
            if direction == ProcessSortDirection::Ascending {
                "升序"
            } else {
                "降序"
            }
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
            .child(controls);
        if self.selected_process.is_some() {
            page = page.child(self.render_process_details(snapshot, window, cx));
        }
        page = page.child(table);
        if snapshot.collection_stale {
            page = page.child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Stale,
                "采集器没有返回新快照；进程列表保留最近结果，指标不再标记为当前。",
                cx,
            ));
        }
        page.into_any_element()
    }

    /// Keeps search and both sort controls available at every viewport size.
    fn process_controls(
        &self,
        compact: bool,
        sort: ProcessSort,
        direction: ProcessSortDirection,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().clone();
        let mut controls = h_flex()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap(px(8.0));
        controls = controls.child(
            div()
                .debug_selector(|| "system-process-search".into())
                .child(Input::new(&self.process_search).small().w(px(if compact {
                    210.0
                } else {
                    300.0
                }))),
        );

        let sort_menu = ramag_ui::clickable_button("system-process-sort-menu")
            .debug_selector(|| "system-process-sort-menu".into())
            .xsmall()
            .outline()
            .label(format!("按 {} 排序", sort.label()))
            .dropdown_caret(true)
            .tooltip("选择进程排序字段")
            .pointer_dropdown_menu(move |mut menu, _, _| {
                for option in ProcessSort::ALL {
                    let target = view.clone();
                    menu = menu.item(
                        ramag_ui::menu_item(option.label())
                            .checked(option == sort)
                            .on_click(move |_, _, cx| {
                                target.update(cx, |this, cx| {
                                    this.select_process_sort(option, cx);
                                });
                            }),
                    );
                }
                menu
            });
        controls = controls.child(sort_menu);

        let direction_label = if direction == ProcessSortDirection::Ascending {
            "升序"
        } else {
            "降序"
        };
        let direction_button = ramag_ui::clickable_button("system-process-sort-direction")
            .debug_selector(|| "system-process-sort-direction".into())
            .xsmall()
            .ghost()
            .icon(Icon::new(if direction == ProcessSortDirection::Ascending {
                IconName::ArrowUp
            } else {
                IconName::ArrowDown
            }))
            .tooltip(format!("当前{direction_label}；点击反转"))
            .on_click(cx.listener(move |this, _, _, cx| this.select_process_sort(sort, cx)));
        controls.child(direction_button)
    }

    /// Places the fixed desktop columns in a horizontally scrollable viewport.
    fn desktop_process_table(
        &self,
        rows: &[&ProcessRow],
        snapshot: &MonitorSnapshot,
        query: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let viewport = div()
            .id("system-process-table-viewport")
            .debug_selector(|| "system-process-table-viewport".into())
            .w_full()
            .min_w_0()
            .overflow_x_scroll()
            .track_scroll(&self.process_table_scroll)
            .horizontal_scrollbar(&self.process_table_scroll)
            .child(self.desktop_table_contents(rows, snapshot, query, cx));
        viewport.into_any_element()
    }

    /// Uses matching tracks for sortable headers and the bounded set of cached process rows.
    fn desktop_table_contents(
        &self,
        rows: &[&ProcessRow],
        snapshot: &MonitorSnapshot,
        query: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let sort = self.monitor.process_sort();
        let direction = self.monitor.process_sort_direction();
        let mut table = v_flex()
            .debug_selector(|| "system-process-table".into())
            .w_full()
            .min_w(px(DESKTOP_WIDTH))
            .gap(px(0.0));
        let mut header = h_flex()
            .w_full()
            .min_h(px(34.0))
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .border_b_1()
            .border_color(theme.border);
        for (column, label, width) in DESKTOP_COLUMNS {
            let mut heading =
                ramag_ui::clickable_button(format!("system-process-sort-{}", column.stable_id()))
                    .debug_selector(move || format!("system-process-sort-{}", column.stable_id()))
                    .xsmall()
                    .ghost()
                    .label(label)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.select_process_sort(column, cx)),
                    );
            if sort == column {
                heading = heading
                    .icon(Icon::new(if direction == ProcessSortDirection::Ascending {
                        IconName::ArrowUp
                    } else {
                        IconName::ArrowDown
                    }))
                    .tooltip(if direction == ProcessSortDirection::Ascending {
                        "当前按此列升序排序"
                    } else {
                        "当前按此列降序排序"
                    });
            }
            let heading = if column == ProcessSort::Name {
                heading.w_full().min_w(px(120.0)).justify_start()
            } else {
                heading.w(px(width)).flex_none()
            };
            let header_cell = div()
                .id(format!("system-process-header-{}", column.stable_id()))
                .debug_selector(move || format!("system-process-header-{}", column.stable_id()))
                .child(heading);
            header = header.child(if column == ProcessSort::Name {
                header_cell.flex_1().min_w(px(120.0))
            } else {
                header_cell.w(px(width)).flex_none()
            });
        }
        header = header.child(col("操作", ACTION_WIDTH, theme));
        table = table.child(header);

        if rows.is_empty() {
            return table.child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                if query.is_empty() {
                    "当前没有进程数据"
                } else {
                    "没有匹配的进程"
                },
                cx,
            ));
        }
        for process in rows.iter().take(MAX_VISIBLE_PROCESSES) {
            table = table.child(self.desktop_row(process, snapshot, cx));
        }
        table
    }

    /// Shows each cached physical value in its own track; clipped text and reasons remain in tooltips.
    fn desktop_row(
        &self,
        process: &ProcessRow,
        snapshot: &MonitorSnapshot,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let pid = process.identity.pid;
        let identity = process.identity.clone();
        let name = process.name.clone();
        let theme = cx.theme();
        let mut row = h_flex()
            .id(format!(
                "system-process-select-{pid}-{}",
                identity.start_time_ticks
            ))
            .debug_selector(move || format!("system-process-row-{pid}"))
            .w_full()
            .min_h(px(38.0))
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .border_b_1()
            .border_color(theme.border.opacity(0.55))
            .bg(
                if self
                    .selected_process
                    .as_ref()
                    .is_some_and(|selected| selected.identity == identity)
                {
                    theme.primary.opacity(0.10)
                } else {
                    theme.background
                },
            )
            .cursor_pointer()
            .hover(|style| style.bg(theme.accent))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_process(identity.clone(), name.clone(), cx);
            }));
        row = row.child(cell(pid, "pid", pid.to_string(), 56.0, None, theme));
        row = row.child(
            cell(pid, "name", process.name.clone(), 0.0, None, theme)
                .flex_1()
                .min_w(px(120.0)),
        );
        let user = process.user.clone().unwrap_or_else(|| "不可用".into());
        row = row.child(cell(
            pid,
            "user",
            user,
            96.0,
            process.user_reason.clone(),
            theme,
        ));
        for (sort, reading, unit, width) in metric_columns(process) {
            let (value, reason) = metric_display(snapshot, reading, unit);
            row = row.child(cell(pid, sort.stable_id(), value, width, reason, theme).text_xs());
        }
        row.child(
            div()
                .w(px(ACTION_WIDTH))
                .flex_none()
                .child(self.force_quit_button(process, false, cx)),
        )
    }

    /// Uses a two-line identity block and a two-by-two physical metric grid below 560px.
    fn compact_process_table(
        &self,
        rows: &[&ProcessRow],
        snapshot: &MonitorSnapshot,
        query: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let mut table = v_flex()
            .debug_selector(|| "system-process-table".into())
            .w_full()
            .min_w_0()
            .gap(px(0.0));
        if rows.is_empty() {
            return table
                .child(ramag_ui::pulse_ui::pulse_status_notice(
                    ramag_ui::pulse_ui::PulseStatus::Unavailable,
                    if query.is_empty() {
                        "当前没有进程数据"
                    } else {
                        "没有匹配的进程"
                    },
                    cx,
                ))
                .into_any_element();
        }
        for process in rows.iter().take(MAX_VISIBLE_PROCESSES) {
            let pid = process.identity.pid;
            let selected_identity = process.identity.clone();
            let selected_name = process.name.clone();
            let identity = h_flex()
                .w_full()
                .min_w_0()
                .items_start()
                .gap(px(8.0))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(cell(pid, "name", process.name.clone(), 0.0, None, theme).w_full())
                        .child(
                            h_flex()
                                .min_w_0()
                                .gap(px(8.0))
                                .child(
                                    cell(pid, "pid", format!("PID {pid}"), 0.0, None, theme)
                                        .flex_none(),
                                )
                                .child(
                                    cell(
                                        pid,
                                        "user",
                                        process.user.clone().unwrap_or_else(|| "用户不可用".into()),
                                        0.0,
                                        process.user_reason.clone(),
                                        theme,
                                    )
                                    .flex_1(),
                                ),
                        ),
                )
                .child(
                    div()
                        .w(px(32.0))
                        .flex_none()
                        .child(self.force_quit_button(process, true, cx)),
                );
            let mut metrics = gpui_kit::component::v_flex().w_full().gap(px(4.0));
            for pair in metric_columns(process).chunks(2) {
                let mut metric_row = h_flex().w_full().min_w_0().gap(px(6.0));
                for (sort, reading, unit, _) in pair {
                    let (value, reason) = metric_display(snapshot, reading, unit);
                    metric_row = metric_row.child(metric_cell(pid, *sort, value, reason, theme));
                }
                metrics = metrics.child(metric_row);
            }
            table = table.child(
                v_flex()
                    .id(format!(
                        "system-process-select-{pid}-{}",
                        selected_identity.start_time_ticks
                    ))
                    .debug_selector(move || format!("system-process-row-{pid}"))
                    .w_full()
                    .min_w_0()
                    .gap(px(6.0))
                    .py(px(8.0))
                    .border_b_1()
                    .border_color(theme.border.opacity(0.55))
                    .bg(
                        if self
                            .selected_process
                            .as_ref()
                            .is_some_and(|selected| selected.identity == selected_identity)
                        {
                            theme.primary.opacity(0.10)
                        } else {
                            theme.background
                        },
                    )
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.accent))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_process(selected_identity.clone(), selected_name.clone(), cx);
                    }))
                    .child(identity)
                    .child(metrics),
            );
        }
        table.into_any_element()
    }

    /// Captures the same verified identity and name used by the existing confirmation path.
    fn force_quit_button(
        &self,
        process: &ProcessRow,
        compact: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let pid = process.identity.pid;
        let identity = process.identity.clone();
        let name = process.name.clone();
        let unsafe_identity = pid <= 1 || identity.start_time_ticks == 0;
        let self_process = pid == std::process::id();
        let disabled = unsafe_identity || self_process || self.termination_in_progress;
        let button = ramag_ui::clickable_button(format!("system-kill-{pid}"))
            .debug_selector(move || format!("system-kill-{pid}"))
            .xsmall()
            .danger()
            .disabled(disabled)
            .tooltip(if self_process {
                "不能强制退出当前 Ramag 进程"
            } else if unsafe_identity {
                "无法验证此进程身份，不能强制退出"
            } else if self.termination_in_progress {
                "正在处理强制退出请求"
            } else {
                "强制退出此进程，可能丢失未保存数据"
            });
        let button = if compact {
            button.icon(Icon::new(IconName::CircleX))
        } else {
            button.label("强制退出")
        };
        if disabled {
            button
        } else {
            button.on_click(cx.listener(move |this, _, _, cx| {
                this.prepare_termination(identity.clone(), name.clone(), cx);
                cx.stop_propagation();
            }))
        }
    }
}
