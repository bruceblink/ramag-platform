//! Process detail panel rendering.
use super::*;

impl MonitorPanel {
    /// Renders the selected process details without changing the table selection.
    pub(super) fn process_details(&self, cx: &App) -> AnyElement {
        let data = self.shared.borrow();
        let selected = self
            .selected
            .as_ref()
            .and_then(|identity| data.processes.iter().find(|row| &row.identity == identity));
        let Some(row) = selected else {
            return div()
                .id("process-details")
                .accessibility_id("process-details")
                .role(Role::Group)
                .aria_label("进程详情")
                .p_3()
                .flex_none()
                .border_t_1()
                .border_color(cx.theme().border)
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("选择进程以查看当前数据。")
                .into_any_element();
        };
        let name = row.cells.get(1).cloned().unwrap_or_default();
        div()
            .id("process-details")
            .accessibility_id("process-details")
            .debug_selector(|| "process-details".into())
            .role(Role::Group)
            .aria_label(format!("{name} 的详情，PID {}", row.identity.pid))
            .flex_none()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(crate::screen_style::palette(cx).surface)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(crate::screen_style::heading(name, 18., cx))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("PID {}", row.identity.pid)),
                    ),
            )
            .child(
                div().flex().flex_wrap().gap_4().children(
                    processes::PROCESS_DETAIL_COLUMNS
                        .iter()
                        .copied()
                        .map(|column| {
                            let selector = format!("process-detail-field:{column}");
                            div()
                                .debug_selector(move || selector.clone())
                                .flex()
                                .flex_col()
                                .gap_1()
                                .min_w(px(110.))
                                .flex_1()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(live::PROCESS_COLUMNS[column]),
                                )
                                .child(
                                    crate::meters::metric_label(
                                        format!("process-detail:{column}"),
                                        row.cells.get(column).cloned().unwrap_or_default(),
                                    )
                                    .text_sm()
                                    .font_family(cx.theme().mono_font_family.clone()),
                                )
                        }),
                ),
            )
            .into_any_element()
    }
}
