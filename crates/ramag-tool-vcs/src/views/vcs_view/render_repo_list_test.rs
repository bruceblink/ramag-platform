use super::super::{add_vcs_window, inject_diff_session, mock_repo};
use gpui_kit::{Bounds, Pixels, TestAppContext, px, size};
use std::rc::Rc;

fn assert_inside(parent: &Bounds<Pixels>, child: &Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 越出父容器：parent={parent:?}, child={child:?}"
    );
}

/// 最近仓库列表在紧凑窗口中把路径移到名称下方，桌面宽度则保持横向信息布局。
#[gpui_kit::test]
fn repo_list_rows_reflow_inside_supported_window_widths(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    let mut repo = mock_repo();
    repo.name = "a-repository-with-a-long-display-name".into();
    repo.path = "C:/workspace/a-repository-with-a-long-display-name".into();
    let repo_key = repo.id.to_string();
    view.update(cx, |view, cx| {
        view.recent_repos = Rc::new(vec![repo]);
        cx.notify();
    });

    for width in [360.0, 640.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(720.0)));
        cx.run_until_parked();

        let root = cx
            .debug_bounds("vcs-repo-list")
            .expect("仓库列表根节点应渲染");
        let header = cx
            .debug_bounds("vcs-repo-list-header")
            .expect("仓库列表头部应渲染");
        let header_inner = cx
            .debug_bounds("vcs-repo-list-header-inner")
            .expect("仓库列表头部内容应渲染");
        let page_title = cx
            .debug_bounds("vcs-repo-list-page-title")
            .expect("仓库列表页标题应渲染");
        let toolbar = cx
            .debug_bounds("vcs-repo-list-toolbar")
            .expect("仓库管理工具栏应渲染");
        let row_selector: &'static str =
            Box::leak(format!("vcs-repo-row-{repo_key}").into_boxed_str());
        let name_selector: &'static str =
            Box::leak(format!("vcs-repo-row-name-{repo_key}").into_boxed_str());
        let path_selector: &'static str =
            Box::leak(format!("vcs-repo-row-path-{repo_key}").into_boxed_str());
        let actions_selector: &'static str =
            Box::leak(format!("vcs-repo-row-actions-{repo_key}").into_boxed_str());
        let row = cx.debug_bounds(row_selector).expect("最近仓库行应渲染");
        let name = cx.debug_bounds(name_selector).expect("仓库名应渲染");
        let path = cx.debug_bounds(path_selector).expect("仓库路径应渲染");
        let actions = cx.debug_bounds(actions_selector).expect("仓库操作应渲染");

        assert_inside(&root, &header, "仓库列表头部");
        assert_inside(&header, &header_inner, "仓库列表头部内容");
        assert_inside(&header_inner, &page_title, "仓库列表页标题");
        assert_inside(&header_inner, &toolbar, "仓库管理工具栏");
        assert!(
            page_title.bottom() <= toolbar.origin.y,
            "仓库标题与操作工具栏应分层排列：title={page_title:?}, toolbar={toolbar:?}"
        );
        assert_inside(&root, &row, "最近仓库行");
        assert_inside(&row, &name, "仓库名");
        assert_inside(&row, &path, "仓库路径");
        assert_inside(&row, &actions, "仓库操作");

        if width < 720.0 {
            assert!(
                path.origin.y > name.origin.y,
                "紧凑宽度应将路径放到仓库名下方：name={name:?}, path={path:?}"
            );
        } else {
            assert!(
                path.origin.x >= name.right(),
                "桌面宽度应保持名称和路径横向排列：name={name:?}, path={path:?}"
            );
        }
    }
}

/// Session 页标题在现有仓库标签栏内随窗口宽度收缩，不占用工作区垂直空间。
#[gpui_kit::test]
fn session_page_title_and_toolbar_fit_supported_widths(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    view.update(cx, |view, cx| {
        inject_diff_session(view);
        let repo = view.repo.as_mut().expect("测试仓库应已打开");
        repo.name = "a-repository-with-a-very-long-display-name-for-responsive-layout".into();
        repo.path =
            "C:/workspace/a-repository-with-a-very-long-display-name-for-responsive-layout".into();
        view.open_repos = vec![repo.clone()];
        cx.notify();
    });

    for width in [360.0, 640.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(720.0)));
        cx.run_until_parked();

        let tabs = cx.debug_bounds("vcs-tabs").expect("VCS 顶层标签栏应渲染");
        let title = cx
            .debug_bounds("vcs-session-page-title")
            .expect("Session 页标题应渲染");
        let compact = width < 720.0;
        let max_title_width = if compact { 150.0 } else { 240.0 };

        assert_inside(&tabs, &title, "Session 页标题");
        assert!(
            title.size.width <= px(max_title_width),
            "标题宽度应随窗口尺寸收缩：width={width}, title={title:?}"
        );
    }
}

/// Pulse 错误提示与保留的复制/清除操作在 RepoList 和 Session 中均不越界。
#[gpui_kit::test]
fn vcs_error_notice_and_actions_stay_inside_both_page_layouts(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    view.update(cx, |view, cx| {
        view.error = Some("Git 操作失败：".to_string() + &"详细错误信息 ".repeat(20));
        cx.notify();
    });

    for width in [360.0, 640.0, 1024.0] {
        cx.simulate_resize(size(px(width), px(720.0)));
        cx.run_until_parked();
        assert_error_layout_inside(cx, "vcs-repo-list");
    }

    view.update(cx, |view, cx| {
        inject_diff_session(view);
        view.error = Some("Git 操作失败：".to_string() + &"详细错误信息 ".repeat(20));
        cx.notify();
    });

    for width in [360.0, 640.0, 1024.0] {
        cx.simulate_resize(size(px(width), px(720.0)));
        cx.run_until_parked();
        assert_error_layout_inside(cx, "vcs-ide-layout-root");
    }
}

fn assert_error_layout_inside(cx: &mut gpui_kit::VisualTestContext, page_selector: &'static str) {
    let page = cx.debug_bounds(page_selector).expect("VCS 页面边界应渲染");
    let error_boundary = cx
        .debug_bounds("vcs-error-banner")
        .expect("错误提示边界应渲染");
    let panel = cx
        .debug_bounds("vcs-error-panel")
        .expect("错误状态面板应渲染");
    let notice = cx
        .debug_bounds("pulse-status-notice")
        .expect("Pulse 错误通知应渲染");
    let toolbar = cx
        .debug_bounds("vcs-error-toolbar")
        .expect("错误操作工具栏应渲染");
    let copy = cx
        .debug_bounds("vcs-error-copy")
        .expect("复制错误操作应渲染");
    let clear = cx
        .debug_bounds("vcs-error-clear")
        .expect("清除错误操作应渲染");

    assert_inside(&page, &error_boundary, "错误提示外边界");
    assert_inside(&error_boundary, &panel, "错误状态面板");
    assert_inside(&panel, &notice, "Pulse 错误通知");
    assert_inside(&panel, &toolbar, "错误操作工具栏");
    assert_inside(&toolbar, &copy, "复制错误操作");
    assert_inside(&toolbar, &clear, "清除错误操作");
    assert!(
        notice.bottom() <= toolbar.origin.y,
        "错误说明与操作工具栏应分层排列：notice={notice:?}, toolbar={toolbar:?}"
    );
}

/// 仓库搜索排序变化后，原仓库仍可通过 RepoId 对应的稳定选择器定位。
#[gpui_kit::test]
fn repo_list_rows_keep_stable_selectors_after_reorder(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_window(cx);
    let first = mock_repo();
    let second = mock_repo();
    let first_key = first.id.to_string();
    let first_selector: &'static str =
        Box::leak(format!("vcs-repo-row-{first_key}").into_boxed_str());
    let first_name_selector: &'static str =
        Box::leak(format!("vcs-repo-row-name-{first_key}").into_boxed_str());
    view.update(cx, |view, cx| {
        view.recent_repos = Rc::new(vec![first, second]);
        cx.notify();
    });
    cx.run_until_parked();

    for width in [360.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(720.0)));
        view.update(cx, |view, cx| {
            let mut repos = view.recent_repos.as_ref().clone();
            repos.reverse();
            view.recent_repos = Rc::new(repos);
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds(first_selector).is_some());
        assert!(cx.debug_bounds(first_name_selector).is_some());
    }
}
