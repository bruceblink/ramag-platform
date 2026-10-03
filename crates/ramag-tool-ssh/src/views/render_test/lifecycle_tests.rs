use super::*;

#[gpui_kit::test]
fn directory_search_state_is_isolated_by_workspace(cx: &mut TestAppContext) {
    let first = profile();
    let mut second = SshProfile::new("staging", "staging.example");
    second.initial_directory = Some("/srv/app".into());
    let preference = SshWorkspacePreference {
        workspaces: vec![
            SshWorkspaceState {
                profile_id: first.id.clone(),
                last_remote_path: "/home/alice".into(),
            },
            SshWorkspaceState {
                profile_id: second.id.clone(),
                last_remote_path: "/srv/app".into(),
            },
        ],
        active_profile_id: Some(first.id.clone()),
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(
        cx,
        service(vec![first.clone(), second.clone()], Some(preference)),
    );
    cx.run_until_parked();
    view.update(cx, |view, _| {
        view.workspace_mut(&first.id)
            .expect("首个工作区应恢复")
            .directory_query = "logs".into();
    });

    cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.select_workspace(second.id.clone(), window, cx);
        });
    });
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        assert_eq!(view.directory_search.read(cx).value(), "");
        assert_eq!(
            view.workspaces
                .iter()
                .find(|workspace| workspace.profile_id() == &first.id)
                .expect("首个工作区应保留")
                .directory_query,
            "logs"
        );
    });

    cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.select_workspace(first.id.clone(), window, cx);
        });
    });
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        assert_eq!(view.directory_search.read(cx).value(), "logs");
    });
}

#[gpui_kit::test]
fn workspace_resize_is_isolated_by_connection(cx: &mut TestAppContext) {
    let first = profile();
    let second = SshProfile::new("staging", "staging.example");
    let preference = SshWorkspacePreference {
        workspaces: vec![
            SshWorkspaceState {
                profile_id: first.id.clone(),
                last_remote_path: "/home/alice".into(),
            },
            SshWorkspaceState {
                profile_id: second.id.clone(),
                last_remote_path: "/srv/app".into(),
            },
        ],
        active_profile_id: Some(first.id.clone()),
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(
        cx,
        service(vec![first.clone(), second.clone()], Some(preference)),
    );
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();

    cx.update(|window, app| {
        view.update(app, |view, cx| {
            let resize = view
                .workspace_resizes
                .get(&first.id)
                .cloned()
                .expect("首个连接应有独立分栏状态");
            resize.update(cx, |state, cx| {
                state.resize_panel(0, px(360.0), window, cx);
            });
        });
    });
    cx.run_until_parked();
    assert_eq!(
        cx.debug_bounds("ssh-file-browser")
            .expect("首个连接应显示文件栏")
            .size
            .width,
        px(360.0)
    );

    cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.select_workspace(second.id.clone(), window, cx);
        });
    });
    cx.run_until_parked();
    assert_eq!(
        cx.debug_bounds("ssh-file-browser")
            .expect("第二个连接应显示文件栏")
            .size
            .width,
        px(280.0),
        "第二个连接不应继承首个连接拖动后的宽度"
    );

    cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.select_workspace(first.id.clone(), window, cx);
        });
    });
    cx.run_until_parked();
    assert_eq!(
        cx.debug_bounds("ssh-file-browser")
            .expect("切回首个连接应显示文件栏")
            .size
            .width,
        px(360.0),
        "切回首个连接应保留当前会话内自己的宽度"
    );
}

#[gpui_kit::test]
fn close_shortcut_closes_first_workspace_when_no_terminal_exists(cx: &mut TestAppContext) {
    let profile = profile();
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile.id.clone(),
            last_remote_path: "/home/alice".into(),
        }],
        active_profile_id: Some(profile.id.clone()),
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(cx, service(vec![profile], Some(preference)));
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(view.workspaces.len(), 1);
        assert!(view.workspaces[0].terminals.is_empty());
    });
    cx.update(|window, app| {
        let focus = view.read(app).focus_handle.clone();
        window.focus(&focus, app);
        window.dispatch_action(Box::new(crate::CloseSshTerminal), app);
    });
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert!(view.workspaces.is_empty());
        assert_eq!(view.active_workspace_id, None);
        assert_eq!(view.view_mode, ViewMode::Manager);
    });
}

#[cfg(unix)]
#[gpui_kit::test]
fn close_shortcut_selects_and_focuses_previous_terminal(cx: &mut TestAppContext) {
    let profile = profile();
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

    let mut previous_terminal = None;
    let mut active_terminal = None;
    cx.update(|window, app| {
        let terminals = (1..=3)
            .map(|id| {
                let core = TerminalCore::start(TerminalCommand::new("/bin/sh", Vec::new()))
                    .expect("测试终端应启动");
                let terminal = app.new(|cx| TerminalView::new(core, window, cx));
                TerminalTab {
                    id,
                    label: format!("终端 {id}").into(),
                    view: terminal,
                }
            })
            .collect::<Vec<_>>();
        previous_terminal = Some(terminals[1].view.clone());
        active_terminal = Some(terminals[2].view.clone());
        view.update(app, |view, cx| {
            let workspace = view
                .workspaces
                .iter_mut()
                .find(|workspace| workspace.profile_id() == &profile_id)
                .expect("工作区应存在");
            workspace.terminals = terminals;
            workspace.active_terminal_id = Some(3);
            cx.notify();
        });
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        active_terminal
            .as_ref()
            .expect("当前终端应存在")
            .read(app)
            .focus_handle(app)
            .focus(window, app);
        window.dispatch_action(Box::new(crate::CloseSshTerminal), app);
    });
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        let workspace = view
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &profile_id)
            .expect("工作区应存在");
        assert_eq!(workspace.active_terminal_id, Some(2));
        assert_eq!(
            workspace
                .terminals
                .iter()
                .map(|terminal| terminal.id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    });
    assert!(cx.update(|window, app| {
        previous_terminal
            .as_ref()
            .expect("上一个终端应存在")
            .read(app)
            .focus_handle(app)
            .is_focused(window)
    }));
}

#[cfg(unix)]
#[gpui_kit::test]
fn reconnect_replaces_the_current_terminal_without_creating_a_tab(cx: &mut TestAppContext) {
    let mut profile = profile();
    profile.production = false;
    profile.ssh_path = Some("/mock/ssh".into());
    let profile_id = profile.id.clone();
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile_id.clone(),
            last_remote_path: "/home/alice".into(),
        }],
        active_profile_id: Some(profile_id.clone()),
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(cx, service_with_working_terminal(profile, preference));
    cx.run_until_parked();

    cx.update(|window, app| {
        let core = TerminalCore::start(TerminalCommand::new(
            "/bin/sh",
            vec!["-c".into(), "exit 7".into()],
        ))
        .expect("测试终端应启动");
        let terminal = app.new(|cx| TerminalView::new(core, window, cx));
        let live_core = TerminalCore::start(TerminalCommand::new(
            "/bin/sh",
            vec!["-c".into(), "exit 0".into()],
        ))
        .expect("并行成功退出测试终端应启动");
        let live_terminal = app.new(|cx| TerminalView::new(live_core, window, cx));
        view.update(app, |view, cx| {
            let workspace = view
                .workspaces
                .iter_mut()
                .find(|workspace| workspace.profile_id() == &profile_id)
                .expect("工作区应存在");
            workspace.terminals = vec![
                TerminalTab {
                    id: 41,
                    label: "终端 9".into(),
                    view: terminal,
                },
                TerminalTab {
                    id: 42,
                    label: "终端 10".into(),
                    view: live_terminal,
                },
            ];
            workspace.active_terminal_id = Some(41);
            workspace.terminal_loading = false;
            workspace.next_terminal_ordinal = 10;
            cx.notify();
        });
    });
    cx.run_until_parked();

    // 重连入口只接受已经退出的终端；等待旧 PTY 的退出事件到达后再触发入口。
    let old_exit_deadline = Instant::now() + Duration::from_secs(3);
    let mut old_exit_code = None;
    while old_exit_code.is_none() && Instant::now() < old_exit_deadline {
        old_exit_code = cx.update(|_, app| {
            view.read(app)
                .workspaces
                .iter()
                .find(|workspace| workspace.profile_id() == &profile_id)
                .and_then(|workspace| workspace.terminals.first())
                .and_then(|terminal| terminal.view.read(app).core().exit_status())
                .and_then(|status| status.code)
        });
        if old_exit_code.is_none() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert_eq!(old_exit_code, Some(7));
    let sibling_exit_deadline = Instant::now() + Duration::from_secs(3);
    let mut sibling_exit_status = None;
    while sibling_exit_status.is_none() && Instant::now() < sibling_exit_deadline {
        cx.run_until_parked();
        sibling_exit_status = cx.update(|_, app| {
            view.read(app)
                .workspaces
                .iter()
                .find(|workspace| workspace.profile_id() == &profile_id)
                .and_then(|workspace| {
                    workspace
                        .terminals
                        .iter()
                        .find(|terminal| terminal.id == 42)
                })
                .and_then(|terminal| terminal.view.read(app).core().exit_status())
        });
        if sibling_exit_status.is_none() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert_eq!(
        super::super::model::terminal_pulse_status(sibling_exit_status.as_ref(), true),
        (ramag_ui::pulse_ui::PulseStatus::Unavailable, "已完成"),
        "成功退出终端应按自己的退出状态显示"
    );
    let old_exit_status = cx.update(|_, app| {
        view.read(app)
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &profile_id)
            .and_then(|workspace| {
                workspace
                    .terminals
                    .iter()
                    .find(|terminal| terminal.id == 41)
            })
            .and_then(|terminal| terminal.view.read(app).core().exit_status())
    });
    assert_eq!(
        super::super::model::terminal_pulse_status(old_exit_status.as_ref(), true),
        (ramag_ui::pulse_ui::PulseStatus::Failed, "异常退出"),
        "非零退出码应按该终端显示异常状态"
    );
    assert_eq!(
        super::super::model::terminal_tab_label("终端 9", old_exit_status.as_ref(), true),
        "终端 9 [退出: 7]",
        "非零退出时应保留终端名称和真实退出码"
    );
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("ssh-terminal-session-status-41").is_some(),
        "退出的终端标签应显示自己的状态"
    );
    let failed_tab = cx
        .debug_bounds("ssh-terminal-tab-41")
        .expect("非零退出终端标签应显示");
    let failed_status = cx
        .debug_bounds("ssh-terminal-session-status-41")
        .expect("非零退出终端应显示异常状态");
    let failed_reconnect = cx
        .debug_bounds("ssh-terminal-reconnect-41")
        .expect("已退出终端应提供重连入口");
    let failed_close = cx
        .debug_bounds("ssh-terminal-close-41")
        .expect("多终端时已退出终端应提供关闭入口");
    let running_tab = cx
        .debug_bounds("ssh-terminal-tab-42")
        .expect("并行运行终端标签应显示");
    let running_status = cx
        .debug_bounds("ssh-terminal-session-status-42")
        .expect("并行运行终端应显示自己的状态");
    let running_close = cx
        .debug_bounds("ssh-terminal-close-42")
        .expect("多终端时运行中的终端应提供关闭入口");
    assert!(
        failed_status.right() <= failed_reconnect.origin.x
            && failed_reconnect.right() <= failed_close.origin.x
            && failed_tab.origin.x <= failed_status.origin.x
            && failed_tab.right() >= failed_close.right(),
        "失败终端的状态、重连和关闭控件应留在自己的标签中：tab={failed_tab:?}, status={failed_status:?}, reconnect={failed_reconnect:?}, close={failed_close:?}"
    );
    assert!(
        running_tab.origin.x <= running_status.origin.x
            && running_status.right() <= running_close.origin.x
            && running_tab.right() >= running_close.right(),
        "第二终端的状态和关闭控件应留在自己的标签中：tab={running_tab:?}, status={running_status:?}, close={running_close:?}"
    );
    assert!(
        cx.debug_bounds("ssh-terminal-reconnect-42").is_some(),
        "成功退出的第二终端应提供自己的重连入口"
    );

    cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.reconnect_terminal(profile_id.clone(), 41, window, cx);
        });
    });
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        let workspace = view
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &profile_id)
            .expect("工作区应存在");
        assert_eq!(workspace.terminals.len(), 2);
        assert_eq!(workspace.terminals[0].id, 41);
        assert_eq!(workspace.terminals[1].id, 42);
        assert_eq!(workspace.terminals[0].label.as_ref(), "终端 9");
        assert_eq!(workspace.terminals[1].label.as_ref(), "终端 10");
        assert_eq!(workspace.active_terminal_id, Some(41));
        assert_eq!(workspace.next_terminal_ordinal, 10);
    });
    // 重连任务和子进程退出事件都异步完成，只接受替换后终端的成功退出码。
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut exit_code = None;
    while exit_code != Some(0) && Instant::now() < deadline {
        cx.run_until_parked();
        exit_code = cx.update(|_, app| {
            view.read(app)
                .workspaces
                .iter()
                .find(|workspace| workspace.profile_id() == &profile_id)
                .and_then(|workspace| {
                    workspace
                        .terminals
                        .iter()
                        .find(|terminal| terminal.id == 41)
                })
                .and_then(|terminal| terminal.view.read(app).core().exit_status())
                .and_then(|status| status.code)
        });
        if exit_code != Some(0) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert_eq!(exit_code, Some(0));
    let successful_exit_status = cx.update(|_, app| {
        view.read(app)
            .workspaces
            .iter()
            .find(|workspace| workspace.profile_id() == &profile_id)
            .and_then(|workspace| {
                workspace
                    .terminals
                    .iter()
                    .find(|terminal| terminal.id == 41)
            })
            .and_then(|terminal| terminal.view.read(app).core().exit_status())
    });
    assert_eq!(
        super::super::model::terminal_pulse_status(successful_exit_status.as_ref(), true),
        (ramag_ui::pulse_ui::PulseStatus::Unavailable, "已完成"),
        "零退出码应按成功结束状态显示"
    );
    assert_eq!(
        super::super::model::terminal_tab_label("终端 9", successful_exit_status.as_ref(), true),
        "终端 9 [退出: 0]",
        "成功退出时应保留终端名称和零退出码"
    );
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("ssh-terminal-session-status-41").is_some(),
        "成功退出的终端标签应显示自己的状态"
    );
    assert!(
        cx.debug_bounds("ssh-terminal-session-status-42").is_some(),
        "并行终端应继续显示独立状态"
    );
    assert!(
        cx.debug_bounds("ssh-terminal-close-41").is_some()
            && cx.debug_bounds("ssh-terminal-close-42").is_some(),
        "两个终端应各自保留关闭入口"
    );
}

#[gpui_kit::test]
fn empty_state_close_button_closes_first_workspace(cx: &mut TestAppContext) {
    let profile = profile();
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile.id.clone(),
            last_remote_path: "/home/alice".into(),
        }],
        active_profile_id: Some(profile.id.clone()),
        path_favorites: Vec::new(),
    };
    let (view, cx) = add_ssh_window(cx, service(vec![profile], Some(preference)));
    cx.run_until_parked();

    let close = cx
        .debug_bounds("close-empty-ssh-workspace")
        .expect("空终端工作区应提供关闭连接按钮");
    let close_point = point(
        close.origin.x + close.size.width / 2.0,
        close.origin.y + close.size.height / 2.0,
    );
    cx.simulate_mouse_down(close_point, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(close_point, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert!(view.workspaces.is_empty());
        assert_eq!(view.active_workspace_id, None);
        assert_eq!(view.view_mode, ViewMode::Manager);
    });
}

#[gpui_kit::test]
fn restored_workspace_keeps_favorites_per_profile(cx: &mut TestAppContext) {
    let profile = profile();
    let preference = SshWorkspacePreference {
        workspaces: vec![SshWorkspaceState {
            profile_id: profile.id.clone(),
            last_remote_path: "/home/alice".into(),
        }],
        active_profile_id: Some(profile.id.clone()),
        path_favorites: vec![SshPathFavorites {
            profile_id: profile.id.clone(),
            paths: vec!["/var/log".into()],
        }],
    };
    let (view, cx) = add_ssh_window(cx, service(vec![profile.clone()], Some(preference)));
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(
            view.path_favorites.get(&profile.id),
            Some(&vec!["/var/log".into()])
        );
    });
}
