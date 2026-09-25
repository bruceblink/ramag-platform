use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{AppContext as _, Entity, Point, Size, TestAppContext, px, size};
use ramag_app::ConnectionService;
use ramag_domain::entities::{ConnectionConfig, ConnectionId, QueryRecord, QueryRecordId};
use ramag_domain::error::Result;
use ramag_domain::traits::Storage;

use super::{
    DragState, MODAL_HEIGHT, MODAL_WIDTH, TablePropertiesDialog, clamp_position, modal_size,
};

#[derive(Default)]
struct NoopStorage;

#[async_trait::async_trait]
impl Storage for NoopStorage {
    async fn list_connections(&self) -> Result<Vec<ConnectionConfig>> {
        Ok(Vec::new())
    }

    async fn get_connection(&self, _id: &ConnectionId) -> Result<Option<ConnectionConfig>> {
        Ok(None)
    }

    async fn save_connection(&self, _config: &ConnectionConfig) -> Result<()> {
        Ok(())
    }

    async fn delete_connection(&self, _id: &ConnectionId) -> Result<()> {
        Ok(())
    }

    async fn append_history(&self, _record: &QueryRecord) -> Result<()> {
        Ok(())
    }

    async fn list_history(
        &self,
        _connection_id: Option<&ConnectionId>,
        _limit: usize,
    ) -> Result<Vec<QueryRecord>> {
        Ok(Vec::new())
    }

    async fn delete_history(&self, _id: &QueryRecordId) -> Result<()> {
        Ok(())
    }

    async fn clear_history(&self, _connection_id: Option<&ConnectionId>) -> Result<()> {
        Ok(())
    }

    async fn get_preference(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn set_preference(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }
}

fn test_dialog(
    cx: &mut TestAppContext,
) -> (
    Entity<TablePropertiesDialog>,
    &mut gpui_kit::VisualTestContext,
) {
    let service = Arc::new(ConnectionService::new(
        HashMap::new(),
        Arc::new(NoopStorage),
    ));
    let mut dialog_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let dialog = cx.new(|cx| TablePropertiesDialog {
            service: service.clone(),
            connection: ConnectionConfig::new_mysql("测试连接", "127.0.0.1", 3306, "root"),
            schema: "public".into(),
            table: "users".into(),
            is_view: false,
            ddl_loading: false,
            ddl_text: Some("CREATE TABLE users (id INTEGER);".into()),
            ddl_error: None,
            request_generation: 0,
            position: None,
            drag_state: None::<DragState>,
            focus_handle: cx.focus_handle(),
            ddl_vertical_scroll: gpui_kit::ScrollHandle::new(),
            ddl_horizontal_scroll: gpui_kit::ScrollHandle::new(),
        });
        dialog_entity = Some(dialog.clone());
        gpui_kit::component::Root::new(dialog, window, cx)
    });
    (dialog_entity.expect("表属性对话框应创建"), visual_cx)
}

#[test]
fn dragged_modal_stays_inside_viewport() {
    let viewport = Size::new(px(1440.0), px(900.0));
    let position = clamp_position(
        Point::new(px(-100.0), px(900.0)),
        viewport,
        modal_size(viewport),
    );
    assert_eq!(position.x, px(16.0));
    assert_eq!(position.y, px(234.0));
}

#[test]
fn modal_fits_narrow_viewports_before_dragging() {
    let viewport = Size::new(px(900.0), px(500.0));
    let size = modal_size(viewport);
    assert_eq!(size.width, px(868.0));
    assert_eq!(size.height, px(468.0));
    assert_eq!(
        clamp_position(Point::new(px(0.0), px(0.0)), viewport, size),
        Point::new(px(16.0), px(16.0))
    );
}

#[test]
fn modal_dimensions_match_the_drag_bounds() {
    assert_eq!(MODAL_WIDTH, 1160.0);
    assert_eq!(MODAL_HEIGHT, 650.0);
}

#[gpui_kit::test]
fn table_properties_only_renders_the_ddl_preview(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_dialog, cx) = test_dialog(cx);
    cx.simulate_resize(size(px(1024.0), px(720.0)));
    cx.run_until_parked();

    let ddl_panel = cx
        .debug_bounds("table-properties-ddl-scroll")
        .expect("表属性应保留 DDL 预览");
    assert!(cx.debug_bounds("table-properties-triggers").is_none());
    let modal = cx
        .debug_bounds("table-properties-modal")
        .expect("表属性弹窗应渲染");
    assert!(ddl_panel.origin.y >= modal.origin.y);
    assert!(ddl_panel.bottom() <= modal.bottom());
}

#[gpui_kit::test]
fn ddl_preview_stays_inside_a_narrow_modal(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_dialog, cx) = test_dialog(cx);
    cx.simulate_resize(size(px(420.0), px(320.0)));
    cx.run_until_parked();

    let ddl_panel = cx
        .debug_bounds("table-properties-ddl-scroll")
        .expect("窄窗口应渲染 DDL 预览");
    assert!(ddl_panel.origin.x >= px(16.0));
    assert!(ddl_panel.right() <= px(404.0));
}

#[gpui_kit::test]
fn ddl_preview_stays_inside_a_compact_modal(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_dialog, cx) = test_dialog(cx);
    cx.simulate_resize(size(px(360.0), px(240.0)));
    cx.run_until_parked();

    let modal = cx
        .debug_bounds("table-properties-modal")
        .expect("窄窗口应渲染表属性弹窗");
    let ddl_panel = cx
        .debug_bounds("table-properties-ddl-scroll")
        .expect("窄窗口应保留 DDL 预览");

    assert!(ddl_panel.origin.y >= modal.origin.y);
    assert!(ddl_panel.bottom() <= modal.bottom());
    assert!(ddl_panel.size.height > px(0.0));
}
