use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use chrono::Utc;

use async_channel::{Receiver, Sender, bounded};
use async_trait::async_trait;
use gpui_kit::{
    AppContext as _, Context, IntoElement, Modifiers, ParentElement as _, Render, ScrollDelta,
    ScrollWheelEvent, Styled as _, TestAppContext, TouchPhase, VisualTestContext, Window, point,
    px, size,
};
use ramag_app::MqttService;
use ramag_domain::entities::{
    ConnectionConfig, ConnectionId, MosquittoAcl, MosquittoAclDecision, MosquittoAclType,
    MosquittoClient, MosquittoDynamicSecuritySnapshot, MosquittoRole, MosquittoRoleBinding,
    MqttBrokerMetrics, MqttBrokerSnapshot, MqttLocalServerConfig, MqttLocalServerEvent,
    MqttLocalServerStatus, MqttMessage, MqttOnlineClient, MqttProfile, MqttPublishRequest,
    MqttPublishResult, MqttQos, MqttSubscription, MqttTopicObservation, MqttTopicSource,
    MqttUserProperty, QueryRecord, QueryRecordId,
};
use ramag_domain::error::Result;
use ramag_domain::traits::{MqttDriver, MqttLocalServerDriver, Storage};

use super::{
    MQTT_SIDEBAR_COLLAPSE_BREAKPOINT, MosquittoManagementSection, MqttProfileConnectionStatus,
    MqttSection, MqttView,
};

pub(super) struct NoopMqttDriver;

#[async_trait]
impl MqttDriver for NoopMqttDriver {}

#[derive(Default)]
pub(super) struct NoopStorage {
    pub(crate) mqtt_profiles: Arc<Mutex<Vec<MqttProfile>>>,
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

struct RecordingLocalServerDriver {
    publishes: Arc<Mutex<Vec<MqttPublishRequest>>>,
    snapshot: MqttBrokerSnapshot,
}

#[async_trait]
impl MqttLocalServerDriver for RecordingLocalServerDriver {
    async fn publish(&self, request: &MqttPublishRequest) -> Result<MqttPublishResult> {
        self.publishes
            .lock()
            .expect("本地 Broker 发布记录锁")
            .push(request.clone());
        Ok(MqttPublishResult {
            topic: request.topic.clone(),
            packet_id: None,
            qos: request.qos,
        })
    }

    async fn snapshot(&self) -> Result<MqttBrokerSnapshot> {
        Ok(self.snapshot.clone())
    }
}

struct EventLocalServerDriver {
    events: Vec<MqttLocalServerEvent>,
}

#[async_trait]
impl MqttLocalServerDriver for EventLocalServerDriver {
    async fn stop(&self) -> Result<MqttLocalServerStatus> {
        Ok(MqttLocalServerStatus::stopped(
            &MqttLocalServerConfig::default(),
        ))
    }

    async fn subscribe_events(
        &self,
        sink: ramag_domain::entities::MqttLocalServerEventSink,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<()> {
        // 事件测试驱动只发送一批确定数据；真实驱动的长连接取消由独立集成测试覆盖。
        for event in &self.events {
            if cancelled.load(Ordering::Acquire) {
                return Ok(());
            }
            let _ = sink(event.clone());
        }
        Ok(())
    }
}

struct BlockingSnapshotDriver {
    release: Receiver<()>,
    started: Sender<()>,
}

#[async_trait]
impl MqttDriver for BlockingSnapshotDriver {
    async fn broker_snapshot(&self, profile: &MqttProfile) -> Result<MqttBrokerSnapshot> {
        self.started
            .send(())
            .await
            .expect("快照测试开始信号应可发送");
        if profile.host == "first.example" {
            self.release.recv().await.expect("快照测试释放信号应可接收");
        }
        Ok(MqttBrokerSnapshot {
            topics: vec![MqttTopicObservation {
                name: format!("{}.topic", profile.host),
                source: MqttTopicSource::Observed,
                retained: false,
                observed_at: None,
                publish_count: 1,
                subscriber_count: 1,
                last_payload_bytes: 5,
            }],
            online_clients: Vec::new(),
            topics_complete: false,
            online_clients_complete: false,
            metrics: MqttBrokerMetrics::default(),
        })
    }
}

pub(super) struct MqttTestHost {
    pub(super) view: gpui_kit::Entity<MqttView>,
}

impl Render for MqttTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = gpui_kit::component::Root::render_dialog_layer(window, cx);
        gpui_kit::div()
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
    cx.simulate_mouse_down(center, gpui_kit::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(center, gpui_kit::MouseButton::Left, Modifiers::default());
}

fn scroll_local_server(cx: &mut VisualTestContext, delta_y: f32) {
    let bounds = cx
        .debug_bounds("mqtt-local-server-scroll")
        .expect("本地 MQTT Broker 滚动容器应参与布局");
    cx.simulate_event(ScrollWheelEvent {
        position: bounds.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(delta_y))),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();
}

#[gpui_kit::test]
fn mqtt_sidebar_collapses_and_can_be_reopened_in_narrow_window(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(NoopMqttDriver),
        Arc::new(NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
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

#[gpui_kit::test]
fn mqtt_profile_sidebar_shows_name_and_endpoint(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let storage = Arc::new(NoopStorage::default());
    let profile = MqttProfile::new("Docker MQTT UI", "127.0.0.1", 18883);
    storage
        .mqtt_profiles
        .lock()
        .expect("写入 MQTT 测试配置锁")
        .push(profile);
    let service = Arc::new(MqttService::new(Arc::new(NoopMqttDriver), storage));
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });

    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.run_until_parked();

    let sidebar = visual_cx
        .debug_bounds("mqtt-sidebar")
        .expect("MQTT 配置栏应渲染");
    for selector in ["mqtt-profile-name", "mqtt-profile-status"] {
        let bounds = visual_cx
            .debug_bounds(selector)
            .expect("MQTT 配置项文本应参与布局");
        assert!(
            bounds.size.width > px(24.0) && bounds.right() <= sidebar.right(),
            "配置项 {} 必须在侧栏内保留可见宽度: sidebar={sidebar:?}, bounds={bounds:?}",
            selector
        );
    }
}

#[gpui_kit::test]
fn mqtt_configuration_saves_and_tests_connection(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
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
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");

    visual_cx.run_until_parked();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.host
                .update(cx, |input, cx| input.set_value("127.0.0.1", window, cx));
            view.port
                .update(cx, |input, cx| input.set_value("1883", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    click(visual_cx, "mqtt-test-connection");
    visual_cx.run_until_parked();
    assert_eq!(
        connection_tests.load(Ordering::Relaxed),
        1,
        "填写 Broker 地址后，测试连接不应要求先填写配置名称"
    );

    click(visual_cx, "mqtt-save-profile");
    visual_cx.run_until_parked();
    assert!(
        storage
            .mqtt_profiles
            .lock()
            .expect("读取保存结果锁")
            .is_empty(),
        "未填写配置名称时不能保存"
    );

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.name
                .update(cx, |input, cx| input.set_value("测试 Broker", window, cx));
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
        2,
        "测试连接按钮必须调用 MQTT 驱动"
    );
    let profile_id = storage
        .mqtt_profiles
        .lock()
        .expect("读取连接状态配置锁")
        .first()
        .expect("测试配置应已保存")
        .id
        .clone();
    assert_eq!(
        view.read_with(visual_cx, |view, _| {
            view.profile_connection_statuses.get(&profile_id).copied()
        }),
        Some(MqttProfileConnectionStatus::Reachable),
        "连接测试成功后侧栏应显示可连接状态"
    );
}

#[gpui_kit::test]
fn mqtt_snapshot_result_does_not_cross_profile_context(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (release_sender, release_receiver) = bounded(1);
    let (started_sender, started_receiver) = bounded(1);
    let driver = Arc::new(BlockingSnapshotDriver {
        release: release_receiver,
        started: started_sender,
    });
    let service = Arc::new(MqttService::new(driver, Arc::new(NoopStorage::default())));
    let first = MqttProfile::new("第一个 Broker", "first.example", 1883);
    let second = MqttProfile::new("第二个 Broker", "second.example", 1883);
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.profiles = vec![first.clone(), second.clone()];
            view.selected_profile_id = Some(first.id.clone());
            view.set_form_from_profile(&first, window, cx);
            view.load_snapshot(window, cx);
        });
    });
    visual_cx.run_until_parked();
    assert!(
        started_receiver.try_recv().is_ok(),
        "第一个 Broker 快照应已开始"
    );

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.select_profile(second.id.clone(), window, cx);
        });
    });
    visual_cx.run_until_parked();
    assert!(view.read_with(visual_cx, |view, _| {
        view.selected_profile_id == Some(second.id.clone())
            && view.snapshot.is_none()
            && !view.loading_snapshot
    }));

    release_sender
        .try_send(())
        .expect("第一个 Broker 快照应可释放");
    visual_cx.run_until_parked();
    assert!(view.read_with(visual_cx, |view, _| {
        view.selected_profile_id == Some(second.id.clone()) && view.snapshot.is_none()
    }));
}

#[gpui_kit::test]
fn mqtt_message_pages_keep_inputs_bounded_and_editable(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(NoopMqttDriver),
        Arc::new(NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");

    visual_cx.simulate_resize(size(px(1440.0), px(900.0)));
    view.update(visual_cx, |view, cx| {
        view.loading_profiles = false;
        view.section = MqttSection::Publish;
        cx.notify();
    });
    visual_cx.run_until_parked();

    let main = visual_cx
        .debug_bounds("mqtt-main")
        .expect("MQTT 主工作区应渲染");
    let publish_input = visual_cx
        .debug_bounds("mqtt-publish-topic-input")
        .expect("发布 Topic 输入框应参与布局");
    assert!(
        publish_input.origin.x > main.origin.x
            && publish_input.right() <= main.right()
            && publish_input.size.width <= px(920.0),
        "发布 Topic 输入框不能越出消息内容区: main={main:?}, input={publish_input:?}"
    );

    click(visual_cx, "mqtt-tab-Subscribe");
    visual_cx.run_until_parked();

    let subscribe_input = visual_cx
        .debug_bounds("mqtt-subscribe-filter-input")
        .expect("订阅 Topic Filter 输入框应参与布局");
    assert!(
        subscribe_input.origin.x > main.origin.x
            && subscribe_input.right() <= main.right()
            && subscribe_input.size.width <= px(920.0),
        "订阅 Topic Filter 输入框不能越出消息内容区: main={main:?}, input={subscribe_input:?}"
    );

    visual_cx.simulate_keystrokes("sensors/#");
    visual_cx.run_until_parked();
    let filter = view.read_with(visual_cx, |view, cx| {
        view.subscribe_filter.read(cx).value().to_string()
    });
    assert_eq!(
        filter, "sensors/#",
        "进入订阅页后 Topic Filter 应立即接收键盘输入"
    );

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.publish_topic
                .update(cx, |input, cx| input.focus(window, cx));
        });
    });
    visual_cx.run_until_parked();
    click(visual_cx, "mqtt-subscribe-filter-input");
    visual_cx.simulate_keystrokes("/alerts");
    visual_cx.run_until_parked();
    let filter = view.read_with(visual_cx, |view, cx| {
        view.subscribe_filter.read(cx).value().to_string()
    });
    assert_eq!(
        filter, "sensors/#/alerts",
        "点击 Topic Filter 输入框应重新获得键盘焦点"
    );
}

#[gpui_kit::test]
fn mqtt_message_operations_reflow_inside_supported_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(NoopMqttDriver),
        Arc::new(NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    visual_cx.simulate_resize(size(px(1440.0), px(900.0)));
    visual_cx.run_until_parked();
    view.update(visual_cx, |view, cx| {
        view.loading_profiles = false;
        view.section = MqttSection::Subscribe;
        view.messages.push_back(MqttMessage {
            topic: "sensors/warehouse/temperature/very-long-topic-name".into(),
            payload: vec![b'x'; 512],
            qos: MqttQos::AtLeastOnce,
            retain: true,
            duplicate: true,
            received_at: Utc::now(),
            user_properties: vec![MqttUserProperty {
                name: "source".into(),
                value: "headless-test".into(),
            }],
        });
        cx.notify();
    });
    visual_cx.run_until_parked();

    for (width, height) in [
        (360.0, 240.0),
        (640.0, 480.0),
        (1024.0, 768.0),
        (1440.0, 900.0),
    ] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        let scroll = visual_cx
            .debug_bounds("mqtt-main")
            .expect("MQTT 主工作区应参与布局");
        for selector in [
            "mqtt-subscribe-options",
            "mqtt-subscribe-qos",
            "mqtt-subscribe-actions",
            "mqtt-subscribe-message-meta",
            "mqtt-subscribe-message-retained",
            "mqtt-subscribe-message-duplicate",
            "mqtt-subscribe-message-properties",
        ] {
            let bounds = visual_cx
                .debug_bounds(selector)
                .expect("订阅操作和消息元数据应参与布局");
            assert!(
                bounds.origin.x >= scroll.origin.x && bounds.right() <= scroll.right(),
                "{}px 窗口中的 {} 不能越出订阅内容区: scroll={scroll:?}, bounds={bounds:?}",
                width,
                selector
            );
        }
    }
}

#[gpui_kit::test]
fn mqtt_local_server_page_reflows_inside_supported_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(NoopMqttDriver),
        Arc::new(NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.loading_profiles = false;
        view.section = MqttSection::LocalServer;
        cx.notify();
    });

    for (width, height) in [(360.0, 640.0), (640.0, 480.0), (1024.0, 768.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        let main = visual_cx
            .debug_bounds("mqtt-main")
            .expect("MQTT 主工作区应参与布局");
        for selector in [
            "mqtt-local-server-config",
            "mqtt-local-server-actions",
            "mqtt-local-server-status",
            "mqtt-local-server-max-connections-input",
            "mqtt-local-server-publish-topic-input",
            "mqtt-local-server-publish-payload-input",
            "mqtt-local-server-publish-options",
            "mqtt-local-server-publish-actions",
            "mqtt-local-server-user-editor",
        ] {
            let bounds = visual_cx
                .debug_bounds(selector)
                .expect("本地 MQTT Broker 控件应参与布局");
            assert!(
                bounds.origin.x >= main.origin.x && bounds.right() <= main.right(),
                "{}px 窗口中的 {} 不能越出本地服务内容区: main={main:?}, bounds={bounds:?}",
                width,
                selector
            );
        }
    }
}

#[gpui_kit::test]
fn mqtt_local_server_event_timeline_consumes_and_clears_events(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let now = Utc::now();
    let message = MqttMessage {
        topic: "ui/events".into(),
        payload: b"hello".to_vec(),
        qos: MqttQos::AtLeastOnce,
        retain: true,
        duplicate: false,
        received_at: now,
        user_properties: Vec::new(),
    };
    let events = vec![
        MqttLocalServerEvent::ClientConnected {
            client: MqttOnlineClient {
                client_id: "ui-client".into(),
                username: Some("operator".into()),
                remote_address: Some("127.0.0.1:41000".into()),
                connected_at: Some(now),
                subscriptions: Vec::new(),
            },
            occurred_at: now,
        },
        MqttLocalServerEvent::ClientSubscribed {
            client_id: "ui-client".into(),
            subscription: MqttSubscription {
                filter: "ui/#".into(),
                qos: MqttQos::AtLeastOnce,
                no_local: false,
            },
            occurred_at: now,
        },
        MqttLocalServerEvent::ClientPublished {
            client_id: "ui-client".into(),
            message: message.clone(),
            occurred_at: now,
        },
        MqttLocalServerEvent::BrokerPublished {
            message,
            occurred_at: now,
        },
        MqttLocalServerEvent::ClientDisconnected {
            client_id: "ui-client".into(),
            reason: Some("测试结束".into()),
            occurred_at: now,
        },
    ];
    let service = Arc::new(
        MqttService::new(Arc::new(NoopMqttDriver), Arc::new(NoopStorage::default()))
            .with_local_server_driver(Arc::new(EventLocalServerDriver { events })),
    );
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");

    visual_cx.simulate_resize(size(px(1440.0), px(900.0)));
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.section = MqttSection::LocalServer;
            view.local_server_status = Some(MqttLocalServerStatus::running(
                &MqttLocalServerConfig::default(),
            ));
            view.start_local_server_events(window, cx);
        });
    });
    visual_cx.run_until_parked();

    assert!(view.read_with(visual_cx, |view, _| {
        view.local_server_events.len() == 5
            && matches!(
                view.local_server_events.back(),
                Some(MqttLocalServerEvent::ClientDisconnected { .. })
            )
    }));
    for selector in [
        "mqtt-local-server-events",
        "mqtt-local-server-events-list",
        "mqtt-local-server-event-0",
        "mqtt-local-server-event-4",
    ] {
        let bounds = visual_cx
            .debug_bounds(selector)
            .expect("Broker 事件时间线控件应参与布局");
        let main = visual_cx
            .debug_bounds("mqtt-main")
            .expect("MQTT 主工作区应参与布局");
        assert!(
            bounds.origin.x >= main.origin.x && bounds.right() <= main.right(),
            "事件时间线不能越出主工作区: selector={selector}, main={main:?}, bounds={bounds:?}"
        );
    }

    visual_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.clear_local_server_events();
            cx.notify();
        });
    });
    visual_cx.run_until_parked();
    assert!(view.read_with(visual_cx, |view, _| { view.local_server_events.is_empty() }));
    assert!(
        visual_cx
            .debug_bounds("mqtt-local-server-events-empty")
            .is_some()
    );

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.stop_local_server(window, cx);
        });
    });
    visual_cx.run_until_parked();
    assert!(view.read_with(visual_cx, |view, _| {
        view.local_server_status
            .as_ref()
            .is_some_and(|status| !status.running)
            && view.local_server_events.is_empty()
    }));
}

#[gpui_kit::test]
fn mqtt_local_server_publish_uses_broker_injection_controls(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let publishes = Arc::new(Mutex::new(Vec::new()));
    let service = Arc::new(
        MqttService::new(Arc::new(NoopMqttDriver), Arc::new(NoopStorage::default()))
            .with_local_server_driver(Arc::new(RecordingLocalServerDriver {
                publishes: publishes.clone(),
                snapshot: MqttBrokerSnapshot {
                    topics: Vec::new(),
                    online_clients: Vec::new(),
                    topics_complete: false,
                    online_clients_complete: true,
                    metrics: MqttBrokerMetrics::default(),
                },
            })),
    );
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.section = MqttSection::LocalServer;
            view.local_server_status = Some(MqttLocalServerStatus::running(
                &MqttLocalServerConfig::default(),
            ));
            view.local_server_publish_qos = MqttQos::ExactlyOnce;
            view.local_server_publish_retain = true;
            view.local_server_publish_topic
                .update(cx, |input, cx| input.set_value("ui/injected", window, cx));
            view.local_server_publish_payload
                .update(cx, |input, cx| input.set_value("hello from ui", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    scroll_local_server(visual_cx, -10000.0);
    click(visual_cx, "mqtt-local-server-publish");
    visual_cx.run_until_parked();

    let recorded = publishes.lock().expect("读取本地 Broker 发布记录锁");
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].topic, "ui/injected");
    assert_eq!(recorded[0].payload, b"hello from ui");
    assert_eq!(recorded[0].qos, MqttQos::ExactlyOnce);
    assert!(recorded[0].retain);
}

#[gpui_kit::test]
fn mqtt_local_server_snapshot_reads_online_clients_and_subscriptions(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let snapshot = MqttBrokerSnapshot {
        topics: vec![MqttTopicObservation {
            name: "devices/one/state".into(),
            source: MqttTopicSource::Observed,
            retained: true,
            observed_at: Some(chrono::Utc::now()),
            publish_count: 4,
            subscriber_count: 1,
            last_payload_bytes: 18,
        }],
        online_clients: vec![MqttOnlineClient {
            client_id: "client-one".into(),
            username: Some("operator".into()),
            remote_address: Some("127.0.0.1:41000".into()),
            connected_at: Some(chrono::Utc::now()),
            subscriptions: vec![MqttSubscription {
                filter: "devices/#".into(),
                qos: MqttQos::AtLeastOnce,
                no_local: true,
            }],
        }],
        topics_complete: true,
        online_clients_complete: true,
        metrics: MqttBrokerMetrics {
            current_connections: 1,
            max_connections: 8,
            peak_connections: 2,
            accepted_connections: 3,
            closed_connections: 2,
            active_subscriptions: 1,
            published_messages: 4,
            retained_messages: 1,
            event_queue_depth: 2,
            event_queue_capacity: 256,
            command_queue_depth: 0,
            command_queue_capacity: 32,
            dropped_events: 0,
            dropped_topics: 0,
        },
    };
    let service = Arc::new(
        MqttService::new(Arc::new(NoopMqttDriver), Arc::new(NoopStorage::default()))
            .with_local_server_driver(Arc::new(RecordingLocalServerDriver {
                publishes: Arc::new(Mutex::new(Vec::new())),
                snapshot,
            })),
    );
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    visual_cx.simulate_resize(size(px(1024.0), px(768.0)));
    visual_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.section = MqttSection::LocalServer;
            view.local_server_status = Some(MqttLocalServerStatus::running(
                &MqttLocalServerConfig::default(),
            ));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    click(visual_cx, "mqtt-local-server-refresh-clients");
    visual_cx.run_until_parked();

    assert!(view.read_with(visual_cx, |view, _| {
        view.local_server_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.online_clients.len() == 1
                && snapshot.online_clients[0].client_id == "client-one"
                && snapshot.online_clients[0].subscriptions[0].filter == "devices/#"
                && snapshot.topics[0].name == "devices/one/state"
                && snapshot.metrics.current_connections == 1
                && snapshot.metrics.event_queue_depth == 2
        })
    }));
    for selector in [
        "mqtt-local-server-metrics",
        "mqtt-local-server-topics",
        "mqtt-local-server-topic-0",
        "mqtt-local-server-clients",
        "mqtt-local-server-client-0",
        "mqtt-local-server-publish-options",
    ] {
        let bounds = visual_cx
            .debug_bounds(selector)
            .expect("本地 Broker 快照和发布控件应参与布局");
        let main = visual_cx
            .debug_bounds("mqtt-main")
            .expect("MQTT 主工作区应参与布局");
        assert!(
            bounds.origin.x >= main.origin.x && bounds.right() <= main.right(),
            "本地 Broker 快照控件不能越出主工作区: selector={selector}, main={main:?}, bounds={bounds:?}"
        );
    }
}

#[gpui_kit::test]
fn mqtt_local_server_accounts_can_fill_the_client_form(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(NoopMqttDriver),
        Arc::new(NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    visual_cx.simulate_resize(size(px(1440.0), px(900.0)));

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.section = MqttSection::LocalServer;
            view.local_server_bind_host
                .update(cx, |input, cx| input.set_value("127.0.0.1", window, cx));
            view.local_server_port
                .update(cx, |input, cx| input.set_value("18884", window, cx));
            view.local_server_username
                .update(cx, |input, cx| input.set_value("operator", window, cx));
            view.local_server_password
                .update(cx, |input, cx| input.set_value("secret", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    scroll_local_server(visual_cx, -10000.0);
    click(visual_cx, "mqtt-local-server-add-user");
    visual_cx.run_until_parked();
    assert!(view.read_with(visual_cx, |view, _| {
        view.local_server_users.len() == 1
            && view.local_server_users[0].username == "operator"
            && view.local_server_users[0].password == "secret"
    }));

    scroll_local_server(visual_cx, 10000.0);
    click(visual_cx, "mqtt-local-server-use-client");
    visual_cx.run_until_parked();
    assert!(view.read_with(visual_cx, |view, cx| {
        view.section == MqttSection::Config
            && view.host.read(cx).value() == "127.0.0.1"
            && view.port.read(cx).value() == "18884"
            && view.username.read(cx).value() == "operator"
            && view.password.read(cx).value() == "secret"
    }));
}

#[gpui_kit::test]
fn mqtt_client_permissions_reflow_inside_supported_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(NoopMqttDriver),
        Arc::new(NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_kit::component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    let client = MosquittoClient {
        username: "operator".into(),
        client_id: None,
        password_configured: true,
        password: None,
        disabled: false,
        text_name: None,
        text_description: None,
        groups: Vec::new(),
        roles: vec![MosquittoRoleBinding {
            role_name: "reader".into(),
            priority: 10,
        }],
    };
    let snapshot = MosquittoDynamicSecuritySnapshot {
        clients: vec![client],
        groups: Vec::new(),
        roles: vec![MosquittoRole {
            role_name: "reader".into(),
            text_name: None,
            text_description: None,
            allow_wildcards_subscriptions: false,
            acls: vec![MosquittoAcl {
                acl_type: MosquittoAclType::SubscribeLiteral,
                topic: "devices/operator/state".into(),
                decision: MosquittoAclDecision::Allow,
                priority: 1,
            }],
        }],
    };

    view.update(visual_cx, |view, cx| {
        view.loading_profiles = false;
        view.section = MqttSection::Mosquitto;
        view.management_enabled = true;
        view.management_snapshot = Some(snapshot);
        view.management_section = MosquittoManagementSection::Clients;
        view.selected_client_username = Some("operator".into());
        cx.notify();
    });

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        let scroll = visual_cx
            .debug_bounds("mqtt-mosquitto-scroll")
            .expect("Mosquitto 内容区应参与布局");
        let permission = visual_cx
            .debug_bounds("mqtt-client-permission-row-0")
            .expect("用户权限行应参与布局");
        assert!(
            permission.origin.x >= scroll.origin.x && permission.right() <= scroll.right(),
            "{}px 窗口中的用户权限行不能越出内容区: scroll={scroll:?}, permission={permission:?}",
            width
        );
    }
}
