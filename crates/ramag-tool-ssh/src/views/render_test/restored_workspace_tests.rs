use super::*;

#[gpui_kit::test]
fn restored_workspace_renders_files_terminal_placeholder_and_transfer(cx: &mut TestAppContext) {
    let profile = profile();
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile.id.clone(),
            last_remote_path: "/home/alice".into(),
        }],
        active_profile_id: Some(profile.id.clone()),
        path_favorites: Vec::new(),
    };
    let service = service(vec![profile.clone()], Some(preference));
    let local_path = std::env::temp_dir().join("ramag-render-download.txt");
    service
        .enqueue_download(&profile, "/home/alice/readme.txt", &local_path)
        .expect("waiting transfer should enqueue");

    let service_for_assert = service.clone();
    let (view, cx) = add_ssh_window(cx, service);
    cx.run_until_parked();
    view.update(cx, |view, cx| {
        let workspace = view
            .workspace_mut(&profile.id)
            .expect("workspace should be restored");
        workspace.entries = Arc::new(vec![
            RemoteEntry {
                name: "readme.txt".into(),
                path: "/home/alice/readme.txt".into(),
                kind: RemoteEntryKind::File,
                size: 1536,
                permissions: Some(0o100644),
                modified_at: None,
            },
            RemoteEntry {
                name: "logs".into(),
                path: "/home/alice/logs".into(),
                kind: RemoteEntryKind::Directory,
                size: 0,
                permissions: Some(0o40755),
                modified_at: None,
            },
        ]);
        cx.notify();
    });
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(view.view_mode, ViewMode::Workspace);
        assert_eq!(view.workspaces.len(), 1);
        assert!(
            view.workspaces[0].connection_started,
            "恢复的活动工作区应自动重连"
        );
        assert!(view.workspaces[0].terminals.is_empty());
        assert_eq!(view.workspaces[0].entries.len(), 2);
    });
    let file_browser = cx
        .debug_bounds("ssh-file-browser")
        .expect("file browser should be rendered");
    assert_eq!(
        file_browser.size.width,
        px(280.0),
        "目录栏默认宽度应与数据库侧栏一致"
    );
    cx.update(|window, app| {
        view.update(app, |view, cx| {
            let resize = view
                .workspace_resizes
                .get(&profile.id)
                .cloned()
                .expect("活动工作区应有独立分栏状态");
            resize.update(cx, |state, cx| {
                state.resize_panel(0, px(360.0), window, cx);
            });
        });
    });
    cx.run_until_parked();
    assert_eq!(
        cx.debug_bounds("ssh-file-browser")
            .expect("拖动后文件树应参与布局")
            .size
            .width,
        px(360.0),
        "文件树宽度应受分隔条状态控制"
    );
    assert!(
        cx.debug_bounds("ssh-directory-summary").is_some(),
        "目录底部应显示文件与目录数量"
    );
    assert!(
        cx.debug_bounds("ssh-directory-breadcrumb").is_some(),
        "目录顶部应显示可滚动路径"
    );
    assert!(
        cx.debug_bounds("ssh-directory-path-label").is_some(),
        "路径面包屑前应显示路径标签"
    );
    assert!(
        cx.debug_bounds("ssh-directory-search").is_some(),
        "目录操作栏应以搜索框开头"
    );
    assert!(
        cx.debug_bounds("ssh-terminal-drop-target").is_some(),
        "终端区域应接收远程目录拖放"
    );
    let directory = cx
        .debug_bounds("sftp-entry-1")
        .expect("directory entry should be rendered");
    let terminal_target = cx
        .debug_bounds("ssh-terminal-drop-target")
        .expect("terminal drop target should be rendered");
    let generation_before = view.read_with(cx, |view, _| view.workspaces[0].terminal_generation);
    let drag_start = point(
        directory.origin.x + px(12.0),
        directory.origin.y + directory.size.height / 2.0,
    );
    let drop_point = point(
        terminal_target.origin.x + terminal_target.size.width / 2.0,
        terminal_target.origin.y + terminal_target.size.height / 2.0,
    );
    cx.simulate_mouse_down(drag_start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(drag_start.x + px(12.0), drag_start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(drop_point, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(drop_point, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(
            view.workspaces[0].terminal_generation > generation_before,
            "拖入目录后应启动一个新终端，不能复用当前终端"
        );
    });
    let file = cx
        .debug_bounds("sftp-entry-0")
        .expect("file entry should be rendered");
    let file_generation_before =
        view.read_with(cx, |view, _| view.workspaces[0].terminal_generation);
    let file_drag_start = point(
        file.origin.x + px(12.0),
        file.origin.y + file.size.height / 2.0,
    );
    cx.simulate_mouse_down(file_drag_start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(file_drag_start.x + px(12.0), file_drag_start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(drop_point, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(drop_point, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.workspaces[0].terminal_generation, file_generation_before,
            "文件行不应伪装成目录拖动入口"
        );
    });
    let path_label = cx
        .debug_bounds("ssh-directory-path-label")
        .expect("directory path label should be rendered");
    let path_generation_before =
        view.read_with(cx, |view, _| view.workspaces[0].terminal_generation);
    let path_drag_start = point(
        path_label.origin.x + path_label.size.width / 2.0,
        path_label.origin.y + path_label.size.height / 2.0,
    );
    cx.simulate_mouse_down(path_drag_start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(path_drag_start.x - px(12.0), path_drag_start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(drop_point, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(drop_point, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(
            view.workspaces[0].terminal_generation > path_generation_before,
            "拖入当前路径后应在该目录启动一个新终端"
        );
    });
    let entry = cx
        .debug_bounds("sftp-entry-0")
        .expect("remote entry should be rendered");
    assert_eq!(
        entry.size.height,
        px(28.0),
        "目录项高度应与数据库树保持一致"
    );
    view.update(cx, |view, cx| {
        let workspace = view
            .workspace_mut(&profile.id)
            .expect("workspace should remain available");
        workspace.session_state = SshSessionState::Connected;
        workspace.sftp_loading = true;
        workspace.directory_loading_path = Some("/home/alice/logs".into());
        cx.notify();
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.workspaces[0].session_state,
            SshSessionState::Connected,
            "目录加载不能把 SSH 会话状态改成加载状态"
        );
    });
    assert!(
        cx.debug_bounds("ssh-workspace-tab-status-0").is_some(),
        "SFTP 加载时仍应显示独立的 SSH 会话状态标签"
    );
    assert!(
        cx.debug_bounds("sftp-entry-loading-1").is_some(),
        "正在打开的目录行应显示加载图标"
    );
    view.update(cx, |view, cx| {
        let workspace = view
            .workspace_mut(&profile.id)
            .expect("workspace should remain available");
        workspace.sftp_loading = false;
        workspace.directory_loading_path = None;
        cx.notify();
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.workspaces[0].session_state,
            SshSessionState::Connected,
            "目录请求结束不能覆盖 SSH 会话状态"
        );
    });
    assert!(
        cx.debug_bounds("sftp-entry-loading-1").is_none(),
        "目录请求结束后应清除加载图标"
    );
    let entry_point = point(
        entry.origin.x + px(8.0),
        entry.origin.y + entry.size.height / 2.0,
    );
    cx.simulate_mouse_down(entry_point, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(entry_point, MouseButton::Right, Modifiers::default());
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.workspaces[0].selected_path.as_deref(),
            Some("/home/alice/readme.txt"),
            "右键文件时应同步选中对应条目"
        )
    });
    cx.simulate_keystrokes("escape");
    assert_eq!(service_for_assert.transfer_tasks().len(), 1);
    assert!(
        cx.debug_bounds("ssh-directory-transfers").is_some(),
        "目录操作应提供明确的传输入口"
    );
    let workspace_before_notice = cx
        .debug_bounds("ssh-workspace-main")
        .expect("workspace should be rendered");
    view.update(cx, |view, cx| {
        view.notice = Some(Notice::error("测试通知"));
        cx.notify();
    });
    cx.run_until_parked();
    let workspace_after_notice = cx
        .debug_bounds("ssh-workspace-main")
        .expect("workspace should remain rendered");
    assert_eq!(
        workspace_after_notice, workspace_before_notice,
        "通知应使用浮层，不应挤压工作区布局"
    );
    view.read_with(cx, |view, _| {
        assert!(view.notice.is_none(), "通知应移交给全局消息层")
    });
    assert!(
        cx.debug_bounds("ssh-transfer-panel").is_none(),
        "传输面板默认不应打开"
    );

    view.update(cx, |view, cx| view.toggle_transfer_panel(cx));
    cx.run_until_parked();
    let workspace = cx
        .debug_bounds("ssh-workspace-main")
        .expect("workspace should be rendered");
    let transfers = cx
        .debug_bounds("ssh-transfer-panel")
        .expect("触发传输入口后应显示面板");
    assert!(
        transfers.origin.x > workspace.origin.x + workspace.size.width / 2.0,
        "传输面板应悬浮在工作区右侧"
    );
    assert!(
        transfers.size.width <= px(520.0),
        "传输面板不应覆盖过多工作区：{:?}",
        transfers.size
    );
    cx.simulate_resize(size(px(360.0), px(640.0)));
    cx.run_until_parked();
    let compact_panel = cx
        .debug_bounds("ssh-transfer-panel")
        .expect("紧凑窗口仍应显示传输面板");
    let compact_row = cx
        .debug_bounds("ssh-transfer-row")
        .expect("紧凑窗口应显示传输行");
    assert!(compact_panel.origin.x >= px(0.0));
    assert!(compact_panel.right() <= px(360.0));
    assert!(
        compact_row.origin.x >= compact_panel.origin.x
            && compact_row.right() <= compact_panel.right(),
        "SSH 传输行不能越出紧凑浮层：panel={compact_panel:?}, row={compact_row:?}"
    );

    view.update(cx, |view, cx| view.hide_transfer_panel(cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("ssh-transfer-panel").is_none(),
        "收起后不应继续占用工作区"
    );

    view.update(cx, |view, cx| {
        view.workspace_mut(&profile.id)
            .expect("workspace should remain available")
            .sftp_error = Some("打开远程目录：权限不足".into());
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("ssh-directory-direct").is_some(),
        "目录无权列出时应允许直达已知路径"
    );
}
