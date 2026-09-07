//! Open the actual shared dialogs to check viewport clipping and reachable controls.

use std::sync::Arc;

use gpui::{
    AppContext as _, Bounds, Context, IntoElement, Modifiers, MouseButton, ParentElement as _,
    Pixels, Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px, size,
};
use gpui_component::{IconName, Root};

struct DialogHost;

impl Render for DialogHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .children(Root::render_dialog_layer(window, cx))
    }
}

/// Missing selectors fail the test instead of skipping an unrendered control.
fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    let bounds = cx.debug_bounds(selector);
    assert!(bounds.is_some(), "missing dialog element: {selector}");
    bounds.unwrap_or_default()
}

fn assert_visible(cx: &mut VisualTestContext, selector: &'static str, width: f32, height: f32) {
    let bounds = bounds(cx, selector);
    assert!(
        bounds.origin.x >= px(0.0)
            && bounds.origin.y >= px(0.0)
            && bounds.right() <= px(width)
            && bounds.bottom() <= px(height)
            && bounds.size.width > px(0.0)
            && bounds.size.height > px(0.0),
        "{selector} must fit {width}x{height}: {bounds:?}"
    );
}

#[gpui::test]
fn shortcut_recording_controls_remain_readable_after_resize(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    cx.update(|app| {
        crate::shortcuts_dialog::init_shortcut_overrides(
            Some(r#"{"open-recent":"ctrl-alt-shift-p"}"#),
            app,
        )
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| DialogHost);
        Root::new(host, window, cx)
    });
    cx.simulate_resize(size(px(1024.0), px(768.0)));
    cx.update(crate::open_shortcuts);
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    cx.simulate_resize(size(px(360.0), px(640.0)));
    cx.run_until_parked();
    let description = bounds(cx, "shortcut-description-open-recent");
    let record = bounds(cx, "shortcut-record-open-recent");
    assert!(
        description.size.width >= px(180.0),
        "description must remain readable: {description:?}"
    );
    assert!(
        description.bottom() <= record.origin.y,
        "compact row must stack: {description:?}, {record:?}"
    );
    assert_visible(cx, "shortcut-record-open-recent", 360.0, 640.0);
    cx.simulate_mouse_down(record.center(), MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(record.center(), MouseButton::Left, Modifiers::default());
    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    assert_visible(cx, "shortcut-error", 360.0, 640.0);
    assert_visible(cx, "shortcut-center-scroll", 360.0, 640.0);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("shortcuts-body").is_some(),
        "Escape must stop recording without closing the dialog"
    );
}

/// Scroll through clipped dialog content and select the final real row.
#[gpui::test]
fn recent_picker_can_scroll_search_and_open_in_a_compact_window(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point};
    use std::sync::Mutex;
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| DialogHost);
        Root::new(host, window, cx)
    });
    let selected = Arc::new(Mutex::new(None));
    let output = selected.clone();
    cx.simulate_resize(size(px(360.0), px(640.0)));
    cx.update(|window, app| {
        crate::recent_items_dialog::open_recent_item_picker(
            window,
            app,
            "Connections",
            "Search",
            "test-recent-scroll",
            (0..30)
                .map(|index| {
                    crate::recent_items_dialog::RecentItem::new(
                        index.to_string(),
                        format!("Database {index}"),
                        "host/database",
                        IconName::Folder,
                    )
                    .secondary("postgres/production")
                })
                .collect(),
            Arc::new(move |id, _, _| {
                *output.lock().unwrap_or_else(|error| error.into_inner()) = Some(id)
            }),
        )
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    let viewport = bounds(cx, "recent-picker-scroll");
    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-10000.0))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();
    let last = bounds(cx, "recent-picker-open-false-29");
    assert!(
        viewport.contains(&last.center()),
        "last row must be reachable: {last:?}, {viewport:?}"
    );
    cx.simulate_input("Database 29");
    cx.run_until_parked();
    let open = bounds(cx, "recent-picker-open-false-0");
    assert!(
        viewport.contains(&open.center()),
        "search must reset scroll position: {open:?}"
    );
    cx.simulate_mouse_down(open.center(), MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(open.center(), MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        *selected.lock().unwrap_or_else(|error| error.into_inner()),
        Some("29".into())
    );
    assert!(cx.debug_bounds("recent-picker-body").is_none());
}

#[gpui::test]
fn shared_dialogs_fit_small_and_short_windows(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| DialogHost);
        Root::new(host, window, cx)
    });
    for kind in ["import", "recent", "shortcuts"] {
        for (width, height) in [
            (360.0, 240.0),
            (360.0, 640.0),
            (1024.0, 768.0),
            (1440.0, 900.0),
        ] {
            cx.simulate_resize(size(px(width), px(height)));
            cx.update(|window, app| {
                use gpui_component::WindowExt as _;
                window.close_all_dialogs(app);
                match kind {
                    "import" => crate::open_import_options_dialog(
                        "Import connections",
                        "Choose how to handle duplicate connections.",
                        true,
                        ("JSON", &["json"]),
                        |_, _, _, _| {},
                        window,
                        app,
                    ),
                    "recent" => crate::recent_items_dialog::open_recent_item_picker(
                        window,
                        app,
                        "Recent connections",
                        "Search",
                        "test-recent",
                        vec![
                            crate::recent_items_dialog::RecentItem::new(
                                "test",
                                "long-connection-name-".repeat(8),
                                "database/path/".repeat(8),
                                IconName::Folder,
                            )
                            .secondary("read-only-secondary-detail-".repeat(6))
                            .badge("Production"),
                        ],
                        Arc::new(|_, _, _| {}),
                    ),
                    _ => crate::open_shortcuts(window, app),
                }
                window.refresh();
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(300));
            cx.run_until_parked();
            let selectors: &[&str] = match kind {
                "import" => &["import-options-form", "ramag-import-cancel"],
                "recent" => &["recent-picker-body", "recent-picker-scroll"],
                _ => &[
                    "shortcuts-body",
                    "shortcut-center-scroll",
                    "shortcut-reset-all",
                ],
            };
            for selector in selectors {
                assert_visible(cx, selector, width, height);
            }
            if kind == "import" {
                let point = bounds(cx, "ramag-import-cancel").center();
                cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::default());
                cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::default());
                cx.run_until_parked();
                assert!(cx.debug_bounds("import-options-form").is_none());
            }
        }
    }
}
