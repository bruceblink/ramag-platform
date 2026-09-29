use super::super::{add_vcs_window, mock_repo};
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
