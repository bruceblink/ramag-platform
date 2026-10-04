use super::{
    ChartPoint, ChartSeries, chart_point, history_chart, latest_point, meter_ratio, range_label,
    time_domain, trace_segments, value_domain,
};
use crate::test_support::TestUnwrapExt;
use gpui_kit::{Context, IntoElement, Render, TestAppContext, Window, hsla, px, size};
use system_pulse_model::{PhysicalUnit, Quantity, ReadingStatus, Sample};

fn sample(at_ms: u64, value: f64) -> Sample {
    Sample::measured(
        at_ms,
        Quantity::Percentage,
        value,
        None,
        PhysicalUnit::Percent,
    )
    .test_unwrap()
}

fn series(samples: Vec<Sample>) -> ChartSeries {
    named_series("CPU", samples)
}

fn named_series(label: &str, samples: Vec<Sample>) -> ChartSeries {
    ChartSeries {
        label: label.into(),
        color: hsla(0.3, 0.8, 0.6, 1.),
        samples,
    }
}

fn measured_sample(
    at_ms: u64,
    quantity: Quantity,
    value: f64,
    total: Option<f64>,
    unit: PhysicalUnit,
) -> Sample {
    Sample::measured(at_ms, quantity, value, total, unit).test_unwrap()
}

type SeriesFactory = fn() -> Vec<ChartSeries>;

fn cpu_series() -> Vec<ChartSeries> {
    vec![series(vec![sample(0, 0.), sample(6_000, 83.)])]
}

fn memory_series() -> Vec<ChartSeries> {
    let gib = 1024_f64.powi(3);
    vec![named_series(
        "Memory",
        vec![
            measured_sample(
                0,
                Quantity::Capacity,
                2. * gib,
                Some(8. * gib),
                PhysicalUnit::Bytes,
            ),
            measured_sample(
                6_000,
                Quantity::Capacity,
                6. * gib,
                Some(8. * gib),
                PhysicalUnit::Bytes,
            ),
        ],
    )]
}

fn temperature_series() -> Vec<ChartSeries> {
    vec![named_series(
        "Thermal",
        vec![
            measured_sample(0, Quantity::Temperature, -30., None, PhysicalUnit::Celsius),
            measured_sample(
                6_000,
                Quantity::Temperature,
                -5.,
                None,
                PhysicalUnit::Celsius,
            ),
        ],
    )]
}

fn network_rate_series() -> Vec<ChartSeries> {
    let pib = 1024_f64.powi(5);
    vec![named_series(
        "Network",
        vec![
            measured_sample(
                0,
                Quantity::Rate,
                2. * pib,
                None,
                PhysicalUnit::BytesPerSecond,
            ),
            measured_sample(
                6_000,
                Quantity::Rate,
                8. * pib,
                None,
                PhysicalUnit::BytesPerSecond,
            ),
        ],
    )]
}

struct ChartPreview {
    series_factory: SeriesFactory,
}

impl Render for ChartPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        history_chart("chart-test", (self.series_factory)(), 120., None, cx)
    }
}

#[gpui_kit::test]
fn range_and_elapsed_align_with_full_width_plot_for_narrow_chart_units(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (view, cx) = cx.add_window_view(|_, _| ChartPreview {
        series_factory: cpu_series,
    });

    for series_factory in [
        cpu_series as SeriesFactory,
        memory_series,
        temperature_series,
        network_rate_series,
    ] {
        view.update(cx, |preview, cx| {
            preview.series_factory = series_factory;
            cx.notify();
        });

        for width in [160.0, 256.0, 640.0] {
            cx.simulate_resize(size(px(width), px(240.)));
            view.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();

            let root = cx.debug_bounds("chart-test").test_unwrap();
            let row = cx.debug_bounds("chart-test:range-row").test_unwrap();
            let range = cx.debug_bounds("chart-test:range-label").test_unwrap();
            let elapsed = cx.debug_bounds("chart-test:elapsed").test_unwrap();
            let plot = cx.debug_bounds("chart-test:plot").test_unwrap();
            assert_eq!(row.origin.x, plot.origin.x);
            assert_eq!(row.right(), plot.right());
            assert_eq!(range.origin.x, plot.origin.x);
            assert!(range.right() <= elapsed.origin.x);
            assert_eq!(elapsed.right(), plot.right());
            assert_eq!(plot.origin.x, root.origin.x);
            assert_eq!(plot.right(), root.right());
            assert!(plot.size.width > px(0.));
        }
    }
}

#[test]
fn uneven_timestamps_align_across_series_without_float_clock_loss() {
    let origin = u64::MAX - 1000;
    let series = vec![
        series(vec![
            sample(origin, 0.),
            sample(origin + 100, 50.),
            sample(origin + 1000, 100.),
        ]),
        series(vec![sample(origin + 500, 25.)]),
    ];
    let time = time_domain(&series);
    assert_eq!(time, (origin, origin + 1000));
    let trace = trace_segments(&series[0].samples, time, (0., 100.));
    assert_eq!(trace[0][1], ChartPoint { x: 0.1, y: 0.5 });
    assert_eq!(
        trace_segments(&series[1].samples, time, (0., 100.))[0][0].x,
        0.5
    );
}

#[test]
fn unavailable_and_nonfinite_samples_break_both_trace_and_area() {
    let mut samples = vec![
        sample(0, 20.),
        sample(100, 40.),
        sample(200, 50.),
        sample(300, 70.),
        sample(400, 80.),
    ];
    samples[1].status = ReadingStatus::Unavailable;
    samples[3].value = Some(f64::NAN);
    let segments = trace_segments(&samples, (0, 400), (0., 100.));
    assert_eq!(
        segments.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 1, 1]
    );
    assert_eq!(segments[1][0].x, 0.5);
}

#[test]
fn stale_latest_never_gets_current_marker() {
    let mut samples = vec![sample(0, 20.), sample(100, 40.)];
    assert!(latest_point(&samples, (0, 100), (0., 100.)).is_some());
    samples[1].status = ReadingStatus::Stale;
    assert!(latest_point(&samples, (0, 100), (0., 100.)).is_none());
    assert_eq!(trace_segments(&samples, (0, 100), (0., 100.))[0].len(), 1);
}

#[test]
fn empty_single_and_zero_domains_stay_finite() {
    assert_eq!(value_domain(&[], None), (0., 1.));
    assert!(trace_segments(&[], (0, 0), (0., 1.)).is_empty());
    let series = vec![series(vec![sample(42, 0.)])];
    let range = value_domain(&series, None);
    assert_eq!(range, (0., 1.));
    assert_eq!(
        trace_segments(&series[0].samples, (42, 42), range)[0][0],
        ChartPoint { x: 1., y: 1. }
    );
    assert_eq!(value_domain(&series, Some((f64::NAN, 0.))), range);
}

#[test]
fn shared_range_covers_every_series_and_keeps_temperature_negative() {
    assert_eq!(
        value_domain(
            &[series(vec![sample(0, 20.)]), series(vec![sample(0, 80.)])],
            None
        ),
        (0., 80.)
    );
    let cold =
        Sample::measured(0, Quantity::Temperature, -5., None, PhysicalUnit::Celsius).test_unwrap();
    let range = value_domain(&[series(vec![cold])], None);
    assert!(range.0 < -5. && range.1 > -5.);
}

#[test]
fn ratios_clamp_but_unavailable_and_nonfinite_do_not_light_cells() {
    assert_eq!(meter_ratio(None), None);
    assert_eq!(meter_ratio(Some(f64::NAN)), None);
    assert_eq!(meter_ratio(Some(f64::INFINITY)), None);
    assert_eq!(meter_ratio(Some(-1.)), Some(0.));
    assert_eq!(meter_ratio(Some(0.)), Some(0.));
    assert_eq!(meter_ratio(Some(0.5)), Some(0.5));
    assert_eq!(meter_ratio(Some(2.)), Some(1.));
}

#[test]
fn range_labels_convert_base_values_without_relabelling_bytes_as_gibibytes() {
    assert_eq!(
        range_label((0., 2. * 1024_f64.powi(3)), "GiB"),
        "Scale: 0.0–2.0 GiB"
    );
    assert_eq!(
        range_label((0., 3. * 1024_f64.powi(2)), "MiB/s"),
        "Scale: 0.0–3.0 MiB/s"
    );
    assert_eq!(
        range_label((0., 8. * 1024_f64.powi(5)), "PiB/s"),
        "Scale: 0.0–8.0 PiB/s"
    );
    assert_eq!(range_label((0., 4e9), "GHz"), "Scale: 0.0–4.0 GHz");
    assert_eq!(range_label((-5., 20.), "°C"), "Scale: -5.0–20.0 °C");
    assert_eq!(range_label((0., 100.), "%"), "Scale: 0.0–100.0 %");
}

#[test]
fn out_of_order_samples_cannot_draw_backwards_or_claim_latest_marker() {
    let samples = vec![sample(100, 10.), sample(300, 30.), sample(200, 20.)];
    let segments = trace_segments(&samples, (100, 300), (0., 100.));
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].len(), 2);
    assert!(latest_point(&samples, (100, 300), (0., 100.)).is_none());
}

#[test]
fn finite_extremes_cannot_overflow_chart_coordinates() {
    let point = chart_point(&sample(100, 0.), (0, 100), (-f64::MAX, f64::MAX)).test_unwrap();
    assert_eq!(point, ChartPoint { x: 1., y: 0.5 });
    for value in [-f64::MAX, f64::MAX] {
        let extreme =
            Sample::measured(0, Quantity::Temperature, value, None, PhysicalUnit::Celsius)
                .test_unwrap();
        let series = vec![series(vec![extreme])];
        let range = value_domain(&series, None);
        assert!(range.0.is_finite() && range.1.is_finite() && range.0 < range.1);
        assert!(
            trace_segments(&series[0].samples, (0, 0), range)[0][0]
                .y
                .is_finite()
        );
    }
}
