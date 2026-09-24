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
