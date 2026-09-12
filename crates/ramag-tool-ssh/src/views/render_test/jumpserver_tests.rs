use super::*;

fn assert_inside(
    parent: gpui::Bounds<gpui::Pixels>,
    child: gpui::Bounds<gpui::Pixels>,
    label: &str,
) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 越出父容器：parent={parent:?}, child={child:?}"
    );
}

include!("jumpserver_tests/part01.rs");
include!("jumpserver_tests/part02.rs");
