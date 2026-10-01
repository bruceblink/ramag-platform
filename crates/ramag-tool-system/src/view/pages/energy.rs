//! Selected energy sensor composition for the Energy page.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Styled, div,
    px,
};
use ramag_infra_system::SensorDescriptor;
use ramag_ui::PointerDropdownMenu as _;

use super::super::{SystemView, helpers};
use crate::{MonitorSnapshot, ReadingStatus, SensorSample};

/// Builds a stable sensor dropdown and saves the selected sensor identity.
pub(super) fn energy_sensor_selector(
    readings: &[&SensorDescriptor],
    saved_id: Option<&str>,
    selected_id: Option<&str>,
    view: Entity<SystemView>,
) -> AnyElement {
    let choices = readings
        .iter()
        .map(|sensor| (sensor.id.clone(), helpers::sensor_title(sensor)))
        .collect::<Vec<_>>();
    let label = selected_id
        .and_then(|id| choices.iter().find(|(sensor, _)| sensor == id))
        .map(|(_, title)| title.clone())
        .unwrap_or_else(|| {
            saved_id
                .map(|id| format!("不可用 · {id}"))
                .unwrap_or_else(|| "选择功率传感器".into())
        });
    let selected_id = selected_id.map(str::to_owned);
    ramag_ui::clickable_button("system-energy-sensor-selector")
        .debug_selector(|| "system-energy-sensor-selector".into())
        .small()
        .max_w(px(360.0))
        .label(label)
        .dropdown_caret(true)
        .pointer_dropdown_menu(move |mut menu, _, _| {
            for (id, title) in &choices {
                let id = id.clone();
                let option_id = sensor_selector_id(&id);
                let target = view.clone();
                menu = menu.item(
                    gpui_kit::component::menu::PopupMenuItem::element({
                        let title = title.clone();
                        let option_id = option_id.clone();
                        move |_, _| {
                            let selector = format!("system-energy-sensor-option-{option_id}");
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
                                .insert("energy".into(), id.clone());
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

fn sensor_selector_id(id: &str) -> String {
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

/// Maps a sample to the value and status that may be shown as live data.
/// A stalled collection invalidates the live value while keeping its history
/// available to the chart for context.
pub(crate) fn displayed_sensor_state(
    snapshot: &MonitorSnapshot,
    sample: Option<&SensorSample>,
) -> (ReadingStatus, Option<f64>) {
    let mut status = sample.map_or(ReadingStatus::Unavailable, |sample| sample.status);
    if snapshot.collection_stale && status == ReadingStatus::Current && sample.is_some() {
        status = ReadingStatus::Stale;
    }
    let current = (!snapshot.collection_stale)
        .then(|| sample.and_then(SensorSample::chart_value))
        .flatten();
    if status == ReadingStatus::Current && current.is_none() {
        status = ReadingStatus::Failed;
    }
    (status, current)
}

pub(super) fn energy_primary(
    descriptor: &SensorDescriptor,
    snapshot: &MonitorSnapshot,
    maximum_gap: f64,
    cx: &Context<SystemView>,
) -> AnyElement {
    let sample = snapshot.latest(&descriptor.id);
    let (status, current) = displayed_sensor_state(snapshot, sample);
    let value = current.map_or_else(
        || status.label().to_owned(),
        |value| helpers::format_value(value, &descriptor.unit),
    );
    let scale = super::devices::chart_max(descriptor, snapshot);
    let fill = current
        .map(|value| (value.max(0.0) / scale).clamp(0.0, 1.0) as f32)
        .unwrap_or(0.0);
    let points = helpers::chart_points(snapshot, &descriptor.id, maximum_gap);
    let note = if status == crate::ReadingStatus::Current {
        "meter follows the observed chart range".to_owned()
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
    ramag_ui::pulse_ui::pulse_panel(cx)
        .debug_selector(|| "system-energy-primary-meter".into())
        .w_full()
        .min_w_0()
        .border_color(cx.theme().warning.opacity(0.42))
        .bg(cx.theme().warning.opacity(0.04))
        .gap(px(8.0))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .items_baseline()
                .justify_between()
                .gap(px(8.0))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .child(helpers::sensor_title(descriptor)),
                        )
                        .child(
                            div()
                                .debug_selector(|| "system-energy-domain".into())
                                .w_full()
                                .min_w_0()
                                .whitespace_normal()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(domain),
                        ),
                )
                .child(
                    div()
                        .debug_selector(|| "system-energy-primary-value".into())
                        .text_2xl()
                        .text_color(cx.theme().warning)
                        .child(value),
                ),
        )
        .child(
            h_flex()
                .debug_selector(|| "system-energy-primary-meter-bar".into())
                .w_full()
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
            helpers::render_chart(
                &points,
                scale,
                &descriptor.unit,
                px(220.0),
                cx.theme().warning,
                cx,
            )
            .debug_selector(|| "system-energy-primary-chart".into()),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(note),
        )
        .into_any_element()
}
