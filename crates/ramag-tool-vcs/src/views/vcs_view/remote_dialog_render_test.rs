#![allow(clippy::expect_used)]

use super::super::super::super::helpers::RemoteOp;
use super::super::{MockGit, MockStorage, VcsView};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, div,
};
use ramag_domain::entities::Remote;
use std::sync::Arc;

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
