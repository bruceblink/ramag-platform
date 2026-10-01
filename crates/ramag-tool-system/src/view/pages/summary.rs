//! Summary page presents primary utilization, recent trends, and process activity.

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, Window, div,
    px,
};
use ramag_infra_system::{MonitorKind, SensorDescriptor, Unit};

use super::super::{SystemSection, SystemView, helpers};
use crate::{MonitorSnapshot, SensorSample};

impl SystemView {
    pub(super) fn render_summary(
        &self,
        snapshot: &MonitorSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cpu = primary_sensor(snapshot, MonitorKind::Cpu);
        let memory = primary_sensor(snapshot, MonitorKind::Memory);
        let cpu_sample = cpu.and_then(|sensor| snapshot.latest(&sensor.id));
        let memory_sample = memory.and_then(|sensor| snapshot.latest(&sensor.id));
        let cpu_value = cpu_sample.and_then(SensorSample::chart_value);
        let memory_value = memory_sample.and_then(SensorSample::chart_value);
        let detail = |sensor: Option<&SensorDescriptor>, sample: Option<&SensorSample>| match (
            sensor, sample,
        ) {
            (Some(sensor), Some(sample)) if sample.status == crate::ReadingStatus::Current => {
                match (sample.value, sample.total) {
                    (Some(value), Some(total)) => format!(
                        "{} / {}",
                        helpers::format_value(value, &sensor.unit),
                        helpers::format_value(total, &sensor.unit)
                    ),
                    _ if !sensor.scope.is_empty() => sensor.scope.clone(),
                    _ => "当前采样".into(),
                }
            }
            (_, Some(sample)) => sample
                .reason
                .clone()
                .unwrap_or_else(|| sample.status.label().to_owned()),
            (_, None) => "等待采样".into(),
        };
        let cpu_parts = cpu_value.map(|value| helpers::metric_parts(value, &Unit::Percent));
        let memory_parts = memory_value
            .zip(memory)
            .map(|(value, sensor)| helpers::metric_parts(value, &sensor.unit));
        let cpu_card = ramag_ui::pulse_ui::pulse_metric_card(
            "CPU 利用率",
            cpu_parts
                .as_ref()
                .map_or_else(|| "—".into(), |parts| parts.0.clone()),
            Some(cpu_parts.as_ref().map_or("%", |parts| parts.1.as_str())),
            Some(detail(cpu, cpu_sample)),
            cx,
        );
        let memory_card = ramag_ui::pulse_ui::pulse_metric_card(
            "内存占用",
            memory_parts
                .as_ref()
                .map_or_else(|| "—".into(), |parts| parts.0.clone()),
            Some(memory_parts.as_ref().map_or("B", |parts| parts.1.as_str())),
            Some(detail(memory, memory_sample)),
            cx,
        );
        let maximum_gap = self.monitor.refresh_interval().duration().as_secs_f64() * 3.0;
        let charts = h_flex()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .gap(px(12.0))
            .child(primary_chart(
                "CPU 使用率",
                cpu,
                snapshot,
                maximum_gap,
                cx.theme().success,
                cx,
            ))
            .child(primary_chart(
                "内存使用",
                memory,
                snapshot,
                maximum_gap,
                cx.theme().magenta,
                cx,
            ));
        let processes = self.render_top_processes(snapshot, cx);
        let subsystems = self.render_subsystems(snapshot, cx);
        let compact = f32::from(window.viewport_size().width) < 720.0;
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
                    .gap(px(10.0))
                    .child(cpu_card)
                    .child(memory_card),
            )
            .child(charts);
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
        body.child(processes).child(subsystems).into_any_element()
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
            .debug_selector(|| "system-summary-processes".into())
            .child(
                v_flex()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .child("占用较高的进程"),
                    )
                    .child(content),
            )
            .into_any_element()
    }

    fn render_subsystems(&self, snapshot: &MonitorSnapshot, cx: &Context<Self>) -> AnyElement {
        let mut cards = h_flex().w_full().min_w_0().flex_wrap().gap(px(8.0));
        for (kind, label, section) in [
            (MonitorKind::Gpu, "GPU", SystemSection::Gpu),
            (MonitorKind::Volume, "磁盘", SystemSection::Disks),
            (MonitorKind::Network, "网络", SystemSection::Network),
        ] {
            let count = snapshot
                .host
                .monitors
                .iter()
                .filter(|monitor| monitor.kind == kind)
                .count();
            let view = cx.entity().clone();
            let item = ramag_ui::pulse_ui::pulse_panel(cx)
                .flex_1()
                .min_w(px(130.0))
                .debug_selector(|| format!("system-subsystem-{}", section.id()))
                .child(
                    v_flex()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .child(if count == 0 {
                                    "不可用".to_owned()
                                } else {
                                    format!("{count} 个设备")
                                }),
                        ),
                );
            cards = cards.child(
                item.on_mouse_up(gpui_kit::MouseButton::Left, move |_, _, app| {
                    view.update(app, |view, cx| view.select_section(section, cx));
                }),
            );
        }
        cards.into_any_element()
    }
}

fn primary_sensor(snapshot: &MonitorSnapshot, kind: MonitorKind) -> Option<&SensorDescriptor> {
    let monitor = snapshot
        .host
        .monitors
        .iter()
        .find(|monitor| monitor.kind == kind)?;
    snapshot
        .host
        .sensors
        .iter()
        .find(|sensor| sensor.id == monitor.summary_sensor_id)
        .or_else(|| {
            snapshot
                .host
                .sensors
                .iter()
                .find(|sensor| sensor.monitor_id == monitor.id)
        })
}

fn primary_chart(
    title: &'static str,
    descriptor: Option<&SensorDescriptor>,
    snapshot: &MonitorSnapshot,
    maximum_gap: f64,
    line: gpui_kit::Hsla,
    cx: &Context<SystemView>,
) -> AnyElement {
    let Some(descriptor) = descriptor else {
        return ramag_ui::pulse_ui::pulse_panel(cx)
            .flex_1()
            .min_w(px(280.0))
            .child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                format!("{title}暂无可用传感器"),
                cx,
            ))
            .into_any_element();
    };
    let points = helpers::chart_points(snapshot, &descriptor.id, maximum_gap);
    let max = chart_max(descriptor, snapshot);
    ramag_ui::pulse_ui::pulse_panel(cx)
        .flex_1()
        .min_w(px(280.0))
        .gap(px(8.0))
        .child(
            v_flex()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{} · {}",
                            descriptor.title,
                            helpers::unit_label(&descriptor.unit)
                        )),
                ),
        )
        .child(helpers::render_chart(
            &points,
            max,
            &descriptor.unit,
            px(116.0),
            line,
            cx,
        ))
        .into_any_element()
}

fn chart_max(descriptor: &SensorDescriptor, snapshot: &MonitorSnapshot) -> f64 {
    if let Some(scale) = descriptor
        .scale
        .filter(|scale| scale.is_finite() && *scale > 0.0)
    {
        return scale;
    }
    if descriptor.unit == Unit::Percent {
        return 100.0;
    }
    let observed = snapshot
        .histories
        .get(&descriptor.id)
        .into_iter()
        .flatten()
        .flat_map(|sample| [sample.chart_value(), sample.total])
        .flatten()
        .filter(|value| value.is_finite())
        .map(f64::abs)
        .reduce(f64::max);
    observed.map_or_else(
        || match descriptor.unit {
            Unit::Celsius => 100.0,
            Unit::Hertz => 1_000_000_000.0,
            _ => 1.0,
        },
        |value| (value * 1.15).max(f64::EPSILON),
    )
}

fn compare_optional_desc(left: Option<f64>, right: Option<f64>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.total_cmp(&left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}
