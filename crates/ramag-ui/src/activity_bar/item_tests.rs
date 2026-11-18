//! 使用 GPUI 实际鼠标事件验证活动栏拖放会更新顺序并保存偏好。

use std::{sync::Arc, time::Duration};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext as _, Context, Entity, ParentElement, Render, Styled as _, TestAppContext, Window,
    px, size,
};
use ramag_app::{StaticPluginHost, TOOL_ORDER_PREF_KEY, ToolRegistry};
use ramag_domain::{Tool, ToolMeta, traits::Storage};
use ramag_infra_storage::RedbStorage;

use super::super::ActivityBar;

struct TestTool(ToolMeta);

impl Tool for TestTool {
    fn meta(&self) -> &ToolMeta {
        &self.0
    }
}

struct ActivityBarHost(Entity<ActivityBar>);

impl Render for ActivityBarHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        gpui_kit::div().size_full().child(self.0.clone())
    }
}

fn activity_registry() -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(TestTool(ToolMeta::new("api", "API", ""))));
    registry.register(Arc::new(TestTool(ToolMeta::new("dbclient", "数据库", ""))));
    registry
}

#[gpui_kit::test]
fn real_sidebar_drag_reorders_tools_and_persists_the_order(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let path = std::env::temp_dir().join(format!(
        "ramag-activity-drag-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos()
    ));
    let Ok(inner) = RedbStorage::open_with_key(&path, &[0x4b; 32]) else {
        unreachable!("隔离拖放测试存储应打开")
    };
    let storage: Arc<dyn Storage> = Arc::new(
        crate::preferences::test_storage::SettingsTestStorage::new(inner),
    );
    cx.update(|app| app.set_global(crate::StorageGlobal(storage.clone())));

    let registry = activity_registry();
    let plugin_host = Arc::new(StaticPluginHost::new(registry.clone()));
    let mut bar_entity = None;
    let (host, visual) = cx.add_window_view(|_, cx| {
        let bar = cx.new(|cx| ActivityBar::new(registry.clone(), plugin_host.clone(), cx));
        bar_entity = Some(bar.clone());
        ActivityBarHost(bar)
    });
    let Some(bar) = bar_entity else {
        unreachable!("活动栏实体应创建")
    };
    visual.simulate_resize(size(px(1440.0), px(900.0)));
    visual.run_until_parked();

    assert_eq!(
        registry.order(),
        vec!["api".to_owned(), "dbclient".to_owned()]
    );
    visual.update(|window, cx| {
        let database = window.find("tool-dbclient").bounds();
        let api = window.find("tool-api").bounds();
        let destination = gpui_kit::point(api.center().x, api.origin.y + px(4.0));
        window.drag(database.center(), destination, cx);
    });
    visual.run_until_parked();

    let expected_order = vec!["dbclient".to_owned(), "api".to_owned()];
    assert_eq!(registry.order(), expected_order);
    let mut save_status = None;
    for _ in 0..20 {
        visual.run_until_parked();
        save_status = visual
            .update(|_, app| crate::preferences::preference_save_status(TOOL_ORDER_PREF_KEY, app));
        if save_status == Some(crate::preferences::PreferenceSaveStatus::Saved) {
            break;
        }
        visual
            .background_executor
            .advance_clock(Duration::from_millis(10));
    }
    assert_eq!(
        save_status,
        Some(crate::preferences::PreferenceSaveStatus::Saved),
        "拖放完成后应持久化工具顺序"
    );
    let Ok(Some(persisted)) =
        futures::executor::block_on(storage.get_preference(TOOL_ORDER_PREF_KEY))
    else {
        unreachable!("应读取刚保存的工具顺序")
    };
    let Ok(persisted_order) = serde_json::from_str::<Vec<String>>(&persisted) else {
        unreachable!("工具顺序偏好应为 JSON ID 列表")
    };
    assert_eq!(persisted_order, expected_order);

    visual.update(|window, app| {
        app.remove_global::<crate::StorageGlobal>();
        window.remove_window();
    });
    drop(host);
    drop(bar);
    visual.run_until_parked();
    drop(storage);
    assert!(std::fs::remove_file(path).is_ok(), "应清理隔离 Redb 文件");
}
