//! Temperature selection and overview composition for the Thermals page.

use gpui_kit::base::Disableable as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Styled, div,
    px,
};
use ramag_infra_system::{SensorDescriptor, SensorKind, Unit};
use ramag_ui::PointerDropdownMenu as _;

use super::super::{SystemSection, SystemView, helpers};
use crate::{MonitorSnapshot, ReadingStatus};

impl SystemView {
    /// Renders the hottest summary, selected sensor history, and every visible temperature card.
    pub(super) fn render_thermals_page(
        &self,
        snapshot: &MonitorSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let readings = snapshot
            .host
            .sensors
            .iter()
            .filter(|sensor| sensor.kind == SensorKind::Temperature)
            .filter(|sensor| sensor.unit == Unit::Celsius)
            .filter(|sensor| !self.presentation.hidden_sensors.contains(&sensor.id))
            .collect::<Vec<_>>();
        let saved_id = self
            .presentation
            .selected_sensors
            .get("thermals")
            .map(String::as_str);
        let selected = selected_thermal_sensor(&readings, saved_id, snapshot);
        let selector = thermal_sensor_selector(
            &readings,
            saved_id,
            selected.map(|sensor| sensor.id.as_str()),
            snapshot,
            cx.entity().clone(),
        );
        let maximum_gap = self.monitor.refresh_interval().duration().as_secs_f64() * 3.0;
        let primary = selected
            .map(|sensor| thermal_primary(sensor, snapshot, maximum_gap, cx))
            .unwrap_or_else(|| {
                let message = saved_id.map_or(
                    "当前平台没有可用温度传感器，或温度采集尚未启用。",
                    |_| "已保存的温度传感器当前不可用；请选择其他传感器。",
                );
                ramag_ui::pulse_ui::pulse_status_notice(
                    ramag_ui::pulse_ui::PulseStatus::Unavailable,
                    message,
                    cx,
                )
                .debug_selector(|| "system-thermals-primary-unavailable".into())
                .into_any_element()
            });
        let hottest = hottest_panel(&readings, snapshot, cx);
        let authorization = cpu_temperature_authorization(self, cx);
        let channels = if readings.is_empty() {
            div()
                .debug_selector(|| "system-thermals-temperature-sensors".into())
                .into_any_element()
        } else {
            v_flex()
                .debug_selector(|| "system-thermals-temperature-sensors".into())
                .w_full()
                .min_w_0()
                .gap(px(8.0))
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child("温度传感器"),
                )
                .child(super::devices::sensor_grid(
                    readings,
                    snapshot,
                    maximum_gap,
                    cx.theme().warning,
                    cx,
                ))
                .into_any_element()
        };
        v_flex()
            .debug_selector(|| "system-page-thermals".into())
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
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child(SystemSection::Thermals.title()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("温度与散热状态"),
                            ),
                    )
                    .child(selector)
                    .when_some(authorization, |header, control| header.child(control)),
            )
            .child(hottest)
            .child(primary)
            .child(channels)
            .into_any_element()
    }
}

/// Resolves a saved temperature sensor without silently replacing a missing identity.
pub(crate) fn selected_thermal_sensor<'a>(
    readings: &[&'a SensorDescriptor],
    saved_id: Option<&str>,
    snapshot: &MonitorSnapshot,
) -> Option<&'a SensorDescriptor> {
    match saved_id {
        Some(id) => readings.iter().find(|sensor| sensor.id == id).copied(),
        None => hottest_current_temperature(readings, snapshot)
            .map(|(sensor, _)| sensor)
            .or_else(|| {
                readings
                    .iter()
                    .min_by(|left, right| left.id.cmp(&right.id))
                    .copied()
            }),
    }
}

/// Finds the highest finite current Celsius sample; ties use the stable sensor ID.
pub(crate) fn hottest_current_temperature<'a>(
    readings: &[&'a SensorDescriptor],
    snapshot: &MonitorSnapshot,
) -> Option<(&'a SensorDescriptor, f64)> {
    let mut hottest: Option<(&SensorDescriptor, f64)> = None;
    for descriptor in readings {
        if descriptor.kind != SensorKind::Temperature || descriptor.unit != Unit::Celsius {
            continue;
        }
        let sample = snapshot.latest(&descriptor.id);
        let (status, value) = super::energy::displayed_sensor_state(snapshot, sample);
        let Some(value) = value.filter(|value| value.is_finite()) else {
            continue;
        };
        if status != ReadingStatus::Current {
            continue;
        }
        let replace = match hottest {
            None => true,
            Some((current, current_value)) => {
                value > current_value
                    || (value == current_value && descriptor.id.as_str() < current.id.as_str())
            }
        };
        if replace {
            hottest = Some((*descriptor, value));
        }
    }
    hottest
}

/// Builds the temperature sensor menu and persists the stable sensor ID.
fn thermal_sensor_selector(
    readings: &[&SensorDescriptor],
    saved_id: Option<&str>,
    selected_id: Option<&str>,
    snapshot: &MonitorSnapshot,
    view: Entity<SystemView>,
) -> AnyElement {
    let choices = readings
        .iter()
        .map(|sensor| (sensor.id.clone(), thermal_sensor_label(snapshot, sensor)))
        .collect::<Vec<_>>();
    let label = selected_id
        .and_then(|id| choices.iter().find(|(sensor, _)| sensor == id))
        .map(|(_, title)| title.clone())
        .unwrap_or_else(|| {
            saved_id
                .map(|id| format!("不可用 · {id}"))
                .unwrap_or_else(|| "选择温度传感器".into())
        });
    let selected_id = selected_id.map(str::to_owned);
    ramag_ui::clickable_button("system-thermals-sensor-selector")
        .debug_selector(|| "system-thermals-sensor-selector".into())
        .small()
        .max_w(px(360.0))
        .label(label)
        .dropdown_caret(true)
        .pointer_dropdown_menu(move |mut menu, _, _| {
            menu = menu.scrollable(true).max_h(px(320.0));
            for (id, title) in &choices {
                let id = id.clone();
                let option_id = selector_id(&id);
                let target = view.clone();
                menu = menu.item(
                    gpui_kit::component::menu::PopupMenuItem::element({
                        let title = title.clone();
                        let option_id = option_id.clone();
                        move |_, _| {
                            let selector = format!("system-thermals-sensor-option-{option_id}");
                            div()
                                .debug_selector(move || selector.clone())
                                .w_full()
                                .child(title.clone())
                        }
                    })
                    .checked(selected_id.as_deref() == Some(id.as_str()))
                    .on_click(move |_, _, cx| {
                        target.update(cx, |this, cx| {
                            this.presentation
                                .selected_sensors
                                .insert("thermals".into(), id.clone());
                            this.save_presentation(cx);
                            cx.notify();
                        });
                    }),
                );
            }
            menu
        })
        .into_any_element()
}

fn hottest_panel(
    readings: &[&SensorDescriptor],
    snapshot: &MonitorSnapshot,
    cx: &Context<SystemView>,
) -> AnyElement {
    let Some((descriptor, value)) = hottest_current_temperature(readings, snapshot) else {
        return ramag_ui::pulse_ui::pulse_status_notice(
            ramag_ui::pulse_ui::PulseStatus::Unavailable,
            "当前没有可用的温度读数。",
            cx,
        )
        .debug_selector(|| "system-thermals-hottest".into())
        .into_any_element();
    };
    h_flex()
        .debug_selector(|| "system-thermals-hottest".into())
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "当前最热传感器 · {}",
                    thermal_sensor_label(snapshot, descriptor)
                )),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().warning)
                .child(helpers::format_value(value, &descriptor.unit)),
        )
        .into_any_element()
}

fn thermal_sensor_label(snapshot: &MonitorSnapshot, descriptor: &SensorDescriptor) -> String {
    let device = snapshot
        .host
        .monitors
        .iter()
        .find(|monitor| monitor.id == descriptor.monitor_id)
        .map(|monitor| monitor.title.as_str())
        .filter(|title| !title.is_empty())
        .unwrap_or(descriptor.scope.as_str());
    format!("{device} · {}", helpers::sensor_title(descriptor))
}

fn thermal_primary(
    descriptor: &SensorDescriptor,
    snapshot: &MonitorSnapshot,
    maximum_gap: f64,
    cx: &Context<SystemView>,
) -> AnyElement {
    let sample = snapshot.latest(&descriptor.id);
    let (status, current) = super::energy::displayed_sensor_state(snapshot, sample);
    let value = current.map_or_else(
        || status.label().to_owned(),
        |value| helpers::format_value(value, &descriptor.unit),
    );
    let (minimum, scale) = super::devices::chart_range(descriptor, snapshot);
    let fill = current
        .map(|value| ((value - minimum) / (scale - minimum)).clamp(0.0, 1.0) as f32)
        .unwrap_or(0.0);
    let points = helpers::chart_points(snapshot, &descriptor.id, maximum_gap);
    let note = if status == ReadingStatus::Current {
        "量表按历史读数范围显示".to_owned()
    } else {
        sample
            .and_then(|sample| sample.reason.clone())
            .unwrap_or_else(|| status.label().to_owned())
    };
    let domain = if descriptor.scope.is_empty() {
        format!(
            "{} · {}",
            descriptor.source,
            helpers::unit_label(&descriptor.unit)
        )
    } else {
        format!(
            "{} · {} · {}",
            descriptor.source,
            descriptor.scope,
            helpers::unit_label(&descriptor.unit)
        )
    };
    v_flex()
        .w_full()
        .min_w_0()
        .border_color(cx.theme().warning.opacity(0.42))
        .gap(px(8.0))
        .child(
            h_flex()
                .debug_selector(|| "system-thermals-primary-meter".into())
                .w_full()
                .min_w_0()
                .items_center()
                .gap(px(16.0))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .h(px(16.0))
                        .gap(px(2.0))
                        .children((0..32).map(|index| {
                            let active = (index as f32) < fill * 32.0;
                            div().flex_1().h_full().rounded(px(2.0)).bg(if active {
                                cx.theme().warning
                            } else {
                                cx.theme().muted.opacity(0.22)
                            })
                        })),
                )
                .child(
                    div()
                        .debug_selector(|| "system-thermals-primary-value".into())
                        .text_2xl()
                        .text_color(cx.theme().warning)
                        .child(value),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{} · {}", helpers::sensor_title(descriptor), note)),
        )
        .child(
            ramag_ui::pulse_ui::pulse_panel(cx)
                .gap(px(8.0))
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(helpers::sensor_title(descriptor)),
                )
                .child(
                    div()
                        .debug_selector(|| "system-thermals-domain".into())
                        .w_full()
                        .min_w_0()
                        .whitespace_normal()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(domain),
                )
                .child(
                    helpers::render_chart_with_range(
                        &points,
                        minimum,
                        scale,
                        &descriptor.unit,
                        px(260.0),
                        cx.theme().warning,
                        cx,
                    )
                    .debug_selector(|| "system-thermals-primary-chart".into()),
                ),
        )
        .into_any_element()
}

#[cfg(target_os = "windows")]
fn cpu_temperature_authorization(
    view: &SystemView,
    cx: &mut Context<SystemView>,
) -> Option<AnyElement> {
    let enabled = view.monitor.cpu_temperatures_enabled();
    let pending = view.cpu_temperature_request_in_flight;
    Some(
        ramag_ui::clickable_button("system-thermals-cpu-temperature-enable")
            .debug_selector(|| "system-thermals-cpu-temperature-enable".into())
            .small()
            .disabled(pending)
            .label(if enabled {
                if pending {
                    "Disabling CPU temperatures..."
                } else {
                    "Disable CPU temperatures"
                }
            } else {
                if pending {
                    "Enabling CPU temperatures..."
                } else {
                    "Enable CPU temperatures..."
                }
            })
            .tooltip("需要单独授权；只在启用后启动系统帮助程序。")
            .on_click(cx.listener(move |this, _, _, cx| this.set_cpu_temperatures(!enabled, cx)))
            .into_any_element(),
    )
}

#[cfg(not(target_os = "windows"))]
fn cpu_temperature_authorization(
    _view: &SystemView,
    _cx: &mut Context<SystemView>,
) -> Option<AnyElement> {
    None
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

#[cfg(test)]
#[path = "thermals/tests.rs"]
mod tests;
