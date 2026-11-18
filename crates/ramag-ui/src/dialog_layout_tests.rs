//! Open the actual shared dialogs to check viewport clipping and reachable controls.

use std::sync::Arc;

use gpui_kit::component::{IconName, Root};
use gpui_kit::{
    AppContext as _, Bounds, Context, ElementId, IntoElement, Modifiers, MouseButton, Pixels,
    Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px, size,
};

struct DialogHost;

impl Render for DialogHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
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

fn accessible_name(cx: &mut VisualTestContext, selector: &'static str) -> String {
    let element_id = ElementId::from(selector);
    cx.update(|window, _| {
        let Some(snapshot) = gpui_kit::base::test_support::find(window, &[], &element_id) else {
            unreachable!("应找到快捷键按钮节点：{selector}");
        };
        let Some(label) = snapshot.label() else {
            unreachable!("快捷键按钮应有辅助名称：{selector}");
        };
        label.to_owned()
    })
}

#[gpui_kit::test]
fn shortcut_record_buttons_announce_action_key_and_recording_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    cx.update(|app| crate::shortcuts_dialog::init_shortcut_overrides(None, app));
    let (_, visual) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| DialogHost);
        Root::new(host, window, cx)
    });
    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.update(crate::open_shortcuts);
    visual.run_until_parked();
    visual
        .executor()
        .advance_clock(std::time::Duration::from_millis(300));
    visual.run_until_parked();

    let query_name = accessible_name(visual, "shortcut-record-new-query");
    let pull_name = accessible_name(visual, "shortcut-record-vcs-pull");
    let Some(query_key) = query_name.strip_prefix("新建查询，快捷键：") else {
        unreachable!("新建查询辅助名称应包含操作和快捷键：{query_name}");
    };
    let Some(pull_key) = pull_name.strip_prefix("拉取，快捷键：") else {
        unreachable!("拉取辅助名称应包含操作和快捷键：{pull_name}");
    };
    assert!(!query_key.is_empty(), "快捷键名称应包含当前组合键");
    assert_eq!(query_key, pull_key, "相同组合键应分别标注各自操作");
    assert_ne!(query_name, pull_name, "不同操作不能只报告相同组合键");

    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    let record = bounds(visual, "shortcut-record-open-recent");
    assert_visible(visual, "shortcut-record-open-recent", 360.0, 640.0);
    visual.simulate_mouse_down(record.center(), MouseButton::Left, Modifiers::default());
    visual.simulate_mouse_up(record.center(), MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
    assert_eq!(
        accessible_name(visual, "shortcut-record-open-recent"),
        "选择连接，正在录制快捷键"
    );
}

#[gpui_kit::test]
fn shortcut_recording_controls_remain_readable_after_resize(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
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
#[gpui_kit::test]
fn recent_picker_can_scroll_search_and_open_in_a_compact_window(cx: &mut TestAppContext) {
    use gpui_kit::{ScrollDelta, ScrollEvent, TouchPhase, point};
    use std::sync::Mutex;
    cx.update(gpui_kit::component::init);
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
    cx.simulate_event(ScrollEvent {
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
    // gpui-kit 0.10 为对话框启用 250ms 退出过渡；推进测试时钟后再检查内容已卸载。
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(260));
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    assert_eq!(
        *selected.lock().unwrap_or_else(|error| error.into_inner()),
        Some("29".into())
    );
    assert!(cx.debug_bounds("recent-picker-body").is_none());
}

#[gpui_kit::test]
fn shared_dialogs_fit_small_and_short_windows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    // 布局测试不验证动效；关闭 reduced motion 后保证每轮切换都立即卸载旧弹窗。
    cx.update(|app| app.set_reduce_motion(true));
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
                use gpui_kit::component::WindowExt as _;
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
                // gpui-kit 0.10 的取消动作也走退出过渡，推进时钟并绘制一帧确认已卸载。
                cx.executor()
                    .advance_clock(std::time::Duration::from_millis(260));
                cx.run_until_parked();
                cx.update(|window, cx| {
                    window.simulate_next_frame(cx);
                    window.draw(cx).clear(cx);
                });
                cx.update(|window, cx| {
                    window.simulate_next_frame(cx);
                    window.draw(cx).clear(cx);
                });
                assert!(
                    cx.debug_bounds("import-options-form").is_none(),
                    "import dialog remained after cancel at {width}x{height}"
                );
            }
        }
    }
}
