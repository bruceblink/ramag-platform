use super::*;

use gpui_kit::{TestAppContext, component, px, size};
use ramag_infra_storage::RedbStorage;
use tempfile::TempDir;

#[test]
fn local_storage_can_be_injected_without_network_clients() {
    let directory = TempDir::new().expect("temp directory");
    let storage =
        RedbStorage::open_with_key(&directory.path().join("collaboration.redb"), &[7; 32])
            .expect("storage opens");
    let _service = CollaborationService::new(Arc::new(storage));
    assert!(directory.path().join("collaboration.redb").exists());
}

#[gpui_kit::test]
fn collaboration_view_keeps_confirmation_controls_inside_supported_widths(cx: &mut TestAppContext) {
    cx.update(component::init);
    let directory = TempDir::new().expect("temp directory");
    let storage = Arc::new(
        RedbStorage::open_with_key(&directory.path().join("collaboration.redb"), &[8; 32])
            .expect("storage opens"),
    );
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| CollaborationView::new(storage, None, None, window, cx));
        component::Root::new(view, window, cx)
    });
    for width in [360.0, 1024.0, 1440.0] {
        visual_cx.simulate_resize(size(px(width), px(900.0)));
        visual_cx.run_until_parked();
        assert!(visual_cx.debug_bounds("collaboration-view").is_some());
        assert!(visual_cx.debug_bounds("collaboration-create").is_some());
        assert!(visual_cx.debug_bounds("collaboration-import").is_some());
        assert!(visual_cx.debug_bounds("collaboration-publish").is_some());
        assert!(visual_cx.debug_bounds("collaboration-fetch").is_some());
        assert!(visual_cx.debug_bounds("collaboration-status").is_some());
    }
}
