use super::*;

#[gpui_kit::test]
fn manager_rows_show_connection_status_inside_supported_widths(cx: &mut TestAppContext) {
    let profile = profile();
    let profile_id = profile.id.clone();
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile_id.clone(),
            last_remote_path: "/home/alice".into(),
        }],
        active_profile_id: None,
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(cx, service(vec![profile], Some(preference)));
    cx.run_until_parked();
    view.update(cx, |view, cx| {
        view.workspace_mut(&profile_id)
            .expect("restored workspace should exist")
            .session_state = SshSessionState::Connected;
        cx.notify();
    });
    cx.run_until_parked();

    for width in [360.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(720.0)));
        cx.run_until_parked();
        let row = cx
            .debug_bounds("ssh-profile-row-0")
            .expect("SSH 连接行应渲染");
        let status = cx
            .debug_bounds("ssh-profile-status-0")
            .expect("SSH 连接行应显示状态");
        assert!(
            status.origin.x >= row.origin.x
                && status.right() <= row.right()
                && status.origin.y >= row.origin.y
                && status.bottom() <= row.bottom(),
            "SSH 连接状态不能越出连接行：row={row:?}, status={status:?}, width={width}"
        );
    }
}
