//! Device and sensor pages group readings by real collector descriptors.

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, Window, div,
    px,
};
use ramag_infra_system::{MonitorKind, SensorDescriptor, SensorKind};

use super::super::{SystemSection, SystemView, helpers};
use crate::{MonitorSnapshot, SensorSample};

impl SystemView {
    pub(super) fn render_device_page(
        &mut self,
        section: SystemSection,
        snapshot: &MonitorSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let kind = match section {
            SystemSection::Cpu => MonitorKind::Cpu,
            SystemSection::Memory => MonitorKind::Memory,
            SystemSection::Gpu => MonitorKind::Gpu,
            SystemSection::Disks => MonitorKind::Volume,
            SystemSection::Network => MonitorKind::Network,
            _ => return div().into_any_element(),
        };
        let line = match section {
            SystemSection::Cpu => cx.theme().success,
            SystemSection::Memory => cx.theme().magenta,
            SystemSection::Gpu => cx.theme().cyan,
            SystemSection::Disks => cx.theme().warning,
            SystemSection::Network => cx.theme().accent,
            _ => cx.theme().accent,
        };
        let monitors = snapshot
            .host
            .monitors
            .iter()
            .filter(|monitor| monitor.kind == kind)
            .collect::<Vec<_>>();
        let devices = monitors
            .iter()
            .map(|monitor| (monitor.id.clone().into(), monitor.title.clone().into()))
            .collect::<Vec<_>>();
        let selected = self
            .presentation
            .selected_devices
            .get(section.id())
            .map(String::as_str)
            .or_else(|| {
                if section == SystemSection::Network {
                    snapshot
                        .host
                        .preferred_network_monitor_id
                        .as_deref()
                        .filter(|preferred| monitors.iter().any(|monitor| monitor.id == *preferred))
                } else {
                    None
                }
            })
            .or_else(|| monitors.first().map(|monitor| monitor.id.as_str()));
        let view = cx.entity().clone();
        let section_for_select = section;
        let selector =
            ramag_ui::pulse_ui::pulse_device_selector(&devices, selected, cx, move |id, _, app| {
                view.update(app, |this, cx| {
                    this.presentation
                        .selected_devices
                        .insert(section_for_select.id().to_owned(), id.to_string());
                    this.save_presentation(cx);
                    cx.notify();
                });
            });

        let body = if let Some(selected_id) = selected {
            if let Some(monitor) = monitors.iter().find(|monitor| monitor.id == selected_id) {
                let readings = snapshot
                    .host
                    .sensors
                    .iter()
                    .filter(|sensor| sensor.monitor_id == monitor.id)
                    .filter(|sensor| !self.presentation.hidden_sensors.contains(&sensor.id))
                    .collect::<Vec<_>>();
                if readings.is_empty() {
                    ramag_ui::pulse_ui::pulse_status_notice(
                        ramag_ui::pulse_ui::PulseStatus::Unavailable,
                        "此设备当前没有可显示的传感器数据",
                        cx,
                    )
                    .into_any_element()
                } else {
                    sensor_grid(
                        readings,
                        snapshot,
                        self.monitor.refresh_interval().duration().as_secs_f64() * 3.0,
                        line,
                        cx,
                    )
                }
            } else {
                let saved_name = self
                    .presentation
                    .selected_devices
                    .get(section.id())
                    .map(String::as_str)
                    .unwrap_or(selected_id);
                ramag_ui::pulse_ui::pulse_status_notice(
                    ramag_ui::pulse_ui::PulseStatus::Unavailable,
                    format!("已保存的设备 {saved_name} 当前不可用；选择设备以查看其他设备。"),
                    cx,
                )
                .into_any_element()
            }
        } else {
            ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "当前平台没有发现此类设备。",
                cx,
            )
            .into_any_element()
        };
        let title = helpers::page_title(section, page_description(section), cx);
        let compact = f32::from(window.viewport_size().width) < 720.0;
        v_flex()
            .debug_selector(|| format!("system-page-{}", section.id()))
            .w_full()
            .min_w_0()
            .gap(px(14.0))
            .p(px(if compact { 12.0 } else { 20.0 }))
            .child(title)
            .child(selector)
            .child(body)
            .into_any_element()
    }

    pub(super) fn render_sensor_kind_page(
        &self,
        section: SystemSection,
        snapshot: &MonitorSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if section == SystemSection::Energy {
            return self.render_energy_page(snapshot, cx);
        }
        let kind = SensorKind::Temperature;
        let readings = snapshot
            .host
            .sensors
            .iter()
            .filter(|sensor| sensor.kind == kind)
            .filter(|sensor| !self.presentation.hidden_sensors.contains(&sensor.id))
            .collect::<Vec<_>>();
        let content = if readings.is_empty() {
            let message = "当前平台没有可用温度传感器，或温度采集尚未启用。";
            ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                message,
                cx,
            )
            .into_any_element()
        } else {
            let line = cx.theme().danger;
            sensor_grid(
                readings,
                snapshot,
                self.monitor.refresh_interval().duration().as_secs_f64() * 3.0,
                line,
                cx,
            )
        };
        v_flex()
            .debug_selector(|| format!("system-page-{}", section.id()))
            .w_full()
            .min_w_0()
            .gap(px(14.0))
            .p(px(20.0))
            .child(helpers::page_title(section, page_description(section), cx))
            .child(content)
            .into_any_element()
    }

    /// Renders the selected power channel and the complete measured-channel list.
    fn render_energy_page(&self, snapshot: &MonitorSnapshot, cx: &mut Context<Self>) -> AnyElement {
        let readings = snapshot
            .host
            .sensors
            .iter()
            .filter(|sensor| sensor.kind == SensorKind::Power)
            .filter(|sensor| !self.presentation.hidden_sensors.contains(&sensor.id))
            .collect::<Vec<_>>();
        let saved_id = self
            .presentation
            .selected_sensors
            .get("energy")
            .map(String::as_str);
        let selected = selected_power_sensor(&readings, saved_id);
        let selector = super::energy::energy_sensor_selector(
            &readings,
            saved_id,
            selected.map(|sensor| sensor.id.as_str()),
            cx.entity().clone(),
        );
        let maximum_gap = self.monitor.refresh_interval().duration().as_secs_f64() * 3.0;
        let primary = selected
            .map(|sensor| super::energy::energy_primary(sensor, snapshot, maximum_gap, cx))
            .unwrap_or_else(|| {
                let message = saved_id.map_or(
                    "当前平台没有可用功率传感器。",
                    |_| "已保存的功率传感器当前不可用；请选择其他传感器。",
                );
                ramag_ui::pulse_ui::pulse_status_notice(
                    ramag_ui::pulse_ui::PulseStatus::Unavailable,
                    message,
                    cx,
                )
                .into_any_element()
            });
        let channels = if readings.is_empty() {
            div()
                .debug_selector(|| "system-energy-measured-channels".into())
                .into_any_element()
        } else {
            v_flex()
                .debug_selector(|| "system-energy-measured-channels".into())
                .w_full()
                .min_w_0()
                .gap(px(8.0))
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child("Measured power channels"),
                )
                .child(sensor_grid(
                    readings,
                    snapshot,
                    maximum_gap,
                    cx.theme().warning,
                    cx,
                ))
                .into_any_element()
        };
        v_flex()
            .debug_selector(|| "system-page-energy".into())
            .w_full()
            .min_w_0()
            .gap(px(14.0))
            .p(px(20.0))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(helpers::page_title(
                        SystemSection::Energy,
                        page_description(SystemSection::Energy),
                        cx,
                    ))
                    .child(selector),
            )
            .child(primary)
            .child(channels)
            .into_any_element()
    }
}

/// Resolves a saved power sensor without silently replacing a missing identity.
pub(crate) fn selected_power_sensor<'a>(
    readings: &[&'a SensorDescriptor],
    saved_id: Option<&str>,
) -> Option<&'a SensorDescriptor> {
    match saved_id {
        Some(id) => readings.iter().find(|sensor| sensor.id == id).copied(),
        None => readings
            .iter()
            .find(|sensor| sensor.title.eq_ignore_ascii_case("CPU package power"))
            .copied()
            .or_else(|| readings.first().copied()),
    }
}

fn page_description(section: SystemSection) -> &'static str {
    match section {
        SystemSection::Cpu => "处理器利用率、频率与核心传感器",
        SystemSection::Memory => "物理内存和交换空间读数",
        SystemSection::Gpu => "图形设备利用率、显存与温度",
        SystemSection::Disks => "卷容量与设备活动读数",
        SystemSection::Network => "接口流量和链路状态",
        SystemSection::Energy => "硬件功率与能耗传感器",
        SystemSection::Thermals => "温度与散热状态",
        _ => "系统传感器读数",
    }
}

fn sensor_grid(
    readings: Vec<&SensorDescriptor>,
    snapshot: &MonitorSnapshot,
    maximum_gap: f64,
    line: gpui_kit::Hsla,
    cx: &Context<SystemView>,
) -> AnyElement {
    let mut grid = h_flex()
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_stretch()
        .gap(px(10.0));
    for descriptor in readings {
        let sample = snapshot.latest(&descriptor.id);
        grid = grid.child(sensor_card(
            descriptor,
            sample,
            snapshot,
            maximum_gap,
            line,
            cx,
        ));
    }
    grid.into_any_element()
}

fn sensor_card(
    descriptor: &SensorDescriptor,
    sample: Option<&SensorSample>,
    snapshot: &MonitorSnapshot,
    maximum_gap: f64,
    line: gpui_kit::Hsla,
    cx: &Context<SystemView>,
) -> AnyElement {
    let (status, current) = super::energy::displayed_sensor_state(snapshot, sample);
    let message = current.map_or_else(
        || status.label().to_owned(),
        |value| helpers::format_value(value, &descriptor.unit),
    );
    let detail = sample.map_or_else(
        || "等待采样".into(),
        |sample| {
            sample
                .reason
                .clone()
                .unwrap_or_else(|| match sample.status {
                    crate::ReadingStatus::Current => "读数已更新".into(),
                    _ => sample.status.label().to_owned(),
                })
        },
    );
    let points = helpers::chart_points(snapshot, &descriptor.id, maximum_gap);
    ramag_ui::pulse_ui::pulse_panel(cx)
        .debug_selector(|| format!("system-sensor-{}", selector_id(&descriptor.id)))
        .flex_1()
        .min_w(px(230.0))
        .min_h(px(142.0))
        .gap(px(8.0))
        .child(
            v_flex().gap(px(2.0)).child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(helpers::sensor_title(descriptor)),
            ),
        )
        .child(
            h_flex()
                .items_baseline()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child(message),
                )
                .child(ramag_ui::pulse_ui::pulse_status_badge(
                    helpers::status(status),
                    cx,
                )),
        )
        .child(helpers::render_chart(
            &points,
            chart_max(descriptor, snapshot),
            &descriptor.unit,
            px(54.0),
            line,
            cx,
        ))
        .child(
            div()
                .w_full()
                .min_w_0()
                .whitespace_normal()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(detail),
        )
        .into_any_element()
}

fn selector_id(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

pub(super) fn chart_max(descriptor: &SensorDescriptor, snapshot: &MonitorSnapshot) -> f64 {
    if let Some(scale) = descriptor
        .scale
        .filter(|value| value.is_finite() && *value > 0.0)
    {
        return scale;
    }
    if descriptor.unit == ramag_infra_system::Unit::Percent {
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
            ramag_infra_system::Unit::Celsius => 100.0,
            ramag_infra_system::Unit::Hertz => 1_000_000_000.0,
            _ => 1.0,
        },
        |value| (value * 1.15).max(f64::EPSILON),
    )
}
