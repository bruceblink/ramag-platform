use std::sync::Arc;

use async_trait::async_trait;
use gpui_kit::{
    AppContext as _, Bounds, Entity, IntoElement, ParentElement as _, Pixels, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, px, size,
};
use ramag_app::ClipboardService;
use ramag_domain::entities::{
    CapturedClip, ClipSource, ConnectionConfig, ConnectionId, QueryRecord, QueryRecordId,
};
use ramag_domain::error::Result;
use ramag_domain::traits::{ClipboardDriver, Storage};

use super::ClipboardView;
use super::clipboard_status_label;

struct TestClipboard;

impl ClipboardDriver for TestClipboard {
    fn change_count(&self) -> i64 {
        0
    }

    fn own_change_count(&self) -> i64 {
        0
    }

    fn read(&self) -> Result<Option<CapturedClip>> {
        Ok(None)
    }

    fn write_text(&self, _: &str, _: Option<&[u8]>) -> Result<()> {
        Ok(())
    }

    fn write_image_png(&self, _: &[u8]) -> Result<()> {
        Ok(())
    }

    fn write_files(&self, _: &[String]) -> Result<()> {
        Ok(())
    }

    fn frontmost_app(&self) -> Option<ClipSource> {
        None
    }

    fn app_icon_png(&self, _: &str) -> Option<Arc<Vec<u8>>> {
        None
    }

    fn persist_media(&self, key: &str, _: &[u8]) -> Result<String> {
        Ok(key.to_string())
    }

    fn read_media(&self, _: &str) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    fn list_media(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn remove_media(&self, _: &str) -> Result<()> {
        Ok(())
    }

    fn clear_media(&self) -> Result<()> {
        Ok(())
    }

    fn accessibility_trusted(&self, _: bool) -> bool {
        false
    }

    fn paste_to_app(&self, _: Option<&str>) -> Result<()> {
        Ok(())
    }

    fn open_url(&self, _: &str) -> Result<()> {
        Ok(())
    }

    fn reveal_in_file_manager(&self, _: &[String]) -> Result<()> {
        Ok(())
    }

    fn paths_exist(&self, _: &[String]) -> bool {
        false
    }
}

#[derive(Default)]
struct TestStorage {
    rows: std::sync::Mutex<Vec<ramag_domain::entities::ClipItem>>,
    fail_delete: std::sync::atomic::AtomicBool,
    fail_search: std::sync::atomic::AtomicBool,
    hold_search: std::sync::atomic::AtomicBool,
    search_held: std::sync::atomic::AtomicBool,
    release_search: std::sync::atomic::AtomicBool,
    search_calls: std::sync::atomic::AtomicUsize,
    search_waker: std::sync::Mutex<Option<std::task::Waker>>,
}

struct ClipboardTestHost {
    view: Entity<ClipboardView>,
}

impl Render for ClipboardTestHost {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(gpui_kit::component::Root::render_notification_layer(
                window, cx,
            ))
    }
}

impl TestStorage {
    fn release_held_search(&self) {
        self.release_search
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(waker) = self
            .search_waker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            waker.wake();
        }
    }
}

#[async_trait]
impl Storage for TestStorage {
    async fn clip_list(&self) -> Result<Vec<ramag_domain::entities::ClipItem>> {
        Ok(self
            .rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone())
    }

    async fn clip_list_recent(&self, _: usize) -> Result<Vec<ramag_domain::entities::ClipItem>> {
        // Model an older entry returned by full search but outside the recent cache.
        Ok(Vec::new())
    }

    async fn clip_save(&self, item: &ramag_domain::entities::ClipItem) -> Result<()> {
        let mut rows = self
            .rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        rows.retain(|row| row.id != item.id);
        rows.push(item.clone());
        Ok(())
    }

    async fn clip_find_by_hash(
        &self,
        hash: &str,
    ) -> Result<Option<ramag_domain::entities::ClipItem>> {
        Ok(self
            .rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .find(|row| row.content_hash == hash)
            .cloned())
    }

    async fn clip_delete(&self, id: &ramag_domain::entities::ClipId) -> Result<()> {
        if self.fail_delete.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(ramag_domain::error::DomainError::Storage(
                "test delete failure".into(),
            ));
        }
        self.rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|row| &row.id != id);
        Ok(())
    }

    async fn clip_search_cancellable(
        &self,
        query: &str,
        limit: usize,
        _: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Vec<ramag_domain::entities::ClipItem>> {
        use std::sync::atomic::Ordering;
        self.search_calls.fetch_add(1, Ordering::SeqCst);
        let snapshot = self
            .rows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|row| row.text.as_deref().is_some_and(|text| text.contains(query)))
            .take(limit)
            .cloned()
            .collect();
        let fail_search = self.fail_search.swap(false, Ordering::SeqCst);
        if self.hold_search.swap(false, Ordering::SeqCst) {
            self.search_held.store(true, Ordering::SeqCst);
            std::future::poll_fn(|cx| {
                *self
                    .search_waker
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(cx.waker().clone());
                if self.release_search.load(Ordering::SeqCst) {
                    std::task::Poll::Ready(())
                } else {
                    std::task::Poll::Pending
                }
            })
            .await;
        }
        if fail_search {
            return Err(ramag_domain::error::DomainError::Storage(
                "test search failure".into(),
            ));
        }
        // Deliberately ignore cancellation to verify the view also rejects an obsolete generation.
        Ok(snapshot)
    }
    async fn list_connections(&self) -> Result<Vec<ConnectionConfig>> {
        Ok(Vec::new())
    }

    async fn get_connection(&self, _: &ConnectionId) -> Result<Option<ConnectionConfig>> {
        Ok(None)
    }

    async fn save_connection(&self, _: &ConnectionConfig) -> Result<()> {
        Ok(())
    }

    async fn delete_connection(&self, _: &ConnectionId) -> Result<()> {
        Ok(())
    }

    async fn append_history(&self, _: &QueryRecord) -> Result<()> {
        Ok(())
    }

    async fn list_history(&self, _: Option<&ConnectionId>, _: usize) -> Result<Vec<QueryRecord>> {
        Ok(Vec::new())
    }

    async fn delete_history(&self, _: &QueryRecordId) -> Result<()> {
        Ok(())
    }

    async fn clear_history(&self, _: Option<&ConnectionId>) -> Result<()> {
        Ok(())
    }

    async fn get_preference(&self, _: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn set_preference(&self, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
}

fn add_clipboard_window(
    cx: &mut TestAppContext,
) -> (Option<Entity<ClipboardView>>, &mut VisualTestContext) {
    cx.update(gpui_kit::component::init);
    let mut view = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let service = Arc::new(ClipboardService::new(
            Arc::new(TestClipboard),
            Arc::new(TestStorage::default()),
        ));
        let entity = cx.new(|cx| ClipboardView::new(service, window, cx));
        view = Some(entity.clone());
        let host = cx.new(|_| ClipboardTestHost { view: entity });
        gpui_kit::component::Root::new(host, window, cx)
    });
    (view, visual_cx)
}

#[path = "search_mutation_tests.rs"]
mod search_mutation_tests;

fn assert_inside(parent: &Bounds<Pixels>, child: &Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x
            && child.origin.y >= parent.origin.y
            && child.right() <= parent.right()
            && child.bottom() <= parent.bottom(),
        "{label} 越出父容器：parent={parent:?}, child={child:?}"
    );
}

#[test]
fn clipboard_status_includes_readable_content_size() {
    assert_eq!(
        clipboard_status_label(257, 18 * 1024 * 1024, false),
        "257 条 · 占用 18.0 MiB"
    );
}

#[test]
fn truncated_search_status_keeps_size_and_limit_hint() {
    assert_eq!(
        clipboard_status_label(500, 2048, true),
        "显示 500 条 · 2 KiB · 历史至少 500 条（仅加载前 500 条）"
    );
}

/// 搜索框和类型筛选在主页面工具栏中应随窗口宽度换行，且每个按钮都留在筛选区内。
#[gpui_kit::test]
fn clipboard_toolbar_wraps_search_and_filters_inside_supported_widths(cx: &mut TestAppContext) {
    let (_, cx) = add_clipboard_window(cx);

    for width in [180.0, 240.0, 360.0, 600.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(480.0)));
        cx.run_until_parked();

        let toolbar = cx.debug_bounds("clipboard-view-toolbar");
        assert!(toolbar.is_some(), "剪贴板主页面工具栏应渲染");
        let Some(toolbar) = toolbar else { continue };
        let page_title = cx.debug_bounds("clipboard-view-page-title");
        assert!(page_title.is_some(), "剪贴板 Pulse 页面标题应渲染");
        if let Some(page_title) = page_title {
            assert!(
                page_title.right() <= toolbar.right(),
                "剪贴板页面标题不能越出工具栏宽度: title={page_title:?}, toolbar={toolbar:?}"
            );
        }
        let search_pane = cx.debug_bounds("clipboard-view-search-pane");
        assert!(search_pane.is_some(), "剪贴板搜索区应渲染");
        let Some(search_pane) = search_pane else {
            continue;
        };
        let search = cx.debug_bounds("clipboard-view-search");
        assert!(search.is_some(), "剪贴板搜索框应渲染");
        let Some(search) = search else { continue };
        let filters = cx.debug_bounds("clipboard-view-filters");
        assert!(filters.is_some(), "剪贴板筛选区应渲染");
        let Some(filters) = filters else { continue };

        assert_inside(&toolbar, &search_pane, "搜索区");
        assert_inside(&search_pane, &search, "搜索框");
        assert_inside(&toolbar, &filters, "筛选区");
        for selector in [
            "clipboard-filter-all",
            "clipboard-filter-文本",
            "clipboard-filter-链接",
            "clipboard-filter-颜色",
            "clipboard-filter-图片",
            "clipboard-filter-文件",
        ] {
            let button = cx.debug_bounds(selector);
            assert!(button.is_some(), "{selector} 应渲染");
            if let Some(button) = button {
                assert_inside(&filters, &button, selector);
            }
        }

        if width <= 240.0 {
            assert!(
                filters.origin.y > search_pane.origin.y,
                "窄窗口应让筛选区换到搜索区下方：search={search_pane:?}, filters={filters:?}"
            );
        }
    }
}

/// 紧凑窗口将列表和详情上下排列；常规窗口保留固定列表栏与可收缩详情栏。
#[gpui_kit::test]
fn clipboard_content_reflows_list_and_detail_inside_supported_widths(cx: &mut TestAppContext) {
    let (_, cx) = add_clipboard_window(cx);

    for width in [360.0, 800.0, 1024.0, 1440.0] {
        cx.simulate_resize(size(px(width), px(480.0)));
        cx.run_until_parked();

        let content = cx.debug_bounds("clipboard-view-content");
        assert!(content.is_some(), "剪贴板内容区应渲染");
        let Some(content) = content else { continue };
        let list = cx.debug_bounds("clipboard-view-list-pane");
        assert!(list.is_some(), "剪贴板列表区应渲染");
        let Some(list) = list else { continue };
        let detail = cx.debug_bounds("clipboard-view-detail-pane");
        assert!(detail.is_some(), "剪贴板详情区应渲染");
        let Some(detail) = detail else { continue };

        assert_inside_horizontal(&content, &list, "列表区");
        assert_inside_horizontal(&content, &detail, "详情区");
        if width < 900.0 {
            assert!(
                detail.origin.y >= list.bottom(),
                "紧凑窗口应将详情区放在列表区下方：list={list:?}, detail={detail:?}"
            );
            assert!(
                list.size.height >= px(240.0) && detail.size.height >= px(240.0),
                "紧凑窗口列表和详情应填充可用高度且保留最小可读区域：list={list:?}, detail={detail:?}"
            );
        } else {
            assert_inside(&content, &list, "列表区");
            assert_inside(&content, &detail, "详情区");
            assert!(
                detail.origin.x >= list.right(),
                "常规窗口应将详情区放在列表区右侧：list={list:?}, detail={detail:?}"
            );
        }
    }
}

fn assert_inside_horizontal(parent: &Bounds<Pixels>, child: &Bounds<Pixels>, label: &str) {
    assert!(
        child.origin.x >= parent.origin.x && child.right() <= parent.right(),
        "{label} 横向越出父容器：parent={parent:?}, child={child:?}"
    );
}
