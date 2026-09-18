use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_channel::{Receiver, Sender, bounded};
use async_trait::async_trait;
use chrono::Utc;
use gpui::{
    AppContext as _, Context, IntoElement, Modifiers, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, point, px, size,
};
use ramag_app::MqttService;
use ramag_domain::entities::{
    MqttMessage, MqttMessageSink, MqttProfile, MqttPublishRequest, MqttPublishResult, MqttQos,
    MqttSubscribeRequest, MqttSubscriptionStatusSink,
};
use ramag_domain::error::Result;
use ramag_domain::traits::MqttDriver;

use super::{MqttSection, MqttView};

struct OptionsMqttDriver {
    publish_requests: Arc<Mutex<Vec<MqttPublishRequest>>>,
    subscribe_requests: Arc<Mutex<Vec<MqttSubscribeRequest>>>,
    publish_profiles: Arc<Mutex<Vec<MqttProfile>>>,
    subscribe_profiles: Arc<Mutex<Vec<MqttProfile>>>,
}

#[async_trait]
impl MqttDriver for OptionsMqttDriver {
    async fn publish(
        &self,
        profile: &MqttProfile,
        request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
        self.publish_profiles
            .lock()
            .expect("记录发布配置锁")
            .push(profile.clone());
        self.publish_requests
            .lock()
            .expect("记录发布请求锁")
            .push(request.clone());
        Ok(MqttPublishResult {
            topic: request.topic.clone(),
            packet_id: Some(1),
            qos: request.qos,
        })
    }

    async fn subscribe(
        &self,
        profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        _sink: MqttMessageSink,
        _status_sink: MqttSubscriptionStatusSink,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        self.subscribe_profiles
            .lock()
            .expect("记录订阅配置锁")
            .push(profile.clone());
        self.subscribe_requests
            .lock()
            .expect("记录订阅请求锁")
            .push(request.clone());
        Ok(())
    }
}

struct BlockingSubscriptionDriver {
    started: Sender<()>,
    release: Receiver<()>,
}

#[async_trait]
impl MqttDriver for BlockingSubscriptionDriver {
    async fn subscribe(
        &self,
        _profile: &MqttProfile,
        _request: &MqttSubscribeRequest,
        _sink: MqttMessageSink,
        _status_sink: MqttSubscriptionStatusSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        self.started.send(()).await.expect("订阅开始信号应可发送");
        self.release.recv().await.expect("订阅释放信号应可接收");
        assert!(cancelled.load(Ordering::Acquire));
        Ok(())
    }
}

struct TestHost {
    view: gpui::Entity<MqttView>,
}

impl Render for TestHost {
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
fn mqtt_message_options_reach_publish_and_subscribe_requests(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let publish_requests = Arc::new(Mutex::new(Vec::new()));
    let subscribe_requests = Arc::new(Mutex::new(Vec::new()));
    let publish_profiles = Arc::new(Mutex::new(Vec::new()));
    let subscribe_profiles = Arc::new(Mutex::new(Vec::new()));
    let driver = Arc::new(OptionsMqttDriver {
        publish_requests: publish_requests.clone(),
        subscribe_requests: subscribe_requests.clone(),
        publish_profiles: publish_profiles.clone(),
        subscribe_profiles: subscribe_profiles.clone(),
    });
    let service = Arc::new(MqttService::new(
        driver,
        Arc::new(super::visual_tests::NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| TestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    let profile = MqttProfile::new("消息选项测试", "127.0.0.1", 1883);

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.profiles = vec![profile.clone()];
            view.selected_profile_id = Some(profile.id.clone());
            view.set_form_from_profile(&profile, window, cx);
            view.host
                .update(cx, |input, cx| input.set_value("198.51.100.10", window, cx));
            view.section = MqttSection::Publish;
            view.publish_topic
                .update(cx, |input, cx| input.set_value("devices/state", window, cx));
            view.publish_payload
                .update(cx, |input, cx| input.set_value("6f6e6c696e65", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    click(visual_cx, "mqtt-publish-payload-format-Hex");
    click(visual_cx, "mqtt-publish-qos-2");
    click(visual_cx, "mqtt-publish-retain");
    click(visual_cx, "mqtt-publish");
    visual_cx.run_until_parked();
    let publish = publish_requests
        .lock()
        .expect("读取发布请求锁")
        .last()
        .cloned()
        .expect("发布按钮应调用 MQTT 驱动");
    assert_eq!(publish.topic, "devices/state");
    assert_eq!(publish.payload, b"online");
    assert_eq!(publish.qos, MqttQos::ExactlyOnce);
    assert!(publish.retain, "Retain 开关应进入发布请求");
    assert_eq!(
        publish_profiles
            .lock()
            .expect("读取发布配置锁")
            .last()
            .expect("发布应带配置")
            .host,
        "198.51.100.10",
        "发布应使用尚未保存的 Broker 地址"
    );

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.section = MqttSection::Subscribe;
            view.subscribe_filter
                .update(cx, |input, cx| input.set_value("devices/#", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();
    click(visual_cx, "mqtt-subscribe-qos-2");
    click(visual_cx, "mqtt-subscribe-no-local");
    click(visual_cx, "mqtt-add-subscription");
    click(visual_cx, "mqtt-subscription-1-qos-1");
    click(visual_cx, "mqtt-start-subscription");
    visual_cx.run_until_parked();
    let subscribe = subscribe_requests
        .lock()
        .expect("读取订阅请求锁")
        .last()
        .cloned()
        .expect("开始订阅按钮应调用 MQTT 驱动");
    let added_subscription = subscribe
        .subscriptions
        .iter()
        .find(|subscription| subscription.filter == "devices/#")
        .expect("新增 Topic 应进入订阅请求");
    assert_eq!(added_subscription.qos, MqttQos::AtLeastOnce);
    assert!(added_subscription.no_local);
    assert_eq!(
        subscribe_profiles
            .lock()
            .expect("读取订阅配置锁")
            .last()
            .expect("订阅应带配置")
            .host,
        "198.51.100.10",
        "订阅应使用尚未保存的 Broker 地址"
    );
}

#[gpui::test]
fn mqtt_subscription_stays_stopping_until_driver_returns(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (started_sender, started_receiver) = bounded(1);
    let (release_sender, release_receiver) = bounded(1);
    let driver = Arc::new(BlockingSubscriptionDriver {
        started: started_sender,
        release: release_receiver,
    });
    let service = Arc::new(MqttService::new(
        driver,
        Arc::new(super::visual_tests::NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| TestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    let profile = MqttProfile::new("停止订阅测试", "127.0.0.1", 1883);
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.profiles = vec![profile.clone()];
            view.selected_profile_id = Some(profile.id.clone());
            view.set_form_from_profile(&profile, window, cx);
            view.section = MqttSection::Subscribe;
            view.subscribe_filter
                .update(cx, |input, cx| input.set_value("devices/#", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    click(visual_cx, "mqtt-add-subscription");
    click(visual_cx, "mqtt-start-subscription");
    visual_cx.run_until_parked();
    assert!(started_receiver.try_recv().is_ok(), "订阅驱动应已开始");

    click(visual_cx, "mqtt-stop-subscription");
    visual_cx.run_until_parked();
    assert!(view.read_with(visual_cx, |view, _| {
        view.subscription_running && view.subscription_stopping
    }));

    release_sender.try_send(()).expect("订阅释放信号应可发送");
    visual_cx.run_until_parked();
    assert!(view.read_with(visual_cx, |view, _| {
        !view.subscription_running && !view.subscription_stopping
    }));
}

#[gpui::test]
fn mqtt_message_controls_keep_inputs_and_actions_bounded(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(super::visual_tests::NoopMqttDriver),
        Arc::new(super::visual_tests::NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| TestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    visual_cx.simulate_resize(size(px(1440.0), px(900.0)));
    view.update(visual_cx, |view, cx| {
        view.loading_profiles = false;
        view.section = MqttSection::Publish;
        cx.notify();
    });
    visual_cx.run_until_parked();

    let main = visual_cx.debug_bounds("mqtt-main").expect("主工作区应渲染");
    for selector in ["mqtt-publish-topic-input", "mqtt-subscribe-filter-input"] {
        if selector == "mqtt-subscribe-filter-input" {
            click(visual_cx, "mqtt-tab-Subscribe");
            visual_cx.run_until_parked();
        }
        let bounds = visual_cx
            .debug_bounds(selector)
            .expect("Topic 输入框应参与布局");
        assert!(bounds.size.width >= px(180.0) && bounds.right() <= main.right());
    }
    let actions = visual_cx
        .debug_bounds("mqtt-subscribe-actions")
        .expect("订阅操作区应参与布局");
    assert!(actions.size.width < main.size.width / 2.0);
    let button = visual_cx
        .debug_bounds("mqtt-start-subscription")
        .expect("开始订阅按钮应参与布局");
    assert!(
        button.size.width < main.size.width / 2.0,
        "开始订阅按钮不能撑满消息区域: main={main:?}, button={button:?}"
    );

    visual_cx.simulate_resize(size(px(360.0), px(640.0)));
    visual_cx.run_until_parked();
    let narrow_main = visual_cx
        .debug_bounds("mqtt-main")
        .expect("窄窗口主工作区应渲染");
    let narrow_actions = visual_cx
        .debug_bounds("mqtt-subscribe-actions")
        .expect("窄窗口订阅操作区应参与布局");
    let narrow_button = visual_cx
        .debug_bounds("mqtt-start-subscription")
        .expect("窄窗口开始订阅按钮应参与布局");
    assert!(
        narrow_button.size.width < narrow_main.size.width,
        "窄窗口开始订阅按钮不能撑满主工作区: main={narrow_main:?}, button={narrow_button:?}"
    );
    assert!(narrow_button.size.width <= narrow_actions.size.width);

    click(visual_cx, "mqtt-tab-Publish");
    visual_cx.run_until_parked();
    let narrow_publish_actions = visual_cx
        .debug_bounds("mqtt-publish-actions")
        .expect("窄窗口发布操作区应参与布局");
    let narrow_publish_button = visual_cx
        .debug_bounds("mqtt-publish")
        .expect("窄窗口发布按钮应参与布局");
    assert!(
        narrow_publish_button.size.width < narrow_main.size.width,
        "窄窗口发布按钮不能撑满主工作区: main={narrow_main:?}, button={narrow_publish_button:?}"
    );
    assert!(narrow_publish_button.size.width <= narrow_publish_actions.size.width);
}

#[gpui::test]
fn mqtt_subscription_topics_can_be_added_and_removed(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(super::visual_tests::NoopMqttDriver),
        Arc::new(super::visual_tests::NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| TestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.section = MqttSection::Subscribe;
            view.subscribe_filter
                .update(cx, |input, cx| input.set_value("alerts/#", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    click(visual_cx, "mqtt-add-subscription");
    assert!(view.read_with(visual_cx, |view, _| {
        view.subscription_topics.len() == 2 && view.subscription_topics[1].filter == "alerts/#"
    }));

    click(visual_cx, "mqtt-remove-subscription-1");
    assert!(view.read_with(visual_cx, |view, _| {
        view.subscription_topics.len() == 1 && view.subscription_topics[0].filter == "+/#"
    }));
}

#[gpui::test]
fn mqtt_subscription_topics_are_saved_and_restored_with_profile(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let storage = Arc::new(super::visual_tests::NoopStorage::default());
    let service = Arc::new(MqttService::new(
        Arc::new(super::visual_tests::NoopMqttDriver),
        storage.clone(),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| TestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    visual_cx.run_until_parked();

    let profile = MqttProfile::new("持久化订阅测试", "127.0.0.1", 1883);
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.profiles = vec![profile.clone()];
            view.selected_profile_id = Some(profile.id.clone());
            view.set_form_from_profile(&profile, window, cx);
            view.section = MqttSection::Subscribe;
            view.subscribe_filter
                .update(cx, |input, cx| input.set_value("alerts/#", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    click(visual_cx, "mqtt-add-subscription");
    click(visual_cx, "mqtt-tab-Config");
    visual_cx.run_until_parked();
    click(visual_cx, "mqtt-save-profile");
    visual_cx.run_until_parked();

    let saved = storage
        .mqtt_profiles
        .lock()
        .expect("读取保存结果锁")
        .last()
        .cloned()
        .expect("保存应写入 MQTT 配置");
    assert!(
        saved
            .subscriptions
            .iter()
            .any(|subscription| subscription.filter == "alerts/#")
    );

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.set_form_from_profile(&saved, window, cx);
            view.section = MqttSection::Subscribe;
            cx.notify();
        });
    });
    assert!(view.read_with(visual_cx, |view, _| {
        view.subscription_topics == saved.subscriptions
    }));
}

#[gpui::test]
fn mqtt_message_timeline_can_pause_and_clear_without_stopping_subscription(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(super::visual_tests::NoopMqttDriver),
        Arc::new(super::visual_tests::NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| TestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    view.update(visual_cx, |view, cx| {
        view.loading_profiles = false;
        view.section = MqttSection::Subscribe;
        view.subscription_running = true;
        view.messages.push_back(MqttMessage {
            topic: "devices/state".into(),
            payload: b"online".to_vec(),
            qos: MqttQos::AtMostOnce,
            retain: false,
            duplicate: false,
            received_at: Utc::now(),
            user_properties: Vec::new(),
        });
        cx.notify();
    });

    for width in [360.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(640.0)));
        visual_cx.run_until_parked();
        let main = visual_cx
            .debug_bounds("mqtt-main")
            .expect("MQTT 主工作区应渲染");
        let actions = visual_cx
            .debug_bounds("mqtt-message-timeline-actions")
            .expect("消息时间线操作区应渲染");
        assert!(
            actions.origin.x >= main.origin.x && actions.right() <= main.right(),
            "{}px 窗口中的消息时间线操作区不能越界: main={main:?}, actions={actions:?}",
            width
        );
    }

    click(visual_cx, "mqtt-message-timeline-pause");
    assert!(view.read_with(visual_cx, |view, _| {
        view.message_timeline_paused && view.subscription_running && view.messages.len() == 1
    }));

    click(visual_cx, "mqtt-message-timeline-clear");
    assert!(view.read_with(visual_cx, |view, _| {
        view.messages.is_empty() && view.subscription_running && view.message_timeline_paused
    }));

    click(visual_cx, "mqtt-message-timeline-pause");
    assert!(view.read_with(visual_cx, |view, _| {
        !view.message_timeline_paused && view.subscription_running
    }));

    view.update(visual_cx, |view, cx| {
        assert!(view.append_received_message(MqttMessage {
            topic: "devices/state".into(),
            payload: b"resumed".to_vec(),
            qos: MqttQos::AtMostOnce,
            retain: false,
            duplicate: false,
            received_at: Utc::now(),
            user_properties: Vec::new(),
        }));
        cx.notify();
    });
    assert!(view.read_with(visual_cx, |view, _| {
        !view.message_timeline_paused && view.subscription_running && view.messages.len() == 1
    }));
}

#[gpui::test]
fn mqtt_message_viewer_stays_inside_supported_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let service = Arc::new(MqttService::new(
        Arc::new(super::visual_tests::NoopMqttDriver),
        Arc::new(super::visual_tests::NoopStorage::default()),
    ));
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| TestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    let message = MqttMessage {
        topic: "devices/state".into(),
        payload: br#"{"online":true}"#.to_vec(),
        qos: MqttQos::AtLeastOnce,
        retain: true,
        duplicate: false,
        received_at: Utc::now(),
        user_properties: Vec::new(),
    };
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.loading_profiles = false;
            view.section = MqttSection::Subscribe;
            view.open_message_viewer(message, window, cx);
        });
    });

    for width in [360.0, 1024.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(640.0)));
        visual_cx.run_until_parked();
        let root = visual_cx
            .debug_bounds("mqtt-root")
            .expect("MQTT 根布局应渲染");
        let viewer = visual_cx
            .debug_bounds("mqtt-message-viewer")
            .expect("消息查看器应渲染");
        let content = visual_cx
            .debug_bounds("mqtt-message-viewer-content-frame")
            .expect("消息查看器内容区应渲染");
        assert!(viewer.origin.x >= root.origin.x && viewer.right() <= root.right());
        assert!(content.origin.x >= viewer.origin.x && content.right() <= viewer.right());
    }
    click(visual_cx, "mqtt-message-viewer-format-JSON");
    visual_cx.run_until_parked();
    assert!(
        visual_cx
            .debug_bounds("mqtt-message-viewer-json-node-root")
            .is_some()
    );
    assert!(
        visual_cx
            .debug_bounds("mqtt-message-viewer-json-node-0")
            .is_some()
    );
    click(visual_cx, "mqtt-message-viewer-json-node-root");
    visual_cx.run_until_parked();
    assert!(
        visual_cx
            .debug_bounds("mqtt-message-viewer-json-node-0")
            .is_none()
    );
    click(visual_cx, "mqtt-message-viewer-json-node-root");
    click(visual_cx, "mqtt-message-viewer-copy-topic");
    click(visual_cx, "mqtt-message-viewer-copy-payload");
}
