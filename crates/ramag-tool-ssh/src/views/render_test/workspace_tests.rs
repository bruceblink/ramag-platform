use super::*;

#[test]
fn jumpserver_unavailable_account_message_explains_connect_permission() {
    let detail = JumpServerAssetDetail {
        asset: JumpServerAsset {
            id: "00000000-0000-0000-0000-000000000001".into(),
            org_id: "org-1".into(),
            name: "server".into(),
            address: "10.0.0.1".into(),
            platform: "Linux".into(),
            labels: Vec::new(),
            node_ids: Vec::new(),
            favorite: false,
            ungrouped: false,
            active: true,
        },
        accounts: vec![JumpServerAccount {
            id: "account-1".into(),
            alias: "account-1".into(),
            name: "root".into(),
            username: "root".into(),
            has_secret: true,
            can_connect: false,
        }],
        ssh_enabled: true,
        rdp_web_enabled: false,
    };

    let message = crate::views::jumpserver_dialog::detail_unavailable_message(&detail)
        .expect("unavailable detail should explain the reason");
    assert!(message.contains("connect 权限"));
}

#[gpui_kit::test]
fn profile_form_inputs_keep_dialog_width_instead_of_collapsing(cx: &mut TestAppContext) {
    let (form, cx) = add_ssh_form_window(cx, service(Vec::new(), None));
    cx.simulate_resize(size(px(720.0), px(800.0)));
    cx.run_until_parked();

    let name = cx
        .debug_bounds("ssh-profile-name-field-input")
        .expect("name input container should be rendered");
    let host = cx
        .debug_bounds("ssh-profile-host-field-input")
        .expect("host input container should be rendered");
    let port = cx
        .debug_bounds("ssh-profile-port-field-input")
        .expect("port input container should be rendered");
    assert!(
        cx.debug_bounds("ssh-command-input").is_some(),
        "新增连接应提供 SSH 命令解析入口"
    );
    assert!(
        name.size.width > px(300.0),
        "名称输入框宽度异常：{:?}",
        name.size
    );
    assert!(
        host.size.width > px(300.0),
        "主机输入框宽度异常：{:?}",
        host.size
    );
    assert!(
        port.size.width >= px(100.0),
        "端口输入框宽度异常：{:?}",
        port.size
    );
    assert!(
        cx.debug_bounds("ssh-profile-executable-field-input")
            .is_some(),
        "高级选项应默认展开"
    );
    let openssh_status = cx
        .debug_bounds("ssh-openssh-status")
        .expect("OpenSSH 状态应显示在路径字段标题行");
    assert!(
        cx.debug_bounds("ssh-openssh-label").is_some(),
        "OpenSSH 状态前应显示本机 SSH 标题"
    );
    assert!(
        cx.debug_bounds("ssh-production-label").is_some(),
        "生产模式标题应参与布局"
    );
    assert!(name.origin.y < host.origin.y, "名称应显示在 Host 上方");
    let executable = cx
        .debug_bounds("ssh-profile-executable-field")
        .expect("OpenSSH 路径字段应参与布局");
    assert!(
        openssh_status.origin.y >= executable.origin.y,
        "OpenSSH 状态应移入路径字段"
    );

    let password_auth = cx
        .debug_bounds("ssh-auth-password")
        .expect("password auth button should be rendered");
    let system_auth = cx
        .debug_bounds("ssh-auth-system")
        .expect("system auth button should be rendered");
    assert!(
        password_auth.origin.x < system_auth.origin.x,
        "密码认证应显示在系统认证前"
    );

    form.read_with(cx, |form, cx| {
        assert_eq!(form.auth_mode, SshAuthMode::Password);
        assert!(!form.is_dirty(cx));
    });
    assert!(
        cx.debug_bounds("ssh-profile-password-field-input")
            .is_some(),
        "新建连接默认应显示密码输入框"
    );
    form.update(cx, |form, cx| form.set_auth_mode(SshAuthMode::System, cx));
    cx.run_until_parked();
    form.read_with(cx, |form, cx| assert!(form.is_dirty(cx)));
}

#[gpui_kit::test]
fn profile_form_stays_inside_compact_window_and_keeps_actions_visible(cx: &mut TestAppContext) {
    let (_, cx) = add_ssh_form_window(cx, service(Vec::new(), None));

    for height in [240.0, 640.0] {
        cx.simulate_resize(size(px(360.0), px(height)));
        cx.run_until_parked();

        let viewport = size(px(360.0), px(height));
        let body = cx
            .debug_bounds("ssh-profile-form-body")
            .expect("紧凑窗口应保留可滚动表单主体");
        let footer = cx
            .debug_bounds("ssh-profile-form-footer")
            .expect("紧凑窗口应保留底部操作区");
        assert!(body.origin.x >= px(0.0) && body.right() <= viewport.width);
        assert!(footer.origin.x >= px(0.0) && footer.right() <= viewport.width);
        assert!(
            footer.origin.y >= px(0.0) && footer.bottom() <= viewport.height,
            "底部操作区必须留在 {height}px 视口内：{footer:?}"
        );

        for selector in [
            "ssh-profile-name-field-input",
            "ssh-profile-host-field-input",
            "ssh-profile-port-field-input",
            "ssh-profile-executable-field-input",
        ] {
            let bounds = cx
                .debug_bounds(selector)
                .expect("SSH 字段应在紧凑窗口中参与布局");
            assert!(
                bounds.origin.x >= px(0.0) && bounds.right() <= viewport.width,
                "{selector} 越出窗口：{bounds:?}"
            );
        }
    }
}

#[gpui_kit::test]
fn profile_form_feedback_wraps_without_pushing_footer_out_of_compact_window(
    cx: &mut TestAppContext,
) {
    let (form, cx) = add_ssh_form_window(cx, service(Vec::new(), None));

    cx.simulate_resize(size(px(360.0), px(240.0)));
    form.update(cx, |form, cx| {
        form.feedback = Some(super::super::profile_dialog::FormFeedback {
            message: "测试完成 · OpenSSH 可用 · 认证 可用 · 执行 可用 · Terminal 可用 · SFTP 可用 · 通道 Windows 兼容 SFTP · 诊断 可用 · 远端 Windows · Shell Windows PowerShell · 路径 Windows 盘符"
                .into(),
            kind: super::super::profile_dialog::FeedbackKind::Success,
        });
        cx.notify();
    });
    cx.run_until_parked();

    let viewport = size(px(360.0), px(240.0));
    let feedback = cx
        .debug_bounds("ssh-profile-form-feedback")
        .expect("测试反馈应参与布局");
    let footer = cx
        .debug_bounds("ssh-profile-form-footer")
        .expect("底部操作区应参与布局");
    assert!(feedback.origin.x >= px(0.0) && feedback.right() <= viewport.width);
    assert!(footer.origin.x >= px(0.0) && footer.right() <= viewport.width);
    assert!(footer.origin.y >= px(0.0) && footer.bottom() <= viewport.height);
    assert!(feedback.size.height > px(16.0), "长反馈应在窄窗口中换行");
}

#[gpui_kit::test]
fn windows_workspace_lists_accessible_drives_before_the_home_directory(cx: &mut TestAppContext) {
    let mut profile = SshProfile::new("windows", "windows.example");
    profile.username = "Administrator".into();
    profile.remote_platform = RemotePlatformPreference::Windows;
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile.id.clone(),
            last_remote_path: ".".into(),
        }],
        active_profile_id: Some(profile.id.clone()),
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(cx, service(vec![profile.clone()], Some(preference)));
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        let workspace = view
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &profile.id)
            .expect("Windows workspace should be restored");
        assert_eq!(workspace.path, "/");
        assert_eq!(
            workspace
                .entries
                .iter()
                .map(|entry| (entry.name.as_str(), entry.path.as_str()))
                .collect::<Vec<_>>(),
            [("C:", "/C:/"), ("D:", "/D:/")]
        );
        assert!(workspace.sftp_error.is_none());
        assert_eq!(
            workspace
                .capabilities
                .as_ref()
                .map(|capabilities| capabilities.sftp_namespace),
            Some(SftpNamespaceKind::Virtual)
        );
        assert_eq!(
            workspace
                .capabilities
                .as_ref()
                .map(|capabilities| capabilities.shell),
            Some(RemoteShellKind::Cmd)
        );
    });
    cx.update(|window, app| {
        view.update(app, |view, cx| {
            let drive = view
                .workspaces
                .iter()
                .find(|workspace| workspace.profile_id() == &profile.id)
                .and_then(|workspace| workspace.entries.first())
                .cloned()
                .expect("Windows drive should be rendered");
            view.activate_remote_entry(profile.id.clone(), drive, window, cx);
        });
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let workspace = view
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &profile.id)
            .expect("Windows workspace should remain open");
        assert_eq!(workspace.path, "/C:/");
        assert!(workspace.sftp_error.is_none());
    });
}

#[gpui_kit::test]
fn production_workspace_renders_terminal_warning_and_hides_sftp_writes(cx: &mut TestAppContext) {
    let mut profile = profile();
    profile.production = true;
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile.id.clone(),
            last_remote_path: "/home/alice".into(),
        }],
        active_profile_id: Some(profile.id.clone()),
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(cx, service(vec![profile.clone()], Some(preference)));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();

    assert!(cx.debug_bounds("ssh-production-terminal-warning").is_some());
    assert!(cx.debug_bounds("ssh-terminal-drop-target").is_some());
    assert!(cx.debug_bounds("sftp-upload").is_none());
    assert!(cx.debug_bounds("sftp-mkdir").is_none());
    view.read_with(cx, |view, _| {
        let workspace = view
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &profile.id)
            .expect("production workspace should exist");
        assert!(!workspace.terminal_loading);
    });
}

#[gpui_kit::test]
fn workspace_header_exposes_connection_status_inside_supported_widths(cx: &mut TestAppContext) {
    let mut profile = profile();
    profile.name = "WSL 本机 SSH 长名称用于检查连接标签是否正确省略显示".into();
    let profile_id = profile.id.clone();
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile_id.clone(),
            last_remote_path: "/home/alice".into(),
        }],
        active_profile_id: Some(profile_id.clone()),
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(cx, service(vec![profile], Some(preference)));
    cx.run_until_parked();

    for mode in [ramag_ui::Mode::Light, ramag_ui::Mode::Dark] {
        cx.update(|_, app| ramag_ui::apply_theme(mode, app));
        for session_state in [
            SshSessionState::Disconnected,
            SshSessionState::Connecting,
            SshSessionState::Connected,
            SshSessionState::Reconnecting,
            SshSessionState::Exited,
            SshSessionState::Failed,
        ] {
            view.update(cx, |view, cx| {
                view.workspace_mut(&profile_id)
                    .expect("workspace should remain available")
                    .session_state = session_state;
                cx.notify();
            });
            cx.run_until_parked();

            for width in [360.0, 1024.0, 1440.0] {
                cx.simulate_resize(size(px(width), px(720.0)));
                cx.run_until_parked();

                let header = cx
                    .debug_bounds("ssh-workspace-page-header")
                    .expect("SSH 工作区应显示公共页头");
                let title = cx
                    .debug_bounds("ssh-workspace-page-title")
                    .expect("SSH 工作区应显示连接标题");
                let header_status = cx
                    .debug_bounds("ssh-workspace-connection-status")
                    .expect("SSH 工作区页头应显示连接状态");
                let connection_tab = cx
                    .debug_bounds("ssh-workspace-tab-0")
                    .expect("顶部连接标签应显示");
                let tab_name = cx
                    .debug_bounds("ssh-workspace-tab-name-0")
                    .expect("顶部连接标签应显示连接名称");
                let tab_status = cx
                    .debug_bounds("ssh-workspace-tab-status-0")
                    .expect("顶部连接标签应显示可读连接状态");
                let close = cx
                    .debug_bounds("ssh-workspace-tab-close-0")
                    .expect("顶部连接标签关闭按钮应显示");
                let main = cx
                    .debug_bounds("ssh-workspace-main")
                    .expect("SSH 工作区主体应参与布局");
                assert!(
                    header.origin.x >= px(0.0) && header.right() <= px(width),
                    "工作区页头不能越出窗口：header={header:?}, width={width}"
                );
                assert!(
                    title.origin.x >= header.origin.x
                        && title.right() <= header.right()
                        && title.origin.y >= header.origin.y
                        && title.bottom() <= header.bottom(),
                    "工作区标题不能越出页头：header={header:?}, title={title:?}"
                );
                let expected_max_tab_width = (width - 120.0).clamp(220.0, 420.0);
                assert!(
                    connection_tab.size.width <= px(expected_max_tab_width),
                    "长连接标签应按窗口宽度收缩：tab={connection_tab:?}, width={width}"
                );
                assert!(
                    connection_tab.origin.x >= px(0.0)
                        && connection_tab.right() <= px(width)
                        && tab_name.origin.x >= connection_tab.origin.x
                        && tab_name.right() <= connection_tab.right()
                        && tab_name.origin.y >= connection_tab.origin.y
                        && tab_name.bottom() <= connection_tab.bottom(),
                    "连接名不能越出顶部标签或窗口：tab={connection_tab:?}, name={tab_name:?}, width={width}"
                );
                assert!(
                    tab_status.origin.x >= connection_tab.origin.x
                        && tab_status.right() <= connection_tab.right()
                        && tab_status.origin.y >= connection_tab.origin.y
                        && tab_status.bottom() <= connection_tab.bottom(),
                    "连接状态必须与名称一起留在顶部标签内：tab={connection_tab:?}, status={tab_status:?}"
                );
                assert!(
                    close.origin.x >= connection_tab.origin.x
                        && close.right() <= connection_tab.right()
                        && close.origin.y >= connection_tab.origin.y
                        && close.bottom() <= connection_tab.bottom()
                        && tab_status.right() <= close.origin.x,
                    "关闭按钮应保持固定命中区域并与连接状态分离：tab={connection_tab:?}, status={tab_status:?}, close={close:?}"
                );
                assert!(
                    header_status.origin.x >= header.origin.x
                        && header_status.right() <= header.right()
                        && header_status.origin.y >= header.origin.y
                        && header_status.bottom() <= header.bottom(),
                    "页头连接状态不能越出页头：header={header:?}, status={header_status:?}"
                );
                assert!(
                    main.origin.y >= header.bottom(),
                    "工作区主体不能覆盖公共页头：header={header:?}, main={main:?}"
                );
            }
        }
    }
}
