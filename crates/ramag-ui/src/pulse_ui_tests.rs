use super::*;
use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, Context, IntoElement, Modifiers, MouseButton, Render, TestAppContext,
    VisualTestContext, px, size,
};

struct PulseComponentsHost {
    selections: Rc<RefCell<Vec<String>>>,
}

impl Render for PulseComponentsHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = [
            PulseTab {
                id: "summary".into(),
                title: "概览".into(),
            },
            PulseTab {
                id: "cpu".into(),
                title: "CPU".into(),
            },
            PulseTab {
                id: "memory".into(),
                title: "内存".into(),
            },
        ];
        let devices = [
            ("gpu-0".into(), "GPU 0".into()),
            (
                "gpu/1:long-id".into(),
                "GPU 1, high performance telemetry adapter".into(),
            ),
        ];
        let samples = [
            ChartPoint {
                at_seconds: 10.0,
                value: Some(14.0),
            },
            ChartPoint {
                at_seconds: 12.0,
                value: Some(37.0),
            },
            ChartPoint {
                at_seconds: 14.0,
                value: None,
            },
            ChartPoint {
                at_seconds: 20.0,
                value: Some(67.0),
            },
        ];
        let selections_for_tab = self.selections.clone();
        let selections_for_device = self.selections.clone();
        v_flex()
            .debug_selector(|| "pulse-test-root".into())
            .size_full()
            .min_w_0()
            .gap(px(12.0))
            .p(px(12.0))
            .child(pulse_page_title("系统监控", Some("硬件状态与历史趋势"), cx))
            .child(pulse_tabs(&tabs, "summary", window, cx, move |id, _, _| {
                selections_for_tab.borrow_mut().push(id.to_string());
            }))
            .child(pulse_device_selector(
                &devices,
                Some("gpu-0"),
                cx,
                move |id, _, _| {
                    selections_for_device.borrow_mut().push(id.to_string());
                },
            ))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(pulse_metric_card(
                        "处理器",
                        "34",
                        Some("%"),
                        Some("8 核"),
                        cx,
                    ))
                    .child(pulse_metric_card(
                        "内存",
                        "12.4",
                        Some("GB"),
                        None::<&str>,
                        cx,
                    )),
            )
            .child(pulse_status_notice(
                PulseStatus::Stale,
                "最近一次读数已过期",
                cx,
            ))
            .child(pulse_panel(cx).child(pulse_time_chart(&samples, 100.0, px(120.0), cx)))
    }
}

/// 测试组件在亮暗主题和窄宽窗口中的实际布局边界。
#[gpui_kit::test]
fn pulse_components_fit_compact_and_wide_windows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let selections = Rc::new(RefCell::new(Vec::new()));
    let selections_for_view = selections.clone();
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| PulseComponentsHost {
            selections: selections_for_view,
        });
        Root::new(view, window, cx)
    });

    for mode in [crate::Mode::Light, crate::Mode::Dark] {
        visual.update(|_, app| crate::apply_theme(mode, app));
        for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
            visual.simulate_resize(size(px(width), px(height)));
            visual.run_until_parked();
            for selector in [
                "pulse-test-root",
                "pulse-page-title",
                "pulse-tabs",
                "pulse-tab-0",
                "pulse-device-selector",
                "pulse-device-0",
                "pulse-device-1",
                "pulse-metric-card",
                "pulse-status-notice",
                "pulse-time-chart",
            ] {
                let bounds = visual.debug_bounds(selector);
                assert!(
                    bounds.is_some(),
                    "missing {selector} at {width}x{height} in {mode:?}"
                );
                let Some(bounds) = bounds else { unreachable!() };
                assert!(bounds.left() >= px(0.0), "{selector}: {bounds:?}");
                assert!(bounds.right() <= px(width), "{selector}: {bounds:?}");
                assert!(bounds.bottom() <= px(height), "{selector}: {bounds:?}");
            }
        }
    }

    click(visual, "pulse-tab-1");
    click(visual, "pulse-device-1");
    assert_eq!(&*selections.borrow(), &["cpu", "gpu/1:long-id"]);
}

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual.debug_bounds(selector);
    assert!(bounds.is_some(), "missing control {selector}");
    let Some(bounds) = bounds else { unreachable!() };
    let center = bounds.center();
    visual.simulate_mouse_move(center, None, Modifiers::default());
    visual.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
    visual.simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
}

#[test]
fn time_chart_keeps_only_the_latest_120_samples() {
    let samples = (0..150)
        .map(|index| ChartPoint {
            at_seconds: f64::from(index),
            value: Some(f64::from(index)),
        })
        .collect::<Vec<_>>();
    let rendered = bounded_chart_points(&samples);
    assert_eq!(rendered.len(), 120);
    assert_eq!(rendered.first().map(|point| point.at_seconds), Some(30.0));
    assert_eq!(rendered.last().map(|point| point.at_seconds), Some(149.0));
}

#[test]
fn chart_uses_elapsed_time_and_breaks_across_missing_samples() {
    let points = [
        ChartPoint {
            at_seconds: 10.0,
            value: Some(10.0),
        },
        ChartPoint {
            at_seconds: 20.0,
            value: Some(30.0),
        },
        ChartPoint {
            at_seconds: 30.0,
            value: None,
        },
        ChartPoint {
            at_seconds: 50.0,
            value: Some(80.0),
        },
        ChartPoint {
            at_seconds: 60.0,
            value: Some(90.0),
        },
    ];
    let segments = normalized_chart_segments(&points, 100.0);
    assert_eq!(segments.len(), 2);
    assert!((segments[0].0 - 0.0).abs() < f32::EPSILON);
    assert!((segments[0].2 - 0.2).abs() < f32::EPSILON);
    assert!((segments[1].0 - 0.8).abs() < f32::EPSILON);
    assert!((segments[1].2 - 1.0).abs() < f32::EPSILON);
}
