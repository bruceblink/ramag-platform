use crate::{
    screen_charts::history_chart,
    screen_data::Channel,
    screen_pages::{hero_meter, metric, series, stat},
    screen_style::{accent, heading, palette, section},
    workspace::Data,
};
use gpui_kit::{prelude::FluentBuilder, *};
use system_pulse_model::{PhysicalUnit, Quantity, Screen};

/// Keep the disk page's capacity, backing-device throughput, and auxiliary
/// counters in separate sections so each value has one clear visual owner.
pub(crate) fn disk(
    rows: &[Channel],
    capacity: Option<&Channel>,
    rate_rows: &[&Channel],
    state: &Data,
    color: Hsla,
    cx: &App,
) -> AnyElement {
    let auxiliary: Vec<_> = rows
        .iter()
        .filter(|channel| {
            channel.quantity != Quantity::Capacity && channel.unit != PhysicalUnit::BytesPerSecond
        })
        .collect();
    let secondary_color = accent(Screen::Thermals, cx);
    let trend = device_trend(
        "device-trend:disks",
        "Read / write throughput",
        "device-history:disks",
        rate_rows,
        state,
        [color, secondary_color],
        cx,
    );
    let stats = device_stats(
        "device-stats:disks",
        "Backing device details",
        &auxiliary,
        state,
        cx,
    );
    div()
        .flex()
        .flex_col()
        .gap_4()
        .when_some(capacity, |view, capacity| {
            view.child(
                div()
                    .id("device-primary:disks")
                    .debug_selector(|| "device-primary:disks".into())
                    .child(hero_meter(capacity, state, Screen::Disks, cx)),
            )
        })
        .child(trend)
        .child(stats)
        .child(
            div()
                .id("device-note:disks")
                .debug_selector(|| "device-note:disks".into())
                .text_size(px(12.))
                .text_color(palette(cx).muted)
                .child("Capacity describes this filesystem. Transfer rates describe its backing block device."),
        )
        .into_any_element()
}

/// Keep network rates together while separating cumulative counters and
/// connection counts from the live throughput trend.
pub(crate) fn network(
    rows: &[Channel],
    rate_rows: &[&Channel],
    state: &Data,
    color: Hsla,
    cx: &App,
) -> AnyElement {
    let counters: Vec<_> = rows
        .iter()
        .filter(|channel| channel.quantity == Quantity::Counter)
        .collect();
    let secondary_color = accent(Screen::Thermals, cx);
    div()
        .flex()
        .flex_col()
        .gap_4()
        .when_some(rate_rows.first().copied(), |view, primary| {
            view.child(
                div()
                    .id("device-primary:network")
                    .debug_selector(|| "device-primary:network".into())
                    .child(hero_meter(primary, state, Screen::Network, cx)),
            )
        })
        .child(device_trend(
            "device-trend:network",
            "Receive / transmit throughput",
            "device-history:network",
            rate_rows,
            state,
            [color, secondary_color],
            cx,
        ))
        .child(device_stats(
            "device-stats:network",
            "Cumulative traffic",
            &counters,
            state,
            cx,
        ))
        .into_any_element()
}

fn device_trend(
    selector: &str,
    title: &str,
    chart_selector: &str,
    rows: &[&Channel],
    state: &Data,
    colors: [Hsla; 2],
    cx: &App,
) -> AnyElement {
    let selector = selector.to_owned();
    let chart_selector = chart_selector.to_owned();
    section(cx)
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .child(heading(title, 19., cx))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_4()
                .children(rows.iter().enumerate().map(|(index, channel)| {
                    metric(
                        format!("rate:{}", channel.sensor),
                        channel.label.clone(),
                        channel.value(state),
                        colors[index.min(1)],
                        cx,
                    )
                })),
        )
        .child(history_chart(
            chart_selector,
            rows.iter()
                .enumerate()
                .map(|(index, channel)| series(channel, state, colors[index.min(1)]))
                .collect(),
            345.,
            None,
            cx,
        ))
        .into_any_element()
}

fn device_stats(
    selector: &str,
    title: &str,
    rows: &[&Channel],
    state: &Data,
    cx: &App,
) -> AnyElement {
    let selector = selector.to_owned();
    let debug_selector = selector.clone();
    let grid_selector = format!("{selector}:grid");
    section(cx)
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || debug_selector.clone())
        .child(heading(title, 19., cx))
        .child(
            div()
                .id(SharedString::from(grid_selector))
                .flex()
                .flex_wrap()
                .gap_4()
                .children(rows.iter().map(|channel| stat(channel, state, cx))),
        )
        .into_any_element()
}
