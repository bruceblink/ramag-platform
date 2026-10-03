#![allow(clippy::expect_used)]

use super::{add_vcs_window, inject_diff_session};
use gpui_kit::{Bounds, Pixels, TestAppContext, px, size};

#[path = "diff_hunk_render_test.rs"]
mod diff_hunk_render_test;
#[path = "remote_dialog_render_test.rs"]
mod remote_dialog_render_test;
#[path = "render_repo_list_test.rs"]
mod render_repo_list_test;

fn assert_inside(parent: &Bounds<Pixels>, child: &Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 越出父容器：parent={parent:?}, child={child:?}"
    );
}

/// 文件栏在最小可拖动宽度下，模式、搜索和固定操作都不能越出工具栏。
#[gpui_kit::test]
fn vcs_files_toolbar_wraps_controls_inside_supported_widths(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    view.update(cx, |view, cx| {
        inject_diff_session(view);
        let status = view.status.as_mut().expect("测试仓库应有状态");
        status.ahead = Some(1);
        status.behind = Some(2);
        cx.notify();
    });
    cx.simulate_resize(size(px(1440.0), px(720.0)));
    cx.run_until_parked();

    for width in [180.0, 280.0, 600.0] {
        cx.update(|window, app| {
            view.update(app, |view, cx| {
                view.ide_left_resize.update(cx, |state, cx| {
                    state.resize_panel(0, px(width), window, cx);
                });
            });
        });
        cx.run_until_parked();

        let files_column = cx
            .debug_bounds("vcs-files-column")
            .expect("VCS 文件栏应渲染");
        let toolbar = cx
            .debug_bounds("vcs-files-toolbar")
            .expect("VCS 文件栏工具栏应渲染");
        let mode_toolbar = cx
            .debug_bounds("vcs-files-mode-toolbar")
            .expect("VCS 模式工具栏应渲染");
        let search_toolbar = cx
            .debug_bounds("vcs-files-search-toolbar")
            .expect("VCS 搜索工具栏应渲染");
        let search = cx
            .debug_bounds("vcs-files-search")
            .expect("VCS 文件搜索框应渲染");
        assert_inside(&files_column, &toolbar, "VCS 文件栏工具栏");
        assert_inside(&toolbar, &mode_toolbar, "VCS 模式工具栏");
        assert_inside(&toolbar, &search_toolbar, "VCS 搜索工具栏");
        assert_inside(&search_toolbar, &search, "VCS 文件搜索框");

        for selector in [
            "vcs-files-tab-project",
            "vcs-files-tab-changes",
            "vcs-files-tab-stash",
            "vcs-branch-picker",
            "vcs-refresh",
            "vcs-pf-toggle-all",
            "vcs-history-pane-toggle",
            "vcs-files-quick-action",
            "vcs-files-remote-actions",
        ] {
            let control = cx.debug_bounds(selector).expect("VCS 文件栏控件应渲染");
            let parent = if selector.starts_with("vcs-files-tab") || selector == "vcs-branch-picker"
            {
                mode_toolbar
            } else {
                search_toolbar
            };
            assert_inside(&parent, &control, selector);
        }

        let project_tab = cx
            .debug_bounds("vcs-files-tab-project")
            .expect("项目文件模式应渲染");
        let changes_tab = cx
            .debug_bounds("vcs-files-tab-changes")
            .expect("变更模式应渲染");
        let stash_tab = cx
            .debug_bounds("vcs-files-tab-stash")
            .expect("储藏模式应渲染");
        let branch_picker = cx
            .debug_bounds("vcs-branch-picker")
            .expect("分支选择器应渲染");
        assert_eq!(
            project_tab.origin.y, changes_tab.origin.y,
            "项目文件和变更模式应保持在同一导航行"
        );
        assert_eq!(
            changes_tab.origin.y, stash_tab.origin.y,
            "变更和储藏模式应保持在同一导航行"
        );
        assert!(
            project_tab.right() <= branch_picker.origin.x
                || branch_picker.right() <= project_tab.origin.x
                || project_tab.bottom() <= branch_picker.origin.y
                || branch_picker.bottom() <= project_tab.origin.y,
            "项目文件模式和分支选择器不能重叠：tab={project_tab:?}, branch={branch_picker:?}"
        );
        if width >= 280.0 {
            assert!(
                branch_picker.size.width >= px(140.0),
                "标准文件栏宽度应为分支选择器保留可读空间：width={width}, branch={branch_picker:?}"
            );
            assert_eq!(
                project_tab.origin.y, branch_picker.origin.y,
                "常规文件栏宽度应让分支选择器与模式按钮保持在同一行：width={width}, tab={project_tab:?}, branch={branch_picker:?}"
            );
        } else {
            assert!(
                branch_picker.size.width >= px(140.0),
                "窄文件栏换行后仍应保留分支选择器的最小可读宽度：branch={branch_picker:?}"
            );
            assert!(
                branch_picker.origin.y > stash_tab.origin.y,
                "最窄文件栏应将分支选择器完整放到模式行下方：tab={stash_tab:?}, branch={branch_picker:?}"
            );
        }

        assert!(
            cx.debug_bounds("vcs-history-remote-actions").is_none(),
            "History 未打开时不应渲染重复远程操作入口"
        );

        if width == 180.0 {
            let history = cx
                .debug_bounds("vcs-history-pane-toggle")
                .expect("历史按钮应渲染");
            assert!(
                history.origin.y > search.origin.y,
                "最小文件栏宽度应让部分搜索操作换行：search={search:?}, history={history:?}"
            );
        }
    }
}

/// 历史搜索栏在紧凑工作区中允许搜索和远程操作分行，所有控件都留在历史内容区内。
#[gpui_kit::test]
fn vcs_history_toolbar_wraps_controls_inside_supported_window_widths(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    view.update(cx, |view, cx| {
        inject_diff_session(view);
        view.history_pane_visible = true;
        let status = view.status.as_mut().expect("测试仓库应有状态");
        status.ahead = Some(3);
        status.behind = Some(2);
        cx.notify();
    });

    for width in [360.0, 800.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(720.0)));
        cx.update(|window, app| {
            view.update(app, |view, cx| {
                view.ide_left_resize.update(cx, |state, cx| {
                    state.resize_panel(0, px(180.0), window, cx);
                });
            });
        });
        cx.run_until_parked();

        let history_content = cx
            .debug_bounds("vcs-history-content")
            .expect("历史右侧内容区应渲染");
        let toolbar = cx
            .debug_bounds("vcs-history-search-toolbar")
            .expect("历史搜索工具栏应渲染");
        let search = cx
            .debug_bounds("vcs-history-search-input")
            .expect("历史搜索框应渲染");
        assert_inside(&history_content, &toolbar, "历史搜索工具栏");
        assert_inside(&toolbar, &search, "历史搜索框");

        for selector in ["vcs-history-reflog-toggle", "vcs-history-search-action"] {
            let control = cx.debug_bounds(selector).expect("历史工具栏控件应渲染");
            assert_inside(&toolbar, &control, selector);
        }

        for selector in ["vcs-files-quick-action", "vcs-files-remote-actions"] {
            let control = cx.debug_bounds(selector).expect("文件工具栏远程控件应渲染");
            assert_inside(
                &cx.debug_bounds("vcs-files-search-toolbar")
                    .expect("VCS 搜索工具栏应渲染"),
                &control,
                selector,
            );
            assert!(
                cx.debug_bounds(if selector == "vcs-files-quick-action" {
                    "vcs-history-quick-action"
                } else {
                    "vcs-history-remote-actions"
                })
                .is_none(),
                "History 工具栏不应重复渲染远程控件"
            );
        }

        if width == 360.0 {
            let search_action = cx
                .debug_bounds("vcs-history-search-action")
                .expect("历史搜索操作应渲染");
            assert!(
                search_action.origin.y > search.origin.y,
                "紧凑窗口应让固定搜索操作换行：search={search:?}, action={search_action:?}"
            );
        }
    }
}

/// 没有选中文件时，Diff 提示必须留在右侧主面板内，不能覆盖左侧文件栏。
#[gpui_kit::test]
fn vcs_empty_diff_status_stays_inside_main_panel_at_narrow_width(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    view.update(cx, |view, cx| {
        inject_diff_session(view);
        view.active_file_tab_idx = None;
        view.current_diff = None;
        view.current_diff_syntax = None;
        view.selected_file = None;
        cx.notify();
    });
    cx.simulate_resize(size(px(360.0), px(720.0)));
    cx.run_until_parked();

    let files_column = cx
        .debug_bounds("vcs-files-column")
        .expect("VCS 文件栏应渲染");
    let status = cx
        .debug_bounds("ramag-centered-status-message")
        .expect("Diff 空状态提示应渲染");
    let panel = cx
        .debug_bounds("vcs-empty-diff-panel")
        .expect("Diff 空状态应渲染为 Pulse 面板");
    let description = cx
        .debug_bounds("vcs-empty-diff-description")
        .expect("Diff 空状态应说明文件选择方式");

    assert!(
        status.origin.x >= files_column.right(),
        "Diff 空状态提示不能覆盖文件栏：files={files_column:?}, status={status:?}"
    );
    assert_inside(&panel, &status, "Diff 空状态标题");
    assert_inside(&panel, &description, "Diff 空状态说明");
    assert!(
        panel.right() <= px(360.0),
        "Diff 空状态面板不能越出窄窗口右侧：panel={panel:?}"
    );
}
