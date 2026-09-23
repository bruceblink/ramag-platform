#![allow(clippy::expect_used)]

use super::{LinesEditor, LinesKind};
use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, Bounds, Modifiers, MouseButton, Pixels, TestAppContext, px, size};

fn assert_inside(parent: &Bounds<Pixels>, child: &Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 越出父容器：parent={parent:?}, child={child:?}"
    );
}

/// List 编辑器的添加、数量和插入方向控件在窄窗口中保持可见且不越界。
#[gpui_kit::test]
fn lines_toolbar_wraps_controls_inside_supported_widths(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let editor = cx.new(|cx| LinesEditor::new(LinesKind::List, window, cx));
        Root::new(editor, window, cx)
    });

    for width in [180.0, 280.0, 600.0] {
        cx.simulate_resize(size(px(width), px(360.0)));
        cx.run_until_parked();

        let editor = cx
            .debug_bounds("redis-lines-editor")
            .expect("Redis List 编辑器应渲染");
        let toolbar = cx
            .debug_bounds("redis-lines-toolbar")
            .expect("Redis List 工具栏应渲染");
        let add = cx.debug_bounds("redis-lines-add").expect("添加按钮应渲染");
        let count = cx
            .debug_bounds("redis-lines-count")
            .expect("行数状态应渲染");
        let label = cx
            .debug_bounds("redis-lines-direction-label")
            .expect("插入位置标签应渲染");
        let head = cx
            .debug_bounds("redis-lines-dir-head")
            .expect("LPUSH 方向按钮应渲染");
        let tail = cx
            .debug_bounds("redis-lines-dir-tail")
            .expect("RPUSH 方向按钮应渲染");

        assert_inside(&editor, &toolbar, "List 工具栏");
        for (control, name) in [
            (add, "添加按钮"),
            (count, "行数状态"),
            (label, "插入位置标签"),
            (head, "LPUSH 方向按钮"),
            (tail, "RPUSH 方向按钮"),
        ] {
            assert_inside(&toolbar, &control, name);
            assert!(control.size.width > px(0.0), "{name} 不能为零宽");
            assert!(control.size.height > px(0.0), "{name} 不能为零高");
        }

        if width == 180.0 {
            assert!(
                tail.origin.y > add.origin.y,
                "最小宽度应让插入方向控件换行：add={add:?}, tail={tail:?}"
            );
        }
    }
}

#[gpui_kit::test]
fn lines_editor_add_button_appends_a_row(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let editor = cx.new(|cx| LinesEditor::new(LinesKind::List, window, cx));
        Root::new(editor, window, cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("redis-lines-row-0").is_some());

    let add = cx
        .debug_bounds("redis-lines-add")
        .expect("List 编辑器应显示添加按钮");
    let position = add.center();
    cx.simulate_mouse_move(position, None, Modifiers::default());
    cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();

    assert!(cx.debug_bounds("redis-lines-row-1").is_some());
}
