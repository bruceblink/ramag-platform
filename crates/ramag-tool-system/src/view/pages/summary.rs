//! Summary page presents primary utilization, recent trends, and process activity.

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, Window, div,
    px,
};
use ramag_infra_system::{MonitorKind, Unit};

use super::super::{SystemSection, SystemView, helpers};
use super::{summary_activity, summary_metrics};
use crate::MonitorSnapshot;

impl SystemView {
    pub(super) fn render_summary(
        &self,
        snapshot: &MonitorSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cpu = summary_metrics::primary_sensor(snapshot, MonitorKind::Cpu);
        let memory = summary_metrics::primary_sensor(snapshot, MonitorKind::Memory);
        let maximum_gap = self.monitor.refresh_interval().duration().as_secs_f64() * 3.0;
        let selected_gpu = self
            .presentation
            .selected_devices
            .get("gpu")
            .map(String::as_str);
        let cpu_metrics = summary_metrics::cpu_metrics(snapshot, selected_gpu, cpu);
        let cpu_meters = summary_metrics::render_cpu_meters(snapshot, &cpu_metrics, cx);
        let cpu_chart = div()
            .debug_selector(|| "system-summary-cpu-overview".into())
            .child(summary_metrics::primary_chart(
                "CPU overview",
                cpu,
                snapshot,
                maximum_gap,
                cx.theme().success,
                cx,
            ));
        let memory_panel = summary_metrics::render_memory_panel(memory, snapshot, maximum_gap, cx);
        let processes = self.render_top_processes(snapshot, cx);
        let activity = summary_activity::render_activity_cards(self, snapshot, cx);
        let compact = f32::from(window.viewport_size().width) < 760.0;
        let mut body = v_flex()
            .debug_selector(|| "system-performance-body".into())
            .w_full()
            .min_w_0()
            .gap(px(14.0))
            .p(px(if compact { 12.0 } else { 20.0 }))
            .child(helpers::page_title(
                SystemSection::Summary,
                "CPU、内存与设备活动",
                cx,
            ))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_stretch()
                    .gap(px(10.0))
                    .child(cpu_meters)
                    .child(cpu_chart)
                    .child(processes),
            )
            .child(memory_panel);
        if let Some(error) = helpers::diagnostic_text(snapshot) {
            body = body.child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Failed,
                error,
                cx,
            ));
        }
        if snapshot.collection_stale {
            body = body.child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Stale,
                "采集器没有返回新快照；保留最近一次读数并标记为过期。",
                cx,
            ));
        }
        body.child(activity).into_any_element()
    }

    fn render_top_processes(&self, snapshot: &MonitorSnapshot, cx: &Context<Self>) -> AnyElement {
        let mut processes = snapshot.host.processes.iter().collect::<Vec<_>>();
        processes.sort_by(|left, right| {
            compare_optional_desc(
                snapshot.process_value(&left.cpu_percent),
                snapshot.process_value(&right.cpu_percent),
            )
        });
        let mut content = v_flex().w_full().min_w_0().gap(px(5.0));
        if processes.is_empty() {
            content = content.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("暂无进程读数"),
            );
        }
        for process in processes.into_iter().take(5) {
            content = content.child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(54.0))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(process.identity.pid.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_sm()
                            .child(process.name.clone()),
                    )
                    .child(
                        div()
                            .w(px(64.0))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                snapshot
                                    .process_value(&process.cpu_percent)
                                    .map_or_else(|| "—".into(), |value| format!("{value:.1}%")),
                            ),
                    )
                    .child(
                        div()
                            .w(px(76.0))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(snapshot.process_value(&process.memory_bytes).map_or_else(
                                || "—".into(),
                                |value| helpers::format_value(value, &Unit::Bytes),
                            )),
                    ),
            );
        }
        ramag_ui::pulse_ui::pulse_panel(cx)
            .debug_selector(|| "system-summary-top-cpu-processes".into())
            .flex_1()
            .min_w(px(250.0))
            .border_color(cx.theme().success.opacity(0.28))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .child("Top CPU processes"),
                    )
                    .child(content),
            )
            .into_any_element()
    }
}

fn compare_optional_desc(left: Option<f64>, right: Option<f64>) -> std::cmp::Ordering {
    right
        .filter(|value| value.is_finite())
        .partial_cmp(&left.filter(|value| value.is_finite()))
        .unwrap_or(std::cmp::Ordering::Equal)
}
