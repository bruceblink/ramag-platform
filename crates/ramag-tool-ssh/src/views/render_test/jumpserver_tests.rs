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

include!("jumpserver_tests/connection_and_session_tests.rs");
include!("jumpserver_tests/connection_editor_tests.rs");
