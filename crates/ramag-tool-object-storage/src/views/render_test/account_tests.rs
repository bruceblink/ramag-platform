//! 对象存储账号列表的窄窗口布局回归测试。

use std::sync::Arc;

use gpui_kit::{TestAppContext, px, size};
use ramag_domain::entities::{CloudProvider, ManualBucket, ObjectStorageAccount};
use ramag_ui::Mode;

use super::{add_form_window, add_workspace_window, service};

fn assert_inside(
    parent: gpui_kit::Bounds<gpui_kit::Pixels>,
    child: gpui_kit::Bounds<gpui_kit::Pixels>,
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

/// 账号行中的固定徽标和操作组在窄窗口内应换行，而不是推出列表内容区。
#[gpui_kit::test]
fn account_rows_stay_inside_supported_window_widths(cx: &mut TestAppContext) {
    let (view, cx) = add_workspace_window(cx, service());
    cx.run_until_parked();

    let mut account = ObjectStorageAccount::new(
        "production-account-with-a-long-name",
        CloudProvider::AliyunOss,
    );
    account.read_only = true;
    account.manual_buckets = vec![
        ManualBucket::new("logs-bucket", "cn-hangzhou"),
        ManualBucket::new("archive-bucket", "cn-hangzhou"),
    ];
    view.update(cx, |view, cx| {
        view.accounts = Arc::new(vec![account]);
        view.loading = false;
        view.management_visible = true;
        cx.notify();
    });

    for (width, manual_count_visible) in [(360.0, false), (1024.0, true), (1440.0, true)] {
        cx.simulate_resize(size(px(width), px(720.0)));
        cx.run_until_parked();

        let row = cx
            .debug_bounds("object-account-row-0")
            .expect("对象存储账号行应渲染");
        assert!(
            row.origin.x >= px(0.0) && row.right() <= px(width),
            "对象存储账号行不能越出窗口：row={row:?}, width={width}"
        );

        for selector in [
            "object-account-provider-0",
            "object-account-provider-icon-0",
            "object-account-read-only-0",
            "object-account-actions-0",
        ] {
            let child = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} 应渲染"));
            assert_inside(row, child, selector);
        }
        if manual_count_visible {
            let count = cx
                .debug_bounds("object-account-bucket-count-0")
                .expect("Bucket 数量应在宽窗口显示");
            assert_inside(row, count, "对象存储 Bucket 数量");
        } else {
            assert!(
                cx.debug_bounds("object-account-bucket-count-0").is_none(),
                "窄窗口不应强行显示 Bucket 数量列"
            );
        }
    }
}

#[gpui_kit::test]
fn account_form_stays_inside_compact_window_and_keeps_actions_visible(cx: &mut TestAppContext) {
    let (_, cx) = add_form_window(cx, service());

    for height in [240.0, 640.0] {
        cx.simulate_resize(size(px(360.0), px(height)));
        cx.run_until_parked();

        let viewport = size(px(360.0), px(height));
        let body = cx
            .debug_bounds("object-account-form-body")
            .expect("紧凑窗口应保留可滚动账号表单主体");
        let footer = cx
            .debug_bounds("object-account-form-footer")
            .expect("紧凑窗口应保留账号表单底部操作区");
        assert!(body.origin.x >= px(0.0) && body.right() <= viewport.width);
        assert!(
            footer.origin.x >= px(0.0)
                && footer.right() <= viewport.width
                && footer.origin.y >= px(0.0)
                && footer.bottom() <= viewport.height,
            "底部操作区必须留在 {height}px 视口内：{footer:?}"
        );

        for selector in [
            "object-provider-tencent-cos",
            "object-provider-aliyun-oss",
            "object-account-name-field",
            "object-access-key-id-field",
            "object-manual-bucket-field",
        ] {
            let bounds = cx
                .debug_bounds(selector)
                .expect("账号表单控件应在紧凑窗口中参与布局");
            assert!(
                bounds.origin.x >= px(0.0) && bounds.right() <= viewport.width,
                "{selector} 越出窗口：{bounds:?}"
            );
        }
    }
}

/// 验证账号管理的 Pulse 标题、响应式工具栏和轻量列表面板不越出客户区。
#[gpui_kit::test]
fn account_manager_uses_shared_page_hierarchy_at_supported_widths(cx: &mut TestAppContext) {
    let (view, cx) = add_workspace_window(cx, service());
    view.update(cx, |view, cx| {
        view.accounts = Arc::new(
            (0..32)
                .map(|index| {
                    let provider = if index % 2 == 0 {
                        CloudProvider::AliyunOss
                    } else {
                        CloudProvider::TencentCos
                    };
                    ObjectStorageAccount::new(format!("account-{index:02}"), provider)
                })
                .collect(),
        );
        view.loading = false;
        view.management_visible = true;
        cx.notify();
    });

    for mode in [Mode::Light, Mode::Dark] {
        cx.update(|_, app| ramag_ui::apply_theme(mode, app));
        for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
            cx.simulate_resize(size(px(width), px(height)));
            cx.run_until_parked();

            let title = cx
                .debug_bounds("object-account-page-title")
                .expect("账号管理页标题应显示");
            let toolbar = cx
                .debug_bounds("object-account-toolbar")
                .expect("账号管理工具栏应显示");
            let search = cx
                .debug_bounds("object-account-search-input")
                .expect("账号搜索框应显示");
            let create = cx
                .debug_bounds("object-new-account")
                .expect("新建账号入口应显示");
            let list = cx
                .debug_bounds("object-account-list-panel")
                .expect("账号列表面板应显示");
            let table_header = if width >= 900.0 {
                Some(
                    cx.debug_bounds("object-account-table-header")
                        .expect("宽窗口账号列表应显示表头"),
                )
            } else {
                None
            };
            let scroll = cx
                .debug_bounds("object-account-list-scroll")
                .expect("账号行应保留面板内的滚动区域");
            let last_row = cx
                .debug_bounds("object-account-row-31")
                .expect("长账号列表末行应参与布局");

            for (name, bounds) in [
                ("title", title),
                ("toolbar", toolbar),
                ("search", search),
                ("create", create),
                ("list", list),
            ] {
                assert!(
                    bounds.left() >= px(0.0),
                    "{name} exceeds left edge: {bounds:?}"
                );
                assert!(
                    bounds.right() <= px(width),
                    "{name} exceeds right edge: {bounds:?}"
                );
                assert!(
                    bounds.bottom() <= px(height),
                    "{name} exceeds bottom edge: {bounds:?}"
                );
            }
            assert!(
                scroll.origin.x >= list.origin.x
                    && scroll.origin.y >= list.origin.y
                    && scroll.right() <= list.right()
                    && scroll.bottom() > list.bottom(),
                "长列表的滚动内容应在面板内部延伸并由视口裁切：list={list:?}, scroll={scroll:?}"
            );
            assert!(
                list.size.height >= px(400.0) && scroll.size.height >= px(380.0),
                "账号列表和内部滚动区域应填充剩余工作区：list={list:?}, scroll={scroll:?}"
            );
            assert!(
                last_row.origin.y >= scroll.origin.y && last_row.bottom() > list.bottom(),
                "长列表末行应位于可滚动视口之后：last_row={last_row:?}, scroll={scroll:?}"
            );
            assert!(title.bottom() <= toolbar.top(), "标题应位于工具栏上方");
            assert!(toolbar.bottom() <= list.top(), "工具栏应位于账号列表上方");
            assert!(search.right() <= create.left(), "新建入口不得覆盖搜索框");
            assert!(
                list.size.height >= px(400.0),
                "账号列表面板应填充标题区下方的剩余空间：{list:?} at {width}x{height}"
            );
            if let Some(table_header) = table_header {
                assert_inside(list, table_header, "对象存储账号表头");
                for selector in [
                    "object-account-table-header-name",
                    "object-account-table-header-provider",
                    "object-account-table-header-status",
                    "object-account-table-header-buckets",
                    "object-account-table-header-actions",
                ] {
                    let column = cx
                        .debug_bounds(selector)
                        .unwrap_or_else(|| panic!("{selector} 应渲染"));
                    assert_inside(table_header, column, selector);
                }
            }
        }
    }
}

/// Empty accounts should keep the Pulse panel and setup action inside supported layouts.
#[gpui_kit::test]
fn empty_account_state_stays_inside_supported_window_sizes_and_themes(cx: &mut TestAppContext) {
    let (view, cx) = add_workspace_window(cx, service());
    view.update(cx, |view, cx| {
        view.accounts = Arc::new(Vec::new());
        view.loading = false;
        view.management_visible = true;
        cx.notify();
    });

    for mode in [Mode::Light, Mode::Dark] {
        cx.update(|_, app| ramag_ui::apply_theme(mode, app));
        for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
            cx.simulate_resize(size(px(width), px(height)));
            cx.run_until_parked();

            let body = cx
                .debug_bounds("object-account-empty-body")
                .expect("账号空状态主体应渲染");
            let panel = cx
                .debug_bounds("object-account-empty-state")
                .expect("账号空状态应使用 Pulse 面板");
            let title = cx
                .debug_bounds("object-account-empty-title")
                .expect("账号空状态标题应渲染");
            let description = cx
                .debug_bounds("object-account-empty-description")
                .expect("账号空状态说明应渲染");
            let action = cx
                .debug_bounds("object-account-empty-cta")
                .expect("新建账号操作应渲染");

            assert_inside(body, panel, "对象存储账号空状态面板");
            assert_inside(panel, title, "对象存储账号空状态标题");
            assert_inside(panel, description, "对象存储账号空状态说明");
            assert_inside(panel, action, "对象存储账号空状态操作");
            assert!(
                panel.size.width <= px(480.0) && panel.size.height >= px(220.0),
                "空状态面板应保持有界尺寸：panel={panel:?}"
            );
            assert!(
                title.bottom() <= description.top() && description.bottom() <= action.top(),
                "空状态应保留标题、说明、操作顺序：title={title:?}, description={description:?}, action={action:?}"
            );
        }
    }
}
