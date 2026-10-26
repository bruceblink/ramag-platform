//! Process table interaction, keeping canonical snapshots separate from the visible order.
use super::*;
use crate::processes::{self, ProcessSort};
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt, PopupMenuItem},
};
use ramag_infra_system::process_control::{self, ProcessSignal};
use std::rc::Rc;

#[path = "process_panel/details.rs"]
mod details;

#[derive(Default)]
pub(super) struct ProcessPanelState {
    input: Option<Entity<InputState>>,
    subscription: Option<Subscription>,
    query: String,
    sort: ProcessSort,
    confirmation: Option<(ProcessIdentity, String, ProcessSignal)>,
}

// The workspace retains this state while native windows and controls are recreated.
#[derive(Default)]
pub(crate) struct ProcessActionState {
    busy: bool,
    notice: String,
}

impl ProcessActionState {
    fn finish_action(
        &mut self,
        name: &str,
        pid: u32,
        result: Result<process_control::ProcessActionOutcome, String>,
    ) {
        self.busy = false;
        self.notice = match result {
            Ok(process_control::ProcessActionOutcome::SignalSent) => {
                format!("已向 {name}（PID {pid}）发送请求。正在等待进程列表更新。")
            }
            Ok(process_control::ProcessActionOutcome::ExitObserved) => {
                format!("{name}（PID {pid}）已退出。")
            }
            Ok(process_control::ProcessActionOutcome::CloseRequested) => format!(
                "已请求关闭 {name}（PID {pid}）。请查看该进程窗口的响应；目前尚未确认退出。"
            ),
            Ok(process_control::ProcessActionOutcome::TerminationPending) => format!(
                "已请求终止 {name}（PID {pid}），但进程仍在运行。再次操作前请先查看进程列表。"
            ),
            Err(error) => error,
        };
    }
}

const UNVERIFIED_PROCESS_NOTICE: &str = "进程源没有提供可验证的启动时间标识，因此此行只能查看。";

fn process_action_block_reason(identity: &ProcessIdentity) -> Option<&'static str> {
    (identity.start_time_ticks == 0).then_some(UNVERIFIED_PROCESS_NOTICE)
}

impl MonitorPanel {
    fn process_projection(&self) -> Vec<usize> {
        processes::project(
            &self.shared.borrow().processes,
            &self.process_state.query,
            self.process_state.sort,
        )
    }

    fn visible_selected_index(&self, rows: &[usize]) -> Option<usize> {
        let selected = self.selected_index()?;
        rows.iter().position(|&index| index == selected)
    }

    fn prepare_process_action(
        &mut self,
        identity: ProcessIdentity,
        signal: ProcessSignal,
        cx: &mut Context<Self>,
    ) {
        if self.shared.borrow().process_action.busy {
            return;
        }
        if let Some(reason) = process_action_block_reason(&identity) {
            self.process_state.confirmation = None;
            self.shared.borrow_mut().process_action.notice = reason.into();
            cx.notify();
            return;
        }
        let name = self
            .shared
            .borrow()
            .processes
            .iter()
            .find(|row| row.identity == identity)
            .and_then(|row| row.cells.get(1))
            .cloned();
        if let Some(name) = name {
            self.selected = Some(identity.clone());
            self.process_state.confirmation = Some((identity, name, signal));
            self.shared.borrow_mut().process_action.notice.clear();
        } else {
            self.process_state.confirmation = None;
            self.shared.borrow_mut().process_action.notice =
                "该进程已退出。请选择其他进程。".into();
        }
        cx.notify();
    }

    fn confirm_process_action(&mut self, cx: &mut Context<Self>) {
        if self.shared.borrow().process_action.busy {
            return;
        }
        let Some((identity, name, signal)) = self.process_state.confirmation.take() else {
            return;
        };
        if let Some(reason) = process_action_block_reason(&identity) {
            self.shared.borrow_mut().process_action.notice = reason.into();
            cx.notify();
            return;
        }
        // Fixtures can never dispatch an OS operation, even when a PID happens to match.
        if !self.shared.borrow().allow_process_actions || self.shared.borrow().snapshot.is_none() {
            self.shared.borrow_mut().process_action.notice =
                "需要取得实时系统数据后才能操作进程。".into();
            cx.notify();
            return;
        }
        self.shared.borrow_mut().process_action.busy = true;
        self.shared.borrow_mut().process_action.notice =
            format!("正在向 {name}（PID {}）发送请求…", identity.pid);
        let shared = self.shared.clone();
        cx.spawn(async move |weak, cx| {
            let pid = identity.pid;
            let result = smol::unblock(move || {
                process_control::send_signal_with_authentication(&identity, signal)
            })
            .await;
            shared
                .borrow_mut()
                .process_action
                .finish_action(&name, pid, result);
            let _ = weak.update(cx, |_, cx| cx.notify());
            // A reopened dashboard owns a new panel. Deliver completion there even
            // when the original panel no longer exists.
            let owner = shared.borrow().owner.clone();
            if let Some(owner) = owner {
                let _ = owner.update(cx, |owner, cx| {
                    if let Some(screen) = &owner.screen_view {
                        let _ = screen.update(cx, |screen, cx| screen.refresh(cx));
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    fn process_toolbar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.process_state.input.is_none() {
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder("搜索名称、PID 或用户…"));
            self.process_state.subscription =
                Some(cx.subscribe(&input, |this, input, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.process_state.query = input.read(cx).value().to_string();
                        this.table_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        cx.notify();
                    }
                }));
            self.process_state.input = Some(input);
        }
        let search = self.process_state.input.as_ref().map_or_else(
            || Empty.into_any_element(),
            |input| Input::new(input).small().into_any_element(),
        );
        let mut toolbar = div().flex().flex_col().gap_1().flex_none().child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .w(px(280.))
                        .flex_none()
                        .min_w_0()
                        .debug_selector(|| "process-search".into())
                        .child(search),
                )
                .children(
                    [
                        ("end-task", "结束任务…", ProcessSignal::Terminate),
                        ("force-quit", "强制退出…", ProcessSignal::Kill),
                    ]
                    .into_iter()
                    .map(|(id, label, signal)| {
                        let disabled =
                            self.selected.as_ref().is_none_or(|identity| {
                                process_action_block_reason(identity).is_some()
                            }) || self.shared.borrow().process_action.busy;
                        Button::new(id)
                            .small()
                            .ghost()
                            .label(label)
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(identity) = this.selected.clone() {
                                    this.prepare_process_action(identity, signal, cx);
                                }
                            }))
                    }),
                ),
        );
        if let Some((identity, name, signal)) = &self.process_state.confirmation {
            let force = *signal == ProcessSignal::Kill;
            let prompt = format!(
                "{} {name}（PID {}）？{}",
                if force {
                    "强制退出"
                } else {
                    "结束任务"
                },
                identity.pid,
                if force {
                    " 未保存的工作可能会丢失。"
                } else if cfg!(target_os = "windows") {
                    " 系统会请求应用正常关闭。应用可能会提示保存，或拒绝关闭。"
                } else {
                    " 系统会向进程发送终止请求。"
                }
            );
            toolbar = toolbar.child(
                div()
                    .id("process-action-confirmation")
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .p_2()
                    .bg(cx.theme().muted)
                    .role(Role::Group)
                    .accessibility_id("process-action-confirmation")
                    .aria_label(prompt.clone())
                    .child(prompt)
                    .child(
                        Button::new("cancel-process-action")
                            .accessibility_label("取消进程操作")
                            .small()
                            .child(
                                div()
                                    .debug_selector(|| "process-action:cancel".into())
                                    .child("取消"),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.controls["table"].handle.focus(window, cx);
                                this.process_state.confirmation = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("confirm-process-action")
                            .accessibility_label(if force {
                                "确认强制退出"
                            } else {
                                "确认结束任务"
                            })
                            .small()
                            .danger()
                            .child(
                                div()
                                    .debug_selector(|| "process-action:confirm".into())
                                    .child(if force {
                                        "强制退出"
                                    } else {
                                        "结束任务"
                                    }),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.controls["table"].handle.focus(window, cx);
                                this.confirm_process_action(cx);
                            })),
                    ),
            );
        }
        if !self.shared.borrow().process_action.notice.is_empty() {
            toolbar = toolbar.child(
                div()
                    .id("process-action-status")
                    .p_1()
                    .role(Role::Status)
                    .accessibility_id("process-action-status")
                    .aria_label(self.shared.borrow().process_action.notice.clone())
                    .child(self.shared.borrow().process_action.notice.clone()),
            );
        }
        toolbar.into_any_element()
    }

    fn request_keyboard_repaint(&mut self, window: &mut Window, cx: &Context<Self>) {
        if self.keyboard_repaint_pending {
            return;
        }
        self.keyboard_repaint_pending = true;
        // Dirtying this event makes GPUI redraw before the next queued key,
        // which can starve live snapshot delivery during ordinary key repeat.
        cx.on_next_frame(window, |this, window, cx| {
            this.keyboard_repaint_pending = false;
            window.refresh();
            cx.notify();
        });
    }

    pub(super) fn process_table(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let toolbar = self.process_toolbar(window, cx);
        let rows = Rc::new(self.process_projection());
        // Snapshot delivery may reorder rows after a key but before layout.
        // Consume the intent even when reconciliation cleared the selection.
        let reveal_pending = std::mem::take(&mut self.process_reveal_pending);
        if reveal_pending && let Some(index) = self.visible_selected_index(&rows) {
            self.table_scroll.scroll_to_item(index, ScrollStrategy::Top);
        }
        let reveal_identity = reveal_pending.then(|| self.selected.clone()).flatten();
        let revealing_selected_row = reveal_identity.is_some();
        let handle = self.controls["table"].handle.clone();
        let ring = cx.theme().ring;
        let count = rows.len();
        let widths = self.shared.borrow().process_widths;
        let width: f32 = processes::VISIBLE_PROCESS_COLUMNS
            .iter()
            .map(|&column| widths[column])
            .sum();
        let sizes = Rc::new(vec![size(px(width), window.rem_size() * 1.75); count]);
        let list = v_virtual_list(
            cx.entity(),
            "process-rows",
            sizes,
            move |this, range, _, cx| {
                let data = this.shared.borrow();
                range
                    .filter_map(|index| {
                        let process = data.processes.get(*rows.get(index)?)?;
                        let identity = process.identity.clone();
                        let menu_identity = identity.clone();
                        let entity = cx.entity().downgrade();
                        let reveal = reveal_identity.as_ref() == Some(&identity);
                        let outer = data.scroll.clone();
                        Some(
                            process_row(
                                process,
                                index,
                                this.selected.as_ref() == Some(&process.identity),
                                &data.process_widths,
                                cx,
                            )
                            .on_prepaint(move |mut bounds, window, _| {
                                if reveal {
                                    // A dock region may be taller than the window.
                                    // Reveal the selected row after virtual layout,
                                    // preserving the user's horizontal position.
                                    let viewport = outer.bounds();
                                    bounds.origin.x = viewport.origin.x;
                                    bounds.size.width = viewport.size.width;
                                    let shift = controls::reveal(bounds, &outer);
                                    if shift.y != px(0.) {
                                        window.refresh();
                                    }
                                }
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.selected = Some(identity.clone());
                                this.controls["table"].handle.focus(window, cx);
                                cx.notify();
                            }))
                            .context_menu(move |menu, _, _| {
                                let mut menu = menu;
                                for (label, signal) in [
                                    ("结束任务…", ProcessSignal::Terminate),
                                    ("强制退出…", ProcessSignal::Kill),
                                ] {
                                    let identity = menu_identity.clone();
                                    let entity = entity.clone();
                                    let disabled = process_action_block_reason(&identity).is_some();
                                    menu = menu.item(
                                        PopupMenuItem::new(label).disabled(disabled).on_click(
                                            move |_, _, cx| {
                                                let _ = entity.update(cx, |this, cx| {
                                                    this.prepare_process_action(
                                                        identity.clone(),
                                                        signal,
                                                        cx,
                                                    )
                                                });
                                            },
                                        ),
                                    );
                                }
                                menu
                            }),
                        )
                    })
                    .collect()
            },
        )
        .track_scroll(&self.table_scroll);
        let outer = self.shared.borrow().scroll.clone();
        let focus = self.controls["table"].clone();
        let horizontal = self.table_horizontal.clone();
        let table = crate::workspace::scroll_viewport(
            "process-table-viewport",
            "processes:viewport".into(),
        )
        .size_full()
        .relative()
        .track_focus(&handle)
        .border_1()
        .border_color(cx.theme().border)
        .focus_visible(move |style| style.border_color(ring))
        .debug_selector(|| "process-table".into())
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
            if event.keystroke.modifiers.alt {
                return;
            }
            if matches!(event.keystroke.key.as_str(), "left" | "right") {
                let old = this.table_horizontal.offset();
                let delta = if event.keystroke.key == "left" {
                    240.
                } else {
                    -240.
                };
                this.table_horizontal.set_offset(point(
                    (old.x + px(delta)).clamp(-this.table_horizontal.max_offset().x, px(0.)),
                    old.y,
                ));
                this.request_keyboard_repaint(window, cx);
                cx.stop_propagation();
                return;
            }
            let rows = this.process_projection();
            let count = rows.len();
            if count == 0 {
                return;
            }
            let current = this.visible_selected_index(&rows);
            let next = match event.keystroke.key.as_str() {
                "up" => current.unwrap_or(0).saturating_sub(1),
                "down" => current.map_or(0, |i| (i + 1).min(count - 1)),
                "home" => 0,
                "end" => count - 1,
                _ => return,
            };
            this.selected = Some(this.shared.borrow().processes[rows[next]].identity.clone());
            this.process_reveal_pending = true;
            this.request_keyboard_repaint(window, cx);
            cx.stop_propagation();
        }))
        .on_prepaint(move |bounds, window, _| {
            if focus.entered(window) && !revealing_selected_row {
                // Entering a tall region reveals its leading viewport. A
                // pending row reveal already chose the keyboard destination.
                let mut target = bounds.dilate(px(1.));
                target.size.height = target.size.height.min(outer.bounds().size.height);
                controls::reveal(target, &outer);
                window.refresh();
            }
        })
        .child(
            div()
                .id("process-horizontal")
                .size_full()
                .overflow_x_scroll()
                .track_scroll(&horizontal)
                .child(
                    Table::new("process-table")
                        .row_count(count + 1)
                        .column_count(processes::VISIBLE_PROCESS_COLUMNS.len())
                        .accessibility_label(format!(
                            "{count} 个可读取的进程；使用方向键导航并滚动列，按 Tab 离开表格"
                        ))
                        .w_full()
                        .min_w(px(width))
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(
                            TableRow::new("process-columns", 1)
                                .flex()
                                .h_7()
                                .flex_none()
                                .children(
                                    processes::VISIBLE_PROCESS_COLUMNS.iter().enumerate().map(
                                        |(visible_index, &column)| {
                                            let title = live::PROCESS_COLUMNS[column];
                                            let direction = (self.process_state.sort.column
                                                == column)
                                                .then_some(if self.process_state.sort.descending {
                                                    ramag_ui::SortDirection::Descending
                                                } else {
                                                    ramag_ui::SortDirection::Ascending
                                                });
                                            TableCell::new(
                                                ("process-heading", column),
                                                visible_index + 1,
                                            )
                                            .role(Role::ColumnHeader)
                                            .aria_label(format!("按 {title} 排序"))
                                            .w(px(widths[column]))
                                            .flex_none()
                                            .when(column == 1, |cell| cell.flex_grow(1.))
                                            .child(
                                                Button::new(("sort-process", column))
                                                    .accessibility_label(format!("按 {title} 排序"))
                                                    .ghost()
                                                    .small()
                                                    .w_full()
                                                    .px_2()
                                                    .justify_start()
                                                    .child(
                                                        ramag_ui::sortable_header_content(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .when(
                                                                    column == 0
                                                                        || (2..7).contains(&column),
                                                                    |label| label.text_right(),
                                                                )
                                                                .child(title),
                                                            direction,
                                                            cx.theme().muted_foreground,
                                                            cx.theme().foreground,
                                                        )
                                                        .debug_selector(move || {
                                                            format!("process-sort:{column}")
                                                        }),
                                                    )
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.process_state.sort.select(column);
                                                            this.process_reveal_pending = true;
                                                            cx.notify();
                                                        },
                                                    )),
                                            )
                                        },
                                    ),
                                ),
                        )
                        .child(
                            crate::workspace::scroll_viewport(
                                "process-row-clip",
                                "processes:rows-viewport".into(),
                            )
                            .flex_1()
                            .min_h_0()
                            .child(list),
                        ),
                ),
        )
        .child(ScrollableMask::new(
            Axis::Vertical,
            self.table_scroll.base_handle(),
        ))
        .child(Scrollbar::vertical(&self.table_scroll).mode(ScrollbarMode::Always))
        .child(Scrollbar::horizontal(&horizontal).mode(ScrollbarMode::Always))
        .into_any_element();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_1()
            .child(toolbar)
            .child(div().flex_1().min_h_0().child(table))
            .when(self.standalone, |view| view.child(self.process_details(cx)))
            .when(count == 0, |view| {
                view.child(div().p_2().child("没有符合条件的进程。"))
            })
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "process_panel_tests.rs"]
mod tests;
