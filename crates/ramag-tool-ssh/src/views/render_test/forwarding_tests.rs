use super::*;

#[gpui_kit::test]
fn edit_profile_form_keeps_fields_and_ssh_command_parser(cx: &mut TestAppContext) {
    let mut saved_profile = profile();
    saved_profile.port_forwardings = vec![SshPortForward::Local {
        bind_address: Some("127.0.0.1".into()),
        listen_port: 8080,
        target_host: "db.internal".into(),
        target_port: 5432,
    }];
    let (form, cx) =
        add_ssh_form_window_with_profile(cx, service(Vec::new(), None), Some(saved_profile));
    cx.simulate_resize(size(px(720.0), px(800.0)));
    cx.run_until_parked();

    form.read_with(cx, |form, _| assert_eq!(form.title(), "编辑"));
    assert!(
        cx.debug_bounds("ssh-profile-host-field-input").is_some(),
        "编辑连接应保留标准 SSH 字段"
    );
    assert!(
        cx.debug_bounds("ssh-command-input").is_some(),
        "编辑连接也应提供 SSH 命令解析入口"
    );
    assert!(
        cx.debug_bounds("ssh-profile-port-forwardings").is_some(),
        "编辑连接应显示已配置的端口转发"
    );
    assert!(
        cx.debug_bounds("ssh-profile-port-forwardings-summary")
            .is_some(),
        "端口转发摘要应参与布局"
    );
}

#[gpui_kit::test]
fn workspace_exposes_independent_port_forwarding_controls(cx: &mut TestAppContext) {
    let mut saved_profile = profile();
    saved_profile.port_forwardings = vec![SshPortForward::Dynamic {
        bind_address: Some("127.0.0.1".into()),
        listen_port: 1080,
    }];
    let profile_id = saved_profile.id.clone();
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile_id.clone(),
            last_remote_path: "/home/alice".into(),
        }],
        active_profile_id: Some(profile_id.clone()),
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(cx, service(vec![saved_profile], Some(preference)));
    cx.simulate_resize(size(px(1024.0), px(768.0)));
    cx.run_until_parked();

    assert!(
        cx.debug_bounds("ssh-port-forwarding-panel").is_some(),
        "工作区应显示独立端口转发面板"
    );
    assert!(
        cx.debug_bounds("ssh-port-forwarding-row-0").is_some(),
        "转发面板应显示每条转发"
    );
    assert!(
        cx.debug_bounds("start-ssh-port-forwarding").is_some(),
        "停止状态应提供启动入口"
    );

    view.update(cx, |view, cx| {
        assert!(view.port_forward_manager.begin_start(&profile_id));
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("stop-ssh-port-forwarding").is_some(),
        "启动中的转发应提供停止入口"
    );
}
