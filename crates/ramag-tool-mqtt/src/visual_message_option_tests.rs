use std::sync::{Arc, Mutex, atomic::AtomicBool};

use async_trait::async_trait;
use gpui::{
    AppContext as _, Context, IntoElement, Modifiers, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, point,
};
use ramag_app::MqttService;
use ramag_domain::entities::{
    MqttMessageSink, MqttProfile, MqttPublishRequest, MqttPublishResult, MqttQos,
    MqttSubscribeRequest,
};
use ramag_domain::error::Result;
use ramag_domain::traits::MqttDriver;

use super::{MqttSection, MqttView};

struct OptionsMqttDriver {
    publish_requests: Arc<Mutex<Vec<MqttPublishRequest>>>,
    subscribe_requests: Arc<Mutex<Vec<MqttSubscribeRequest>>>,
}

#[async_trait]
impl MqttDriver for OptionsMqttDriver {
    async fn publish(
        &self,
        _profile: &MqttProfile,
        request: &MqttPublishRequest,
    ) -> Result<MqttPublishResult> {
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
        _profile: &MqttProfile,
        request: &MqttSubscribeRequest,
        _sink: MqttMessageSink,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        self.subscribe_requests
            .lock()
            .expect("记录订阅请求锁")
            .push(request.clone());
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
    let driver = Arc::new(OptionsMqttDriver {
        publish_requests: publish_requests.clone(),
        subscribe_requests: subscribe_requests.clone(),
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
            view.section = MqttSection::Publish;
            view.publish_topic
                .update(cx, |input, cx| input.set_value("devices/state", window, cx));
            view.publish_payload
                .update(cx, |input, cx| input.set_value("online", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

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
    click(visual_cx, "mqtt-start-subscription");
    visual_cx.run_until_parked();
    let subscribe = subscribe_requests
        .lock()
        .expect("读取订阅请求锁")
        .last()
        .cloned()
        .expect("开始订阅按钮应调用 MQTT 驱动");
    assert_eq!(subscribe.subscriptions[0].filter, "devices/#");
    assert_eq!(subscribe.subscriptions[0].qos, MqttQos::ExactlyOnce);
}
