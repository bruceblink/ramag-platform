//! Sampling and sensor visibility preferences live in the monitor-only settings page.

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, button::ButtonVariants as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, div, px,
};

use super::super::{SystemSection, SystemView, helpers};
use crate::{MonitorSnapshot, RefreshInterval};

impl SystemView {
    pub(super) fn render_settings(
        &mut self,
        snapshot: &MonitorSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let mut rates = h_flex().w_full().flex_wrap().gap(px(7.0));
        for interval in [
            RefreshInterval::OneSecond,
            RefreshInterval::TwoSeconds,
            RefreshInterval::FiveSeconds,
        ] {
            let selected = self.monitor.refresh_interval() == interval;
            let button = ramag_ui::clickable_button(format!("system-refresh-{}", interval.label()))
                .debug_selector(|| format!("system-refresh-{}", interval.label()))
                .xsmall()
                .label(interval.label());
            rates = rates.child(
                if selected {
                    button.primary()
                } else {
                    button.ghost()
                }
                .on_click(
                    cx.listener(move |this, _, _, cx| this.set_refresh_interval(interval, cx)),
                ),
            );
        }

        let mut sensors = snapshot.host.sensors.iter().collect::<Vec<_>>();
        sensors.sort_by(|left, right| {
            left.monitor_id
                .cmp(&right.monitor_id)
                .then(left.title.cmp(&right.title))
        });
        let sensor_count = sensors.len();
        let view = cx.entity().clone();
        let mut sensor_rows = v_flex().w_full().gap(px(5.0));
        for sensor in sensors {
            let id = sensor.id.clone();
            let selector = selector_id(&id);
            let hidden = self.presentation.hidden_sensors.contains(&id);
            let label = if sensor.scope.is_empty() {
                sensor.title.clone()
            } else {
                format!("{} · {}", sensor.title, sensor.scope)
            };
            let unit = helpers::unit_label(&sensor.unit);
            sensor_rows = sensor_rows.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(div().text_sm().child(label))
                            .child(div().text_xs().text_color(theme.muted_foreground).child(
                                format!(
                                    "{} · {}",
                                    monitor_title(snapshot, &sensor.monitor_id),
                                    unit
                                ),
                            )),
                    )
                    .child(
                        ramag_ui::clickable_checkbox(format!("system-sensor-visible-{}", id))
                            .debug_selector(move || format!("system-sensor-visible-{selector}"))
                            .checked(!hidden)
                            .on_click({
                                let view = view.clone();
                                let id = sensor.id.clone();
                                move |checked, _, app| {
                                    view.update(app, |this, cx| {
                                        if *checked {
                                            this.presentation.hidden_sensors.remove(&id);
                                        } else {
                                            this.presentation.hidden_sensors.insert(id.clone());
                                        }
                                        this.save_presentation(cx);
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            );
        }
        if sensor_count == 0 {
            sensor_rows = sensor_rows.child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Unavailable,
                "采样器尚未发现任何传感器。",
                cx,
            ));
        }
        let mut service_state = v_flex()
            .gap(px(6.0))
            .child(div().text_sm().child(format!(
                "{} 个设备 · {} 个传感器",
                snapshot.host.monitors.len(),
                snapshot.host.sensors.len()
            )))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!(
                        "{} 条采集诊断信息",
                        snapshot.host.diagnostics.len()
                    )),
            );
        if let Some(error) = &snapshot.collection_error {
            service_state = service_state.child(ramag_ui::pulse_ui::pulse_status_notice(
                ramag_ui::pulse_ui::PulseStatus::Failed,
                error.clone(),
                cx,
            ));
        }
        for diagnostic in &snapshot.host.diagnostics {
            service_state = service_state.child(ramag_ui::pulse_ui::pulse_status_notice(
                match diagnostic.availability {
                    ramag_infra_system::Availability::Available => {
                        ramag_ui::pulse_ui::PulseStatus::Current
                    }
                    ramag_infra_system::Availability::WarmingUp => {
                        ramag_ui::pulse_ui::PulseStatus::Warming
                    }
                    ramag_infra_system::Availability::Unavailable => {
                        ramag_ui::pulse_ui::PulseStatus::Unavailable
                    }
                    ramag_infra_system::Availability::Failed => {
                        ramag_ui::pulse_ui::PulseStatus::Failed
                    }
                },
                format!("{}：{}", diagnostic.backend, diagnostic.reason),
                cx,
            ));
        }

        #[cfg(target_os = "windows")]
        let thermal_opt_in = {
            let enabled = self.monitor.cpu_temperatures_enabled();
            let view = cx.entity().clone();
            Some(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(div().text_sm().child("CPU 温度采集"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("需要单独授权；只在你启用后启动系统帮助程序。"),
                            ),
                    )
                    .child(
                        ramag_ui::clickable_switch("system-cpu-thermal-enable")
                            .checked(enabled)
                            .on_click(move |checked, _, app| {
                                view.update(app, |this, cx| {
                                    this.set_cpu_temperatures(*checked, cx)
                                });
                            }),
                    )
                    .into_any_element(),
            )
        };
        #[cfg(not(target_os = "windows"))]
        let thermal_opt_in: Option<AnyElement> = None;
        v_flex()
            .debug_selector(|| "system-page-settings".into())
            .w_full()
            .min_w_0()
            .gap(px(14.0))
            .p(px(20.0))
            .child(helpers::page_title(
                SystemSection::Settings,
                "采样频率、传感器显示与采集状态",
                cx,
            ))
            .child(
                ramag_ui::pulse_ui::pulse_panel(cx).child(
                    v_flex()
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .child("采样频率"),
                        )
                        .child(rates),
                ),
            )
            .when_some(thermal_opt_in, |page, toggle| {
                page.child(
                    ramag_ui::pulse_ui::pulse_panel(cx).child(
                        v_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                    .child("温度采集"),
                            )
                            .child(toggle),
                    ),
                )
            })
            .child(
                ramag_ui::pulse_ui::pulse_panel(cx).child(
                    v_flex()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .child("采集器状态"),
                        )
                        .child(service_state),
                ),
            )
            .child(
                ramag_ui::pulse_ui::pulse_panel(cx).child(
                    v_flex()
                        .w_full()
                        .gap(px(9.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .child("传感器可见性"),
                        )
                        .child(sensor_rows),
                ),
            )
            .child(
                ramag_ui::clickable_button("system-settings-global")
                    .label("打开全局系统工具设置")
                    .on_click(|_, window, cx| {
                        window.dispatch_action(
                            Box::new(ramag_ui::actions::OpenToolSettings {
                                tool_id: "system".into(),
                            }),
                            cx,
                        )
                    }),
            )
            .into_any_element()
    }
}

fn monitor_title<'a>(snapshot: &'a MonitorSnapshot, monitor_id: &str) -> &'a str {
    snapshot
        .host
        .monitors
        .iter()
        .find(|monitor| monitor.id == monitor_id)
        .map_or("主机", |monitor| monitor.title.as_str())
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
