use gpui_kit::component::{Root, WindowExt as _};
use gpui_kit::{AppContext as _, Modifiers, TestAppContext, px, size};
use ramag_domain::entities::ConnectionConfig;

use super::{ConnectionListPanel, ConnectionSession, TreeEvent};
use crate::views::table_tree::render_tests::build_services;

#[gpui_kit::test]
fn database_object_dialog_keeps_query_width_and_restores_desktop_tree(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (service, redis, mongo) = build_services();
    let mut session = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let list = cx.new(|cx| ConnectionListPanel::new(service.clone(), redis, mongo, window, cx));
        let view = cx.new(|cx| {
            ConnectionSession::new(
                ConnectionConfig::new_sqlite("本地测试连接", ":memory:"),
                service,
                ramag_ui::ResultMemoryBudget::default(),
                list,
                window,
                cx,
            )
        });
        session = Some(view.clone());
        Root::new(view, window, cx)
    });
    let Some(session) = session else {
        unreachable!("数据库会话应初始化");
    };

    for height in [640.0, 240.0] {
        visual.simulate_resize(size(px(360.0), px(height)));
        visual.run_until_parked();
        let Some(query) = visual.debug_bounds("database-workbench-query-workbench") else {
            unreachable!("应显示查询工作区");
        };
        assert_eq!(query.size.width, px(360.0));
        let Some(button) = visual.debug_bounds("open-table-tree") else {
            unreachable!("窄窗口应显示对象入口");
        };
        visual.simulate_click(button.center(), Modifiers::default());
        visual.run_until_parked();
        let Some(tree) = visual.debug_bounds("database-object-dialog-tree") else {
            unreachable!("应显示对象对话框");
        };
        assert!(tree.size.width > px(0.0) && tree.size.height > px(0.0));
        assert!(tree.origin.x >= px(0.0) && tree.right() <= px(360.0));
        assert!(tree.origin.y >= px(0.0) && tree.bottom() <= px(height));
        assert_eq!(
            visual.debug_bounds("database-workbench-query-workbench"),
            Some(query)
        );
        assert!(
            visual
                .debug_bounds("database-workbench-object-explorer")
                .is_none()
        );
        assert!(visual.update(|window, cx| window.has_active_dialog(cx)));

        visual.simulate_keystrokes("tab");
        assert!(visual.update(|window, cx| {
            gpui_kit::base::active_focus_trap(window, cx)
                .zip(window.focused(cx))
                .is_some_and(|(trap, focused)| trap.contains(&focused, window))
        }));
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        assert!(!visual.update(|window, cx| window.has_active_dialog(cx)));
        assert!(!visual.update(|_, cx| session.read(cx).object_dialog_open));
        assert!(visual.update(|window, _| {
            gpui_kit::base::test_support::find(window, &[], &"open-table-tree".into())
                .is_some_and(|button| button.focused() == Some(true))
        }));
    }

    let Some(button) = visual.debug_bounds("open-table-tree") else {
        unreachable!("窄窗口应显示对象入口");
    };
    visual.simulate_click(button.center(), Modifiers::default());
    visual.run_until_parked();

    visual.simulate_resize(size(px(1024.0), px(768.0)));
    visual.run_until_parked();
    assert!(
        visual
            .debug_bounds("database-workbench-object-explorer")
            .is_none()
    );
    let Some(close) = visual.debug_bounds("database-object-dialog-close") else {
        unreachable!("对象对话框应显示关闭按钮");
    };
    visual.simulate_click(close.center(), Modifiers::default());
    visual.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    assert!(visual.update(|window, cx| session.read(cx).focus_handle.is_focused(window)));
    assert!(visual.debug_bounds("database-object-dialog-tree").is_none());
    assert!(
        visual
            .debug_bounds("database-workbench-object-explorer")
            .is_some()
    );
    assert!(visual.debug_bounds("open-table-tree").is_none());

    visual.simulate_resize(size(px(360.0), px(640.0)));
    visual.run_until_parked();
    let Some(button) = visual.debug_bounds("open-table-tree") else {
        unreachable!("窄窗口应恢复对象入口");
    };
    visual.simulate_click(button.center(), Modifiers::default());
    visual.run_until_parked();
    let tree = visual.update(|_, cx| session.read(cx).tree.clone());
    tree.update(visual, |_, cx| {
        cx.emit(TreeEvent::TableSelected {
            schema: "main".into(),
            table: "users".into(),
        });
    });
    visual.run_until_parked();
    assert!(!visual.update(|window, cx| window.has_active_dialog(cx)));
    assert!(visual.debug_bounds("database-object-dialog-tree").is_none());
}
