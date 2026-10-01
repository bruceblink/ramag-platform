use super::*;
use crate::ProcessSort;
use gpui_kit::{Bounds, Pixels, VisualTestContext};

const SORT_CONTROLS: [(ProcessSort, &str, [u32; 3]); 7] = [
    (
        ProcessSort::Cpu,
        "system-process-sort-cpu",
        [1003, 1002, 1001],
    ),
    (
        ProcessSort::Memory,
        "system-process-sort-memory",
        [1002, 1001, 1003],
    ),
    (
        ProcessSort::Pid,
        "system-process-sort-pid",
        [1001, 1002, 1003],
    ),
    (
        ProcessSort::Name,
        "system-process-sort-name",
        [1003, 1002, 1001],
    ),
    (
        ProcessSort::User,
        "system-process-sort-user",
        [1003, 1002, 1001],
    ),
    (
        ProcessSort::Read,
        "system-process-sort-read",
        [1001, 1002, 1003],
    ),
    (
        ProcessSort::Write,
        "system-process-sort-write",
        [1002, 1003, 1001],
    ),
];

const COLUMN_SELECTORS: [(&str, &str); 7] = [
    ("system-process-cell-1001-pid", "system-process-header-pid"),
    (
        "system-process-cell-1001-name",
        "system-process-header-name",
    ),
    (
        "system-process-cell-1001-user",
        "system-process-header-user",
    ),
    ("system-process-cell-1001-cpu", "system-process-header-cpu"),
    (
        "system-process-cell-1001-memory",
        "system-process-header-memory",
    ),
    (
        "system-process-cell-1001-read",
        "system-process-header-read",
    ),
    (
        "system-process-cell-1001-write",
        "system-process-header-write",
    ),
];

/// Values vary independently so each header must actually sort its own physical field.
fn sortable_snapshot() -> MonitorSnapshot {
    let mut data = process_snapshot();
    let original = data.host.processes[0].clone();
    data.host.processes = [
        (
            1001,
            "zulu-worker-with-a-very-long-process-name-数据库后台任务",
            "用户c",
            10.0,
            20.0,
            30.0,
            10.0,
        ),
        (1002, "beta-worker", "用户b", 20.0, 30.0, 20.0, 30.0),
        (1003, "alpha-worker", "用户a", 30.0, 10.0, 10.0, 20.0),
    ]
    .into_iter()
    .map(|(pid, name, user, cpu, memory, read, write)| {
        let mut row = original.clone();
        row.identity.pid = pid;
        row.name = name.into();
        row.user = Some(user.into());
        row.cpu_percent.value = Some(cpu);
        row.memory_bytes.value = Some(memory * 1024.0 * 1024.0);
        row.read_bytes_per_second.value = Some(read * 1024.0);
        row.write_bytes_per_second.value = Some(write * 1024.0);
        row
    })
    .collect();
    data
}

fn row_selector(pid: u32) -> &'static str {
    match pid {
        1001 => "system-process-row-1001",
        1002 => "system-process-row-1002",
        1003 => "system-process-row-1003",
        _ => unreachable!("known test PID"),
    }
}

/// Reads visual row positions, rather than recomputing the model's sort result.
fn visible_order(visual: &mut VisualTestContext) -> Vec<u32> {
    let mut rows = [1001, 1002, 1003].map(|pid| {
        (
            pid,
            required!(visual.debug_bounds(row_selector(pid)), "process row").top(),
        )
    });
    rows.sort_by(|left, right| {
        left.1
            .partial_cmp(&right.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.into_iter().map(|(pid, _)| pid).collect()
}

fn contained(child: Bounds<Pixels>, parent: Bounds<Pixels>) {
    assert!(
        child.left() >= parent.left() && child.right() <= parent.right(),
        "horizontal bounds: {child:?} in {parent:?}"
    );
    assert!(
        child.top() >= parent.top() && child.bottom() <= parent.bottom(),
        "vertical bounds: {child:?} in {parent:?}"
    );
}

/// Exercises real header hit targets in both themes and type sizes, and verifies column geometry.
#[gpui_kit::test]
fn seven_headers_sort_actual_rows_and_keep_columns_aligned(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = test_view(window, cx, sortable_snapshot());
            view.section = SystemSection::Processes;
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_entity, "system view");
    for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
        for text_size in [
            ramag_ui::InterfaceTextSize::Standard,
            ramag_ui::InterfaceTextSize::Large,
        ] {
            visual.update(|_, app| {
                ramag_ui::apply_theme(mode, app);
                ramag_ui::set_system_settings(
                    ramag_ui::SystemSettings {
                        text_size,
                        ..Default::default()
                    },
                    app,
                );
            });
            for (width, height) in [(1024.0, 768.0), (1440.0, 900.0)] {
                visual.simulate_resize(size(px(width), px(height)));
                for (sort, selector, expected) in SORT_CONTROLS {
                    visual.update(|_, app| {
                        view.update(app, |this, cx| {
                            // Start on a different field to exercise the selected column's default.
                            let other = if sort == ProcessSort::Pid {
                                ProcessSort::Name
                            } else {
                                ProcessSort::Pid
                            };
                            this.select_process_sort(other, cx);
                        })
                    });
                    visual.run_until_parked();
                    let button = required!(visual.debug_bounds(selector), "sortable header");
                    let viewport = required!(
                        visual.debug_bounds("system-process-table-viewport"),
                        "table viewport"
                    );
                    contained(button, viewport);
                    visual.simulate_click(button.center(), gpui_kit::Modifiers::default());
                    visual.run_until_parked();
                    assert_eq!(visible_order(visual), expected, "default {sort:?}");
                    let button =
                        required!(visual.debug_bounds(selector), "updated sortable header");
                    visual.simulate_click(button.center(), gpui_kit::Modifiers::default());
                    visual.run_until_parked();
                    assert_eq!(
                        visible_order(visual),
                        expected.into_iter().rev().collect::<Vec<_>>(),
                        "reverse {sort:?}"
                    );
                }
                let row = required!(visual.debug_bounds("system-process-row-1001"), "long row");
                contained(
                    row,
                    required!(
                        visual.debug_bounds("system-process-table-viewport"),
                        "viewport"
                    ),
                );
                let mut previous_right = row.left();
                for (selector, header_selector) in COLUMN_SELECTORS {
                    let cell = required!(visual.debug_bounds(selector), "physical cell");
                    let header = required!(visual.debug_bounds(header_selector), "column header");
                    contained(cell, row);
                    assert!(cell.left() >= previous_right, "columns overlap");
                    assert_eq!(cell.left(), header.left(), "header/body left alignment");
                    assert_eq!(cell.right(), header.right(), "header/body right alignment");
                    previous_right = cell.right();
                }
                let action = required!(visual.debug_bounds("system-kill-1001"), "row action");
                contained(action, row);
                assert!(
                    action.left() >= previous_right,
                    "action overlaps last metric"
                );
            }
        }
    }
}

/// A compact menu uses GPUI keyboard selection and the direction button reverses the visible rows.
#[gpui_kit::test]
fn compact_menu_search_and_confirmation_keep_the_process_identity(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = test_view(window, cx, sortable_snapshot());
            view.section = SystemSection::Processes;
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_entity, "system view");
    visual.simulate_resize(size(px(360.0), px(640.0)));
    for (index, sort) in ProcessSort::ALL.into_iter().enumerate() {
        visual.run_until_parked();
        let trigger = required!(visual.debug_bounds("system-process-sort-menu"), "sort menu");
        visual.simulate_click(trigger.center(), gpui_kit::Modifiers::default());
        visual.run_until_parked();
        for _ in 0..=index {
            visual.simulate_keystrokes("down");
        }
        visual.simulate_keystrokes("enter");
        visual.run_until_parked();
        visual.update(|_, app| assert_eq!(view.read(app).monitor.process_sort(), sort));
        let before = visible_order(visual);
        let reverse = required!(
            visual.debug_bounds("system-process-sort-direction"),
            "reverse control"
        );
        visual.simulate_click(reverse.center(), gpui_kit::Modifiers::default());
        visual.run_until_parked();
        assert_eq!(
            visible_order(visual),
            before.into_iter().rev().collect::<Vec<_>>()
        );
    }
    let search = required!(visual.debug_bounds("system-process-search"), "search field");
    visual.simulate_click(search.center(), gpui_kit::Modifiers::default());
    visual.simulate_input("用户b");
    visual.run_until_parked();
    assert!(visual.debug_bounds("system-process-row-1001").is_none());
    assert!(visual.debug_bounds("system-process-row-1003").is_none());
    let action = required!(visual.debug_bounds("system-kill-1002"), "filtered action");
    visual.simulate_click(action.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
    visual.update(|_, app| {
        let target = required!(
            view.read(app).termination_request.as_ref(),
            "captured target"
        );
        assert_eq!(target.identity.pid, 1002);
        assert_eq!(target.identity.start_time_ticks, 918273);
        assert_eq!(target.name, "beta-worker");
    });
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    visual.update(|_, app| {
        assert!(view.read(app).termination_request.is_none());
        assert!(!view.read(app).termination_in_progress);
    });
}

/// Compact metrics remain within each row; a medium viewport scrolls the complete desktop table.
#[gpui_kit::test]
fn compact_metrics_and_medium_horizontal_scroll_fit_the_theme_size_matrix(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let mut snapshot = process_snapshot();
    snapshot.host.processes[0].name = "long-running-worker-process-数据库后台任务".repeat(8);
    snapshot.host.processes[0].user = Some("long-user-name-管理员".repeat(8));
    let mut view_entity = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = test_view(window, cx, snapshot);
            view.section = SystemSection::Processes;
            view
        });
        view_entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = required!(view_entity, "system view");
    for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
        for text_size in [
            ramag_ui::InterfaceTextSize::Standard,
            ramag_ui::InterfaceTextSize::Large,
        ] {
            visual.update(|_, app| {
                ramag_ui::apply_theme(mode, app);
                ramag_ui::set_system_settings(
                    ramag_ui::SystemSettings {
                        text_size,
                        ..Default::default()
                    },
                    app,
                );
            });
            visual.simulate_resize(size(px(360.0), px(640.0)));
            visual.run_until_parked();
            let row = required!(
                visual.debug_bounds("system-process-row-4242"),
                "compact row"
            );
            for selector in [
                "system-process-cell-4242-pid",
                "system-process-cell-4242-name",
                "system-process-cell-4242-user",
                "system-process-cell-4242-cpu",
                "system-process-cell-4242-memory",
                "system-process-cell-4242-read",
                "system-process-cell-4242-write",
                "system-kill-4242",
            ] {
                contained(
                    required!(visual.debug_bounds(selector), "compact field"),
                    row,
                );
            }
            assert!(
                visual
                    .debug_bounds("system-process-table-viewport")
                    .is_none()
            );
            visual.simulate_resize(size(px(640.0), px(640.0)));
            visual.run_until_parked();
            let viewport = required!(
                visual.debug_bounds("system-process-table-viewport"),
                "medium viewport"
            );
            assert!(viewport.left() >= px(0.0) && viewport.right() <= px(640.0));
            let last = required!(
                visual.debug_bounds("system-process-cell-4242-write"),
                "last metric"
            );
            assert!(
                last.right() > viewport.right(),
                "medium width must retain readable tracks"
            );
            visual.simulate_event(gpui_kit::ScrollWheelEvent {
                position: viewport.center(),
                delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(-700.0), px(0.0))),
                modifiers: gpui_kit::Modifiers::default(),
                touch_phase: gpui_kit::TouchPhase::Moved,
            });
            visual.run_until_parked();
            visual
                .update(|_, app| assert!(view.read(app).process_table_scroll.offset().x < px(0.0)));
            let action = required!(visual.debug_bounds("system-kill-4242"), "scrolled action");
            contained(action, viewport);
            visual.update(|_, app| {
                view.update(app, |this, cx| {
                    this.process_table_scroll
                        .set_offset(gpui_kit::point(px(0.0), px(0.0)));
                    cx.notify();
                })
            });
        }
    }
}
