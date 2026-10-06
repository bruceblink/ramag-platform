use super::{chart, hero_meter, stat};
use crate::{
    screen_data as data,
    screen_style::{accent, empty, heading, palette, section},
    workspace::Data,
};
use gpui_kit::component::{ActiveTheme, tooltip::Tooltip};
use gpui_kit::{prelude::FluentBuilder, *};
use system_pulse_model::{PhysicalUnit, Quantity, Screen};

fn unavailable_selection(channel: &data::Channel, state: &Data, cx: &App) -> AnyElement {
    let screen = if channel.quantity == Quantity::Power {
        Screen::Energy
    } else {
        Screen::Thermals
    };
    let color = accent(screen, cx);
    let detail = data::unavailable_detail(channel, state)
        .unwrap_or_else(|| format!("{} · {} · Unavailable", channel.device, channel.label));
    let tooltip = detail.clone();
    let has_history = channel
        .samples(state)
        .iter()
        .any(|sample| sample.chart_value().is_some());
    section(cx)
        .id("selected-channel-unavailable")
        .debug_selector(|| "selected-channel-unavailable".into())
        .role(Role::Group)
        .aria_label(format!("Selected sensor unavailable: {detail}"))
        .child(heading("Selected sensor unavailable", 18., cx))
        .child(
            div()
                .text_sm()
                .text_color(palette(cx).muted)
                .child(format!("{} · {}", channel.device, channel.label)),
        )
        .child(
            crate::meters::metric_text(
                "selected-channel-unavailable-value".into(),
                detail,
                "Unavailable".into(),
            )
            .font_family(cx.theme().mono_font_family.clone())
            .text_color(color)
            .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx)),
        )
        .when(has_history, |view| {
            view.child(
                div()
                    .id("selected-channel-history")
                    .debug_selector(|| "selected-channel-history".into())
                    .child(chart(channel, state, color, 260., cx)),
            )
        })
        .into_any_element()
}

pub(super) fn render(screen: Screen, state: &Data, width: f32, cx: &App) -> AnyElement {
    let (quantity, unit) = if screen == Screen::Energy {
        (Quantity::Power, PhysicalUnit::Watts)
    } else {
        (Quantity::Temperature, PhysicalUnit::Celsius)
    };
    let rows = data::environmental_channels(state, quantity, unit);
    let selected = data::selected_channel(state, screen);
    let unavailable = data::selected_environmental_channel(state, screen)
        .filter(|channel| data::unavailable_detail(channel, state).is_some());
    if rows.is_empty() {
        if let Some(channel) = unavailable.as_ref() {
            return unavailable_selection(channel, state, cx);
        }
        return empty(
            if screen == Screen::Energy {
                "No power measurement available"
            } else {
                "No temperature measurement available"
            },
            "This screen uses measured sensor data. Choose an available sensor when one is reported.",
            cx,
        );
    }
    let hottest = (screen == Screen::Thermals)
        .then(|| data::highest_current(state, &rows))
        .flatten();
    let has_unavailable_selection = unavailable.is_some();
    let color = accent(screen, cx);
    let grid_width = if width >= 1100. {
        (width - 24.) / 3.
    } else {
        (width - 12.) / 2.
    };

    // Keep the all-sensor maximum in its own card; the card below owns the selected sensor's meter and chart.
    div()
        .flex()
        .flex_col()
        .gap_4()
        .when_some(hottest, |view, hottest| {
            let hottest_source = format!("{} · {}", hottest.device, hottest.label);
            view.child(
                section(cx)
                    .id("thermal-hottest-panel")
                    .debug_selector(|| "thermal-hottest-panel".into())
                    .role(Role::Group)
                    .aria_label(format!("Hottest current sensor: {hottest_source}"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(palette(cx).muted)
                                            .child("Hottest current sensor"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(palette(cx).muted)
                                            .child(hottest_source),
                                    ),
                            )
                            .child(
                                crate::meters::metric_label(
                                    "thermal-hottest".into(),
                                    hottest.value(state),
                                )
                                .debug_selector(|| "thermal-hottest".into())
                                .text_size(px(19.))
                                .text_color(color)
                                .font_family(cx.theme().mono_font_family.clone()),
                            ),
                    ),
            )
        })
        .when_some(unavailable, |view, channel| {
            view.child(unavailable_selection(&channel, state, cx))
        })
        .when(selected.is_none() && !has_unavailable_selection, |view| {
            view.child(empty(
                "Selected sensor unavailable",
                "Its saved identity is preserved. Choose an available sensor above.",
                cx,
            ))
        })
        .when_some(selected, |view, selected| {
            let selected_source = format!("{} · {}", selected.device, selected.label);
            let selected_history = section(cx)
                .child(heading(selected.label.clone(), 20., cx))
                .child(
                    div()
                        .text_sm()
                        .text_color(palette(cx).muted)
                        .child(selected.device.clone()),
                )
                .child(
                    div()
                        .id("selected-channel-history")
                        .debug_selector(|| "selected-channel-history".into())
                        .child(chart(&selected, state, color, 260., cx)),
                )
                .when(!selected.scope.is_empty(), |view| {
                    view.child(
                        div()
                            .text_size(px(12.))
                            .text_color(palette(cx).muted)
                            .child(selected.scope.clone()),
                    )
                });
            if screen == Screen::Thermals {
                view.child(
                    section(cx)
                        .id("selected-channel-panel")
                        .debug_selector(|| "selected-channel-panel".into())
                        .role(Role::Group)
                        .aria_label(format!("Selected sensor: {selected_source}"))
                        .child(heading("Selected sensor", 18., cx))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette(cx).muted)
                                .child(selected_source),
                        )
                        .child(hero_meter(&selected, state, screen, cx)),
                )
                .child(selected_history)
            } else {
                view.child(hero_meter(&selected, state, screen, cx))
                    .child(selected_history)
            }
        })
        .child(heading(
            if screen == Screen::Thermals {
                "Temperature sensors"
            } else {
                "Measured power channels"
            },
            24.,
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .children(rows.iter().map(|channel| {
                    section(cx)
                        .w(px(grid_width))
                        .flex_none()
                        .border_color(color.opacity(0.4))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette(cx).muted)
                                .child(channel.device.clone()),
                        )
                        .child(stat(channel, state, cx))
                        .when_some(
                            channel
                                .latest(state)
                                .and_then(|sample| sample.reason.clone()),
                            |view, reason| {
                                let id = format!("sensor-availability:{}", channel.sensor);
                                view.child(
                                    div()
                                        .debug_selector(move || id.clone())
                                        .text_sm()
                                        .text_color(palette(cx).muted)
                                        .child(reason),
                                )
                            },
                        )
                        .child(chart(channel, state, color, 95., cx))
                })),
        )
        .into_any_element()
}
