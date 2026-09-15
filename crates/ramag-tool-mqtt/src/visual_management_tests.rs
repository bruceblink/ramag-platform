use std::sync::Arc;

use gpui::{AppContext as _, TestAppContext, px, size};
use ramag_app::MqttService;
use ramag_domain::entities::{
    MosquittoAcl, MosquittoAclDecision, MosquittoAclType, MosquittoClient,
    MosquittoDynamicSecuritySnapshot, MosquittoGroup, MosquittoGroupBinding, MosquittoRole,
    MosquittoRoleBinding,
};

use super::visual_tests::{MqttTestHost, NoopMqttDriver, NoopStorage};
use super::{MosquittoManagementSection, MqttSection, MqttView};

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
