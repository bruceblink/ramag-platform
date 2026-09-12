#[gpui::test]
fn jumpserver_rdp_button_opens_created_web_session(cx: &mut TestAppContext) {
    let (panel, cx) = add_jumpserver_panel_window(cx, service_with_jumpserver());
    cx.run_until_parked();
    let asset = JumpServerAsset {
        id: "00000000-0000-0000-0000-000000000001".into(),
        org_id: "org-1".into(),
        name: "windows".into(),
        address: "10.0.0.2".into(),
        platform: "Windows".into(),
        labels: Vec::new(),
        node_ids: Vec::new(),
        favorite: false,
        ungrouped: true,
        active: true,
    };
    let account = JumpServerAccount {
        id: "account-1".into(),
        alias: "account-1".into(),
        name: "admin".into(),
        username: "Administrator".into(),
        has_secret: true,
        can_connect: true,
    };
    panel.update(cx, |panel, cx| {
        let connection = JumpServerConnection::new(JumpServerCredential {
            base_url: "https://jump.example.com".into(),
            ssh_port: 2222,
            username: "alice".into(),
            password: "password".into(),
        });
        panel.selected_connection_id = Some(connection.id.clone());
        panel.connections = Arc::new(vec![connection]);
        panel.session = Some(JumpServerSession {
            base_url: "https://jump.example.com/".into(),
            ssh_host: "jump.example.com".into(),
            ssh_port: 2222,
            username: "alice".into(),
            password: "password".into(),
            token_keyword: "Bearer".into(),
            token: "api-token".into(),
            organizations: Vec::new(),
        });
        panel.assets = Arc::new(vec![asset.clone()]);
        panel.selected_asset_id = Some(asset.id.clone());
        panel.selected_account_id = Some(account.id.clone());
        panel.detail_error = Some("该资源未开放 SSH 协议，无法导入为 SSH 连接。".into());
        panel.detail = Some(JumpServerAssetDetail {
            asset,
            accounts: vec![account],
            ssh_enabled: false,
            rdp_web_enabled: true,
        });
        panel.operation = None;
        cx.notify();
    });
    cx.simulate_resize(size(px(920.0), px(820.0)));
    cx.run_until_parked();

    let button = cx
        .debug_bounds("jumpserver-inline-rdp-button-0")
        .expect("RDP 资产应显示远程桌面按钮");
    cx.simulate_click(button.center(), Modifiers::default());
    cx.run_until_parked();

    assert_eq!(
        cx.opened_url().as_deref(),
        Some("https://jump.example.com/lion/connect?token=00000000-0000-0000-0000-000000000002")
    );
}

#[gpui::test]
fn jumpserver_new_connection_shows_form_test_and_save_actions(cx: &mut TestAppContext) {
    let (_panel, cx) = add_jumpserver_panel_window(cx, service(Vec::new(), None));
    cx.run_until_parked();
    cx.simulate_resize(size(px(920.0), px(720.0)));
    cx.run_until_parked();

    for selector in [
        "jumpserver-new-connection-form",
        "jumpserver-url-field-input",
        "jumpserver-password-field-input",
        "test-jumpserver-connection",
        "save-jumpserver-connection",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector} 应参与布局");
    }
    assert!(cx.debug_bounds("load-jumpserver-assets").is_none());
}

#[gpui::test]
fn jumpserver_new_connection_form_stays_inside_compact_window(cx: &mut TestAppContext) {
    let (_, cx) = add_jumpserver_panel_window(cx, service(Vec::new(), None));
    cx.simulate_resize(size(px(360.0), px(240.0)));
    cx.run_until_parked();

    let form = cx
        .debug_bounds("jumpserver-new-connection-form")
        .expect("紧凑窗口应显示 JumpServer 新建连接表单");
    assert!(form.origin.x >= px(0.0) && form.right() <= px(360.0));
    for selector in [
        "jumpserver-url-field-input",
        "jumpserver-ssh-port-field-input",
        "jumpserver-username-field-input",
        "jumpserver-password-field-input",
        "test-jumpserver-connection",
        "save-jumpserver-connection",
    ] {
        let bounds = cx
            .debug_bounds(selector)
            .expect("JumpServer 表单控件应在紧凑窗口中参与布局");
        assert!(
            bounds.origin.x >= px(0.0) && bounds.right() <= px(360.0),
            "{selector} 越出窗口：{bounds:?}"
        );
    }
}

#[gpui::test]
fn jumpserver_saved_connection_edit_reuses_connection_form(cx: &mut TestAppContext) {
    let (panel, cx) = add_jumpserver_panel_window(cx, service(Vec::new(), None));
    cx.run_until_parked();
    panel.update(cx, |panel, cx| {
        let connection = JumpServerConnection::new(JumpServerCredential {
            base_url: "https://jump.example.com".into(),
            ssh_port: 2222,
            username: "alice".into(),
            password: "password".into(),
        });
        panel.selected_connection_id = Some(connection.id.clone());
        panel.connections = Arc::new(vec![connection]);
        panel.editing_connection = true;
        cx.notify();
    });
    cx.simulate_resize(size(px(920.0), px(720.0)));
    cx.run_until_parked();

    for selector in [
        "jumpserver-new-connection-form",
        "test-jumpserver-connection",
        "save-jumpserver-connection",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector} 应参与布局");
    }
}

#[gpui::test]
fn jumpserver_catalog_defaults_to_organization_with_assets(cx: &mut TestAppContext) {
    let (panel, cx) = add_jumpserver_panel_window(cx, service(Vec::new(), None));
    cx.run_until_parked();
    panel.update(cx, |panel, _| {
        panel.apply_catalog(JumpServerCatalog {
            assets: vec![JumpServerAsset {
                id: "00000000-0000-0000-0000-000000000001".into(),
                org_id: "org-default".into(),
                name: "server".into(),
                address: "10.0.0.1".into(),
                platform: "Linux".into(),
                labels: Vec::new(),
                node_ids: vec!["node-default".into()],
                favorite: false,
                ungrouped: false,
                active: true,
            }],
            nodes: vec![
                ramag_domain::entities::JumpServerNode {
                    id: "node-empty".into(),
                    org_id: "org-all".into(),
                    key: "1".into(),
                    name: "All Organizations".into(),
                    full_name: "All Organizations".into(),
                    assets_amount: 0,
                },
                ramag_domain::entities::JumpServerNode {
                    id: "node-default".into(),
                    org_id: "org-default".into(),
                    key: "1".into(),
                    name: "DEFAULT".into(),
                    full_name: "DEFAULT".into(),
                    assets_amount: 1,
                },
            ],
        });
    });

    panel.read_with(cx, |panel, _| {
        assert_eq!(panel.filtered_assets().len(), 1);
        assert_eq!(
            panel.selected_tree_item,
            crate::views::jumpserver_dialog::JumpServerTreeSelection::Node {
                org_id: "org-default".into(),
                node_id: "node-default".into(),
            }
        );
    });
}
