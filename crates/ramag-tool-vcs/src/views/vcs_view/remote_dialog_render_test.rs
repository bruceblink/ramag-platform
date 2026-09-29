#![allow(clippy::expect_used)]

use super::super::super::super::helpers::RemoteOp;
use super::super::{MockGit, MockStorage, VcsView};
use crate::views::sidebar::{SidebarSection, open_sidebar_create_dialog};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, div,
};
use gpui_kit::{Bounds, Modifiers, Pixels, point, px, size};
use ramag_domain::entities::Remote;
use std::sync::Arc;

/// 检查可交互区域的四边，避免只有弹窗外框合规但内部控件已被裁掉。
fn assert_dialog_bounds(parent: Bounds<Pixels>, child: Bounds<Pixels>, label: &str) {
    assert!(
        child.size.width > px(0.0)
            && child.size.height > px(0.0)
            && child.left() >= parent.left()
            && child.top() >= parent.top()
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 必须可见且位于容器内: parent={parent:?}, child={child:?}"
    );
}

/// 创建表单在短窗口中仍可读和取消，关闭后不触发 Git 写入状态。
#[gpui_kit::test]
fn sidebar_create_dialogs_fit_short_windows_and_cancel(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_dialog_window(cx);
    cx.run_until_parked();
    // 空存储未实现最近仓库读取；清理初始化错误后，专门观察取消路径是否调用提交校验。
    view.update(cx, |view, _| view.error = None);
    for (section, primary, cancel) in [
        (
            SidebarSection::Tag,
            "vcs-create-tag",
            "vcs-create-tag-cancel",
        ),
        (
            SidebarSection::RemoteRepo,
            "vcs-create-remote",
            "vcs-create-remote-cancel",
        ),
    ] {
        for (width, height) in [
            (360.0, 240.0),
            (360.0, 640.0),
            (1024.0, 768.0),
            (1440.0, 900.0),
        ] {
            cx.simulate_resize(size(px(width), px(height)));
            cx.update(|window, app| open_sidebar_create_dialog(view.clone(), section, window, app));
            cx.run_until_parked();
            let viewport = Bounds::new(point(px(0.0), px(0.0)), size(px(width), px(height)));
            let dialog = cx.debug_bounds("dialog-0").expect("创建弹窗应打开");
            assert_dialog_bounds(viewport, dialog, "创建弹窗");
            for selector in ["ramag-dialog-title", primary, cancel] {
                let control = cx.debug_bounds(selector).expect("标题和操作应参与布局");
                assert_dialog_bounds(dialog, control, selector);
            }
            let title = cx.debug_bounds("ramag-dialog-title").expect("表单标题");
            let fields = cx.debug_bounds("vcs-create-fields").expect("正文滚动区域");
            let footer = cx.debug_bounds("ramag-dialog-footer").expect("表单操作区");
            assert_dialog_bounds(dialog, fields, "正文滚动区域");
            assert_dialog_bounds(dialog, footer, "表单操作区");
            assert!(
                fields.size.height >= px(40.0),
                "短窗口也应至少容纳一个输入框"
            );
            assert!(
                title.bottom() <= fields.top() && fields.bottom() <= footer.top(),
                "标题、正文和底部操作不能重叠"
            );
            let (name, detail) = view.read_with(cx, |view, _| match section {
                SidebarSection::Tag => (
                    view.create_tag_input.clone(),
                    view.create_tag_message_input.clone(),
                ),
                _ => (
                    view.create_remote_name_input.clone(),
                    view.create_remote_url_input.clone(),
                ),
            });
            assert!(
                name.read_with(cx, |input, _| input.value().is_empty()),
                "重新打开应清空名称"
            );
            assert!(
                detail.read_with(cx, |input, _| input.value().is_empty()),
                "重新打开应清空备注或地址"
            );
            let name_bounds = cx.debug_bounds("vcs-create-name").expect("名称输入框");
            assert_dialog_bounds(fields, name_bounds, "首个输入框");
            cx.simulate_click(name_bounds.center(), Modifiers::default());
            cx.simulate_input("release-check");
            cx.run_until_parked();
            assert_eq!(
                name.read_with(cx, |input, _| input.value().to_string()),
                "release-check"
            );
            let detail_bounds = cx.debug_bounds("vcs-create-detail").expect("第二个输入框");
            assert_dialog_bounds(dialog, detail_bounds, "第二个输入框");
            let cancel_bounds = cx.debug_bounds(cancel).expect("取消按钮应可见");
            cx.simulate_click(cancel_bounds.center(), Modifiers::default());
            cx.run_until_parked();
            assert!(cx.debug_bounds("dialog-0").is_none(), "取消应关闭表单");
            view.read_with(cx, |view, _| {
                assert!(!view.busy, "取消不应开始 Git 操作");
                assert!(
                    view.error.is_none(),
                    "取消不应进入提交校验: {:?}",
                    view.error
                );
            });
        }
    }
}

struct VcsDialogTestHost {
    view: Entity<VcsView>,
}

impl Render for VcsDialogTestHost {
    fn render(
        &mut self,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(gpui_kit::component::Root::render_dialog_layer(window, cx))
    }
}

fn add_vcs_dialog_window(cx: &mut TestAppContext) -> (Entity<VcsView>, &mut VisualTestContext) {
    cx.update(gpui_kit::component::init);

    let mut view = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let vcs_view =
            cx.new(|cx| VcsView::new(Arc::new(MockGit), Arc::new(MockStorage), window, cx));
        view = Some(vcs_view.clone());
        let host = cx.new(|_| VcsDialogTestHost { view: vcs_view });
        gpui_kit::component::Root::new(host, window, cx)
    });

    (view.expect("VcsView should be initialized"), visual_cx)
}

/// 首次 Push 远程选择弹窗按远程名定位按钮，不依赖弹窗内的可见顺序。
#[gpui_kit::test]
fn first_push_remote_picker_uses_remote_name_selectors(cx: &mut TestAppContext) {
    let (view, cx) = add_vcs_dialog_window(cx);
    cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.remotes = vec![
                Remote {
                    name: "upstream".into(),
                    fetch_url: "https://example.test/upstream.git".into(),
                    push_url: None,
                },
                Remote {
                    name: "fork".into(),
                    fetch_url: "https://example.test/fork.git".into(),
                    push_url: None,
                },
            ];
            view.confirm_remote_op(RemoteOp::Push, window, cx);
        });
    });
    cx.run_until_parked();

    for remote in ["upstream", "fork"] {
        let selector: &'static str =
            Box::leak(format!("vcs-first-push-remote-{remote}").into_boxed_str());
        assert!(
            cx.debug_bounds(selector).is_some(),
            "首次 Push 应渲染 {remote} 远程选择按钮"
        );
    }
}
