use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use async_trait::async_trait;
use gpui::{
    AppContext as _, Context, IntoElement, Modifiers, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, point, px, size,
};
use ramag_app::MqttService;
use ramag_domain::entities::{
    ConnectionConfig, ConnectionId, MqttProfile, QueryRecord, QueryRecordId,
};
use ramag_domain::error::Result;
use ramag_domain::traits::{MqttDriver, Storage};

use super::{MQTT_SIDEBAR_COLLAPSE_BREAKPOINT, MqttView};

struct NoopMqttDriver;

#[async_trait]
impl MqttDriver for NoopMqttDriver {}

#[derive(Default)]
struct NoopStorage {
    mqtt_profiles: Arc<Mutex<Vec<MqttProfile>>>,
}

#[async_trait]
impl Storage for NoopStorage {
    async fn list_mqtt_profiles(&self) -> Result<Vec<MqttProfile>> {
        Ok(self
            .mqtt_profiles
            .lock()
            .expect("读取 MQTT 测试配置锁")
            .clone())
    }

    async fn save_mqtt_profile(&self, profile: &MqttProfile) -> Result<()> {
        let mut profiles = self.mqtt_profiles.lock().expect("写入 MQTT 测试配置锁");
        if let Some(existing) = profiles.iter_mut().find(|item| item.id == profile.id) {
            *existing = profile.clone();
        } else {
            profiles.push(profile.clone());
        }
        Ok(())
    }

    async fn list_connections(&self) -> Result<Vec<ConnectionConfig>> {
        Ok(Vec::new())
    }

    async fn get_connection(&self, _: &ConnectionId) -> Result<Option<ConnectionConfig>> {
        Ok(None)
    }

    async fn save_connection(&self, _: &ConnectionConfig) -> Result<()> {
        Ok(())
    }

    async fn delete_connection(&self, _: &ConnectionId) -> Result<()> {
        Ok(())
    }

    async fn append_history(&self, _: &QueryRecord) -> Result<()> {
        Ok(())
    }

    async fn list_history(&self, _: Option<&ConnectionId>, _: usize) -> Result<Vec<QueryRecord>> {
        Ok(Vec::new())
    }

    async fn delete_history(&self, _: &QueryRecordId) -> Result<()> {
        Ok(())
    }

    async fn clear_history(&self, _: Option<&ConnectionId>) -> Result<()> {
        Ok(())
    }

    async fn get_preference(&self, _: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn set_preference(&self, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
}

struct RecordingMqttDriver {
    connection_tests: Arc<AtomicUsize>,
}

#[async_trait]
impl MqttDriver for RecordingMqttDriver {
    async fn test_connection(&self, _profile: &MqttProfile) -> Result<()> {
        self.connection_tests.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

struct MqttTestHost {
    view: gpui::Entity<MqttView>,
}

impl Render for MqttTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = gpui_component::Root::render_dialog_layer(window, cx);
        gpui::div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(dialog_layer)
    }
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("控件应参与布局: {selector}"));
    let center = point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );
    cx.simulate_mouse_move(center, None, Modifiers::default());
    cx.simulate_mouse_down(center, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(center, gpui::MouseButton::Left, Modifiers::default());
}

#[gpui::test]
fn mqtt_sidebar_collapses_and_can_be_reopened_in_narrow_window(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(NoopMqttDriver),
        Arc::new(NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");

    view.update(visual_cx, |view, cx| {
        view.loading_profiles = false;
        cx.notify();
    });

    for (width, height) in [(1024.0, 720.0), (360.0, 640.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();

        let root = visual_cx
            .debug_bounds("mqtt-root")
            .expect("MQTT 根布局应渲染");
        let main = visual_cx
            .debug_bounds("mqtt-main")
            .expect("MQTT 主工作区应渲染");
        assert!(main.origin.x >= root.origin.x);
        assert!(main.right() <= root.right());

        if width < MQTT_SIDEBAR_COLLAPSE_BREAKPOINT {
            assert!(
                visual_cx.debug_bounds("mqtt-sidebar").is_none(),
                "窄窗口默认应收起 MQTT 配置栏"
            );
            let protocol_field = visual_cx
                .debug_bounds("mqtt-protocol-field")
                .expect("协议版本字段应参与布局");
            let protocol_buttons = visual_cx
                .debug_bounds("mqtt-protocol-buttons")
                .expect("协议版本按钮应参与布局");
            assert!(
                protocol_buttons.origin.x >= protocol_field.origin.x
                    && protocol_buttons.right() <= protocol_field.right(),
                "窄窗口协议按钮不能越出自身字段：field={protocol_field:?}, buttons={protocol_buttons:?}"
            );
            click(visual_cx, "mqtt-show-sidebar");
            visual_cx
                .executor()
                .advance_clock(Duration::from_millis(50));
            visual_cx.run_until_parked();

            let sidebar = visual_cx
                .debug_bounds("mqtt-sidebar")
                .expect("打开后 MQTT 配置栏应渲染");
            let main = visual_cx
                .debug_bounds("mqtt-main")
                .expect("打开配置栏后主工作区应渲染");
            assert!(sidebar.right() <= root.right());
            assert!(sidebar.bottom() <= root.bottom());
            assert!(main.origin.y >= sidebar.bottom());
            assert!(visual_cx.debug_bounds("mqtt-hide-sidebar").is_some());

            click(visual_cx, "mqtt-hide-sidebar");
            visual_cx.run_until_parked();
            assert!(visual_cx.debug_bounds("mqtt-sidebar").is_none());
            assert!(visual_cx.debug_bounds("mqtt-show-sidebar").is_some());
        } else {
            let sidebar = visual_cx
                .debug_bounds("mqtt-sidebar")
                .expect("桌面窗口应显示 MQTT 配置栏");
            assert!(sidebar.right() <= root.right());
            assert!(sidebar.bottom() <= root.bottom());
        }
    }
}

#[gpui::test]
fn mqtt_configuration_saves_and_tests_connection(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let storage = Arc::new(NoopStorage::default());
    let connection_tests = Arc::new(AtomicUsize::new(0));
    let driver = Arc::new(RecordingMqttDriver {
        connection_tests: connection_tests.clone(),
    });
    let service = Arc::new(MqttService::new(driver, storage.clone()));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");

    visual_cx.run_until_parked();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.name
                .update(cx, |input, cx| input.set_value("测试 Broker", window, cx));
            view.host
                .update(cx, |input, cx| input.set_value("127.0.0.1", window, cx));
            view.port
                .update(cx, |input, cx| input.set_value("1883", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    click(visual_cx, "mqtt-save-profile");
    visual_cx.run_until_parked();
    assert_eq!(
        storage.mqtt_profiles.lock().expect("读取保存结果锁").len(),
        1,
        "保存按钮必须调用 MQTT 配置存储"
    );

    click(visual_cx, "mqtt-test-connection");
    visual_cx.run_until_parked();
    assert_eq!(
        connection_tests.load(Ordering::Relaxed),
        1,
        "测试连接按钮必须调用 MQTT 驱动"
    );
}
