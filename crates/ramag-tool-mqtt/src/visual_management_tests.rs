use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use gpui::{AppContext as _, Modifiers, TestAppContext, VisualTestContext, point, px, size};
use ramag_app::MqttService;
use ramag_domain::entities::{
    MosquittoAcl, MosquittoAclDecision, MosquittoAclType, MosquittoClient,
    MosquittoDynamicSecuritySnapshot, MosquittoGroup, MosquittoGroupBinding, MosquittoRole,
    MosquittoRoleBinding, MqttProfile,
};
use ramag_domain::error::Result;
use ramag_domain::traits::MosquittoDynamicSecurityDriver;

use super::visual_tests::{MqttTestHost, NoopMqttDriver, NoopStorage};
use super::{MosquittoManagementSection, MqttSection, MqttView};

struct RecordingManagementDriver {
    snapshot: MosquittoDynamicSecuritySnapshot,
    snapshot_calls: Arc<AtomicUsize>,
    saved_clients: Arc<Mutex<Vec<MosquittoClient>>>,
    saved_groups: Arc<Mutex<Vec<MosquittoGroup>>>,
    saved_roles: Arc<Mutex<Vec<MosquittoRole>>>,
}

#[async_trait]
impl MosquittoDynamicSecurityDriver for RecordingManagementDriver {
    async fn snapshot(&self, _profile: &MqttProfile) -> Result<MosquittoDynamicSecuritySnapshot> {
        self.snapshot_calls.fetch_add(1, Ordering::Relaxed);
        Ok(self.snapshot.clone())
    }

    async fn save_client(&self, _profile: &MqttProfile, client: &MosquittoClient) -> Result<()> {
        self.saved_clients
            .lock()
            .expect("记录用户保存请求锁")
            .push(client.clone());
        Ok(())
    }

    async fn save_group(&self, _profile: &MqttProfile, group: &MosquittoGroup) -> Result<()> {
        self.saved_groups
            .lock()
            .expect("记录 Group 保存请求锁")
            .push(group.clone());
        Ok(())
    }

    async fn save_role(&self, _profile: &MqttProfile, role: &MosquittoRole) -> Result<()> {
        self.saved_roles
            .lock()
            .expect("记录 Role 保存请求锁")
            .push(role.clone());
        Ok(())
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
fn mqtt_management_editors_reflow_inside_supported_window_widths(cx: &mut TestAppContext) {
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
    let snapshot = MosquittoDynamicSecuritySnapshot {
        clients: vec![MosquittoClient {
            username: "operator".into(),
            client_id: None,
            password_configured: true,
            password: None,
            disabled: false,
            text_name: None,
            text_description: None,
            groups: vec![MosquittoGroupBinding {
                group_name: "operations".into(),
                priority: 10,
            }],
            roles: vec![MosquittoRoleBinding {
                role_name: "reader".into(),
                priority: 10,
            }],
        }],
        groups: vec![MosquittoGroup {
            group_name: "operations".into(),
            text_name: Some("Operations".into()),
            text_description: None,
            roles: vec![MosquittoRoleBinding {
                role_name: "reader".into(),
                priority: 10,
            }],
        }],
        roles: vec![MosquittoRole {
            role_name: "reader".into(),
            text_name: Some("Reader".into()),
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
        view.selected_client_username = Some("operator".into());
        view.selected_group_name = Some("operations".into());
        view.selected_role_name = Some("reader".into());
        cx.notify();
    });

    for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        for (section, editor_selector, fields_selector) in [
            (
                MosquittoManagementSection::Clients,
                "mqtt-client-editor",
                "mqtt-client-identity-fields",
            ),
            (
                MosquittoManagementSection::Groups,
                "mqtt-group-editor",
                "mqtt-group-fields",
            ),
            (
                MosquittoManagementSection::Roles,
                "mqtt-role-editor",
                "mqtt-role-fields",
            ),
        ] {
            view.update(visual_cx, |view, cx| {
                view.management_section = section;
                cx.notify();
            });
            visual_cx.run_until_parked();
            let scroll = visual_cx
                .debug_bounds("mqtt-mosquitto-scroll")
                .expect("Mosquitto 内容区应参与布局");
            let editor = visual_cx
                .debug_bounds(editor_selector)
                .expect("管理编辑器应参与布局");
            let fields = visual_cx
                .debug_bounds(fields_selector)
                .expect("管理字段行应参与布局");
            assert!(
                editor.origin.x >= scroll.origin.x && editor.right() <= scroll.right(),
                "{}px 窗口中的 {} 编辑器不能越出内容区: scroll={scroll:?}, editor={editor:?}",
                width,
                editor_selector
            );
            assert!(
                fields.origin.x >= scroll.origin.x && fields.right() <= scroll.right(),
                "{}px 窗口中的 {} 字段行不能越出内容区: scroll={scroll:?}, fields={fields:?}",
                width,
                fields_selector
            );
        }
    }
}

#[gpui::test]
fn mqtt_management_buttons_send_changes_to_dynamic_security_driver(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let snapshot = MosquittoDynamicSecuritySnapshot {
        clients: Vec::new(),
        groups: Vec::new(),
        roles: Vec::new(),
    };
    let snapshot_calls = Arc::new(AtomicUsize::new(0));
    let saved_clients = Arc::new(Mutex::new(Vec::new()));
    let saved_groups = Arc::new(Mutex::new(Vec::new()));
    let saved_roles = Arc::new(Mutex::new(Vec::new()));
    let service = Arc::new(
        MqttService::new(Arc::new(NoopMqttDriver), Arc::new(NoopStorage::default()))
            .with_dynamic_security_driver(Arc::new(RecordingManagementDriver {
                snapshot: snapshot.clone(),
                snapshot_calls: snapshot_calls.clone(),
                saved_clients: saved_clients.clone(),
                saved_groups: saved_groups.clone(),
                saved_roles: saved_roles.clone(),
            })),
    );
    let mut view_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MqttView::new(service, window, cx));
        view_entity = Some(view.clone());
        let host = cx.new(|_| MqttTestHost { view });
        gpui_component::Root::new(host, window, cx)
    });
    let view = view_entity.expect("MQTT 视图应初始化");
    let mut profile = MqttProfile::new("Dynamic Security UI", "127.0.0.1", 18883);
    profile.management.enabled = true;

    // 这个 headless 场景走真实页面按钮和 Service，管理驱动只记录最终请求对象。
    view.update(visual_cx, |view, cx| {
        view.loading_profiles = false;
        view.profiles = vec![profile.clone()];
        view.selected_profile_id = Some(profile.id.clone());
        view.management_enabled = true;
        view.management_snapshot = Some(snapshot.clone());
        view.section = MqttSection::Mosquitto;
        view.management_section = MosquittoManagementSection::Clients;
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(1440.0), px(900.0)));
    visual_cx.run_until_parked();

    click(visual_cx, "mqtt-load-management");
    visual_cx.run_until_parked();
    assert_eq!(snapshot_calls.load(Ordering::Relaxed), 1);

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.client_username
                .update(cx, |input, cx| input.set_value("operator", window, cx));
            view.client_password
                .update(cx, |input, cx| input.set_value("secret", window, cx));
            view.client_groups
                .update(cx, |input, cx| input.set_value("operations", window, cx));
            view.client_roles
                .update(cx, |input, cx| input.set_value("reader", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();
    click(visual_cx, "mqtt-client-save");
    visual_cx.run_until_parked();
    assert_eq!(
        saved_clients
            .lock()
            .expect("读取用户保存请求锁")
            .last()
            .map(|client| (
                client.username.clone(),
                client.password.clone(),
                client.groups[0].group_name.clone(),
                client.roles[0].role_name.clone(),
            )),
        Some((
            "operator".into(),
            Some("secret".into()),
            "operations".into(),
            "reader".into(),
        )),
    );

    click(visual_cx, "mqtt-management-tab-Groups");
    visual_cx.run_until_parked();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.group_name_editor
                .update(cx, |input, cx| input.set_value("operations", window, cx));
            view.group_roles
                .update(cx, |input, cx| input.set_value("reader", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();
    click(visual_cx, "mqtt-group-save");
    visual_cx.run_until_parked();
    assert_eq!(
        saved_groups
            .lock()
            .expect("读取 Group 保存请求锁")
            .last()
            .map(|group| (group.group_name.clone(), group.roles[0].role_name.clone())),
        Some(("operations".into(), "reader".into())),
    );

    click(visual_cx, "mqtt-management-tab-Roles");
    visual_cx.run_until_parked();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.role_name_editor
                .update(cx, |input, cx| input.set_value("reader", window, cx));
            view.role_acls.update(cx, |input, cx| {
                input.set_value("subscribeLiteral|devices/#|allow|1", window, cx)
            });
            cx.notify();
        });
    });
    visual_cx.run_until_parked();
    click(visual_cx, "mqtt-role-save");
    visual_cx.run_until_parked();
    assert_eq!(
        saved_roles
            .lock()
            .expect("读取 Role 保存请求锁")
            .last()
            .map(|role| (
                role.role_name.clone(),
                role.acls[0].topic.clone(),
                role.acls[0].decision,
            )),
        Some((
            "reader".into(),
            "devices/#".into(),
            MosquittoAclDecision::Allow,
        )),
    );
}
