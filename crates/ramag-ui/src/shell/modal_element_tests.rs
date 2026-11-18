//! 模态包装器的 AccessKit 语义与焦点循环验收。

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::component::Root;
use gpui_kit::{
    AppContext as _, Context, Element, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement, Render, Role, StatefulInteractiveElement as _, TestAppContext, Window, div,
};

use super::ModalElement;

/// 模态内容保留对话框节点的原有语义，并额外标记 AccessKit modal 属性。
#[gpui_kit::test]
fn modal_element_preserves_dialog_semantics_and_marks_accesskit_node(cx: &mut TestAppContext) {
    cx.update(|app| {
        let trap = div()
            .id("modal-test-dialog")
            .role(Role::Dialog)
            .aria_label("工具导航")
            .focus_trap("shell-navigation-focus-trap", &app.focus_handle());
        let modal = ModalElement::new(trap);

        assert_eq!(
            Element::id(&modal),
            Some("shell-navigation-focus-trap".into())
        );
        assert_eq!(Element::a11y_role(&modal), Some(Role::Dialog));
        let mut node = gpui_kit::accesskit::Node::new(Role::Dialog);
        Element::write_a11y_info(&modal, &mut node);
        assert_eq!(node.label(), Some("工具导航"));
        assert!(node.is_modal());
    });
}

#[derive(Clone)]
struct TrapHandles {
    container: FocusHandle,
    first: FocusHandle,
    last: FocusHandle,
}

struct ModalTrapHost {
    handles: TrapHandles,
}

impl Render for ModalTrapHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        ModalElement::new(
            div()
                .id("modal-trap-host")
                .role(Role::Dialog)
                .aria_label("测试对话框")
                .child(
                    div()
                        .id("modal-trap-first")
                        .track_focus(&self.handles.first)
                        .tab_index(0),
                )
                .child(
                    div()
                        .id("modal-trap-last")
                        .track_focus(&self.handles.last)
                        .tab_index(0),
                )
                .focus_trap("modal-trap-container", &self.handles.container),
        )
    }
}

/// ModalElement 必须继续注册焦点陷阱，并在 Tab 到达末项后回到首项。
#[gpui_kit::test]
fn modal_element_keeps_focus_trap_tab_cycle_usable(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let handles = Rc::new(RefCell::new(None::<TrapHandles>));
    let (_, visual) = cx.add_window_view({
        let handles = handles.clone();
        move |window, cx| {
            let trap_handles = TrapHandles {
                container: cx.focus_handle(),
                first: cx.focus_handle().tab_stop(true),
                last: cx.focus_handle().tab_stop(true),
            };
            *handles.borrow_mut() = Some(trap_handles.clone());
            let host = cx.new(|_| ModalTrapHost {
                handles: trap_handles,
            });
            Root::new(host, window, cx)
        }
    });
    let Some(handles) = handles.borrow().clone() else {
        unreachable!("焦点句柄应随窗口创建");
    };
    visual.run_until_parked();

    visual.update(|window, cx| {
        handles.last.focus(window, cx);
        window.draw(cx).clear(cx);
        assert!(
            gpui_kit::base::active_focus_trap(window, cx)
                .is_some_and(|trap| trap.contains(&handles.last, window)),
            "模态元素应保留焦点陷阱注册",
        );
    });
    visual.simulate_keystrokes("tab");
    visual.run_until_parked();

    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert!(handles.first.is_focused(window), "Tab 应在对话框内循环");
        assert!(
            gpui_kit::base::active_focus_trap(window, cx)
                .is_some_and(|trap| trap.contains(&handles.first, window)),
            "焦点陷阱应继续包含循环后的焦点",
        );
    });
}
