use super::*;
use gpui_kit::component::{Root, v_flex};
use gpui_kit::{AppContext as _, Context, IntoElement, Render, TestAppContext, Window, px, size};

macro_rules! required {
    ($value:expr, $message:literal) => {{
        let value = $value;
        assert!(value.is_some(), $message);
        match value {
            Some(value) => value,
            None => unreachable!("assertion above guarantees a value"),
        }
    }};
}

#[test]
fn chart_axis_labels_match_the_physical_scale_and_zero_baseline() {
    assert_eq!(chart_axis_maximum(100.0), 100.0);
    assert_eq!(
        format_value(chart_axis_maximum(100.0), &Unit::Percent),
        "100.0 %"
    );
    assert_eq!(
        format_value(chart_axis_maximum(47.9 * 1024.0_f64.powi(3)), &Unit::Bytes),
        "47.9 GiB"
    );
    assert_eq!(format_value(0.0, &Unit::Bytes), "0.0 B");
    assert_eq!(format_value(0.0, &Unit::Percent), "0.0 %");
    assert_eq!(format_value(1024.0, &Unit::BytesPerSecond), "1.0 KiB/s");
    assert_eq!(
        format_value(chart_axis_maximum(f64::NAN), &Unit::Bytes),
        "1.0 B"
    );
    assert_eq!(format_value(chart_axis_maximum(0.0), &Unit::Bytes), "1.0 B");
    for invalid in [-1.0, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(chart_axis_maximum(invalid), 1.0);
    }
}

struct ChartAxisTestHost {
    height: gpui_kit::Pixels,
    unit: Unit,
    maximum: f64,
}

impl Render for ChartAxisTestHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let value = self.maximum * 0.4;
        let points = [
            ramag_ui::pulse_ui::ChartPoint {
                at_seconds: 0.0,
                value: Some(value * 0.8),
            },
            ramag_ui::pulse_ui::ChartPoint {
                at_seconds: 59.0,
                value: Some(value),
            },
        ];
        v_flex().size_full().child(
            v_flex()
                .debug_selector(|| "system-chart-axis-test-host".into())
                .w(px(256.0))
                .h(px(180.0))
                .min_w_0()
                .p(px(8.0))
                .child(render_chart(
                    &points,
                    self.maximum,
                    &self.unit,
                    self.height,
                    cx.theme().accent,
                    cx,
                )),
        )
    }
}

/// Checks that axis labels fit beside the plot and the elapsed-time labels begin at its edge.
fn assert_chart_layout(visual: &mut gpui_kit::gpui::VisualTestContext, expected_height: f32) {
    let host = required!(
        visual.debug_bounds("system-chart-axis-test-host"),
        "chart test host should render"
    );
    let axis = required!(
        visual.debug_bounds("system-chart-axis"),
        "vertical axis should render"
    );
    let maximum = required!(
        visual.debug_bounds("system-chart-axis-max"),
        "maximum tick should render"
    );
    let minimum = required!(
        visual.debug_bounds("system-chart-axis-min"),
        "zero tick should render"
    );
    let plot = required!(
        visual.debug_bounds("pulse-time-chart"),
        "time-series plot should render"
    );
    let time_range = required!(
        visual.debug_bounds("system-chart-time-range"),
        "time labels should render"
    );

    assert_eq!(f32::from(axis.size.height), expected_height);
    assert_eq!(f32::from(plot.size.height), expected_height);
    assert_eq!(axis.top(), plot.top());
    assert_eq!(axis.bottom(), plot.bottom());
    assert!(maximum.origin.y < minimum.origin.y);
    assert!(maximum.top() >= axis.top() && maximum.bottom() <= minimum.top());
    assert!(
        minimum.bottom() <= axis.bottom(),
        "ticks must stay inside the axis: axis={axis:?}, maximum={maximum:?}, minimum={minimum:?}"
    );
    assert!((f32::from(maximum.origin.x) - f32::from(axis.origin.x)).abs() <= 1.0);
    assert!((f32::from(minimum.origin.x) - f32::from(axis.origin.x)).abs() <= 1.0);
    assert!(maximum.origin.x >= axis.origin.x);
    assert!(maximum.origin.x + maximum.size.width <= axis.origin.x + axis.size.width);
    assert!(minimum.origin.x >= axis.origin.x);
    assert!(minimum.origin.x + minimum.size.width <= axis.origin.x + axis.size.width);
    assert!(axis.origin.x + axis.size.width <= plot.origin.x);
    assert!(f32::from(axis.size.width) <= 70.0);
    assert!((f32::from(plot.origin.x) - f32::from(axis.origin.x)) <= 70.0);
    assert!((f32::from(time_range.origin.x) - f32::from(plot.origin.x)).abs() <= 1.0);
    assert!(
        (f32::from(time_range.origin.x + time_range.size.width)
            - f32::from(plot.origin.x + plot.size.width))
        .abs()
            <= 1.0
    );
    assert!(plot.origin.x + plot.size.width <= host.origin.x + host.size.width);
}

macro_rules! run_chart_layout_matrix {
    ($visual:ident, $height:expr) => {
        for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
            $visual.update(|_, app| ramag_ui::apply_theme(mode, app));
            for text_size in ramag_ui::InterfaceTextSize::ALL {
                $visual.update(|_, app| {
                    ramag_ui::set_system_settings(
                        ramag_ui::SystemSettings {
                            text_size,
                            ..Default::default()
                        },
                        app,
                    );
                });
                for (width, window_height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
                    $visual.simulate_resize(size(px(width), px(window_height)));
                    $visual.run_until_parked();
                    assert_chart_layout($visual, $height);
                }
            }
        }
    };
}

#[gpui_kit::test]
fn chart_axis_and_footer_align_at_narrow_width_and_all_display_settings(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    {
        let (_, visual) = cx.add_window_view(|window, cx| {
            let host = cx.new(|_| ChartAxisTestHost {
                height: px(54.0),
                unit: Unit::Percent,
                maximum: 100.0,
            });
            Root::new(host, window, cx)
        });
        run_chart_layout_matrix!(visual, 54.0);
    }

    {
        let (_, visual) = cx.add_window_view(|window, cx| {
            let host = cx.new(|_| ChartAxisTestHost {
                height: px(116.0),
                unit: Unit::Bytes,
                maximum: 47.9 * 1024.0_f64.powi(3),
            });
            Root::new(host, window, cx)
        });
        run_chart_layout_matrix!(visual, 116.0);
    }

    {
        let (_, visual) = cx.add_window_view(|window, cx| {
            let host = cx.new(|_| ChartAxisTestHost {
                height: px(54.0),
                unit: Unit::BytesPerSecond,
                maximum: 999.9 * 1024.0_f64.powi(3),
            });
            Root::new(host, window, cx)
        });
        run_chart_layout_matrix!(visual, 54.0);
    }

    {
        let (_, visual) = cx.add_window_view(|window, cx| {
            let host = cx.new(|_| ChartAxisTestHost {
                height: px(54.0),
                unit: Unit::CountPerSecond,
                maximum: 10_000.0,
            });
            Root::new(host, window, cx)
        });
        run_chart_layout_matrix!(visual, 54.0);
    }
}
