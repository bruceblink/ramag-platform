//! 对象存储账号列表的窄窗口布局回归测试。

use std::sync::Arc;

use gpui::{TestAppContext, px, size};
use ramag_domain::entities::{CloudProvider, ManualBucket, ObjectStorageAccount};

use super::{add_workspace_window, service};

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

/// 账号行中的固定徽标和操作组在窄窗口内应换行，而不是推出列表内容区。
#[gpui::test]
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
