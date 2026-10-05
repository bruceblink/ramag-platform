use std::sync::Arc;

use gpui_kit::{Modifiers, TestAppContext, VisualTestContext, px, size};
use ramag_domain::entities::{
    CloudProvider, HttpsEndpoint, ObjectStorageAccount, ObjectStorageMount, ObjectStorageMountId,
};

use super::super::mount_sort::{MountSort, MountSortColumn};
use super::{add_workspace_window, service};

fn mount(
    account: &ObjectStorageAccount,
    bucket: &str,
    root_prefix: Option<&str>,
) -> ObjectStorageMount {
    ObjectStorageMount {
        id: ObjectStorageMountId::new(),
        account_id: account.id.clone(),
        bucket: bucket.into(),
        region: "cn-hangzhou".into(),
        endpoint: HttpsEndpoint::parse_official(
            CloudProvider::AliyunOss,
            "https://oss-cn-hangzhou.aliyuncs.com",
        )
        .expect("test endpoint should be valid"),
        root_prefix: root_prefix.map(str::to_owned),
        created_at: None,
        storage_class: None,
    }
}

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} 应显示为可点击的排序列头"));
    visual.simulate_click(bounds.center(), Modifiers::default());
    visual.run_until_parked();
}

fn row_y(visual: &mut VisualTestContext, bucket: &'static str) -> gpui_kit::Pixels {
    visual
        .debug_bounds(match bucket {
            "Alpha" => "object-mount-row-Alpha",
            "beta" => "object-mount-row-beta",
            "zeta" => "object-mount-row-zeta",
            _ => panic!("unexpected test bucket"),
        })
        .unwrap_or_else(|| panic!("{bucket} 挂载行应参与布局"))
        .origin
        .y
}

#[gpui_kit::test]
fn mount_table_columns_sort_rows_and_preserve_selected_mount(cx: &mut TestAppContext) {
    let (view, visual) = add_workspace_window(cx, service());
    visual.run_until_parked();
    let account = ObjectStorageAccount::new("test-account", CloudProvider::AliyunOss);
    let mounts = vec![
        mount(&account, "zeta", None),
        mount(&account, "beta", Some("a/")),
        mount(&account, "Alpha", Some("z/")),
    ];
    let selected_mount = mounts[1].clone();
    let selected_id = selected_mount.id.clone();
    view.update(visual, |view, cx| {
        view.accounts = Arc::new(vec![account.clone()]);
        view.selected_account_id = Some(account.id.clone());
        view.management_visible = false;
        view.mounts = Arc::new(mounts);
        view.selected_mount = Some(selected_mount);
        view.show_mounts = true;
        view.loading = false;
        cx.notify();
    });
    visual.simulate_resize(size(px(1200.0), px(800.0)));
    visual.run_until_parked();

    let headers = visual
        .debug_bounds("object-mount-columns")
        .expect("Bucket 表应显示表头");
    for selector in ["object-mount-sort-bucket", "object-mount-sort-root-path"] {
        let header = visual
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} 应显示为可操作表头"));
        assert!(header.origin.x >= headers.origin.x && header.right() <= headers.right());
    }

    click(visual, "object-mount-sort-bucket");
    assert_eq!(
        visual.update(|_, app| view.read(app).mount_sort),
        Some(MountSort {
            column: MountSortColumn::Bucket,
            ascending: true,
        })
    );
    assert!(row_y(visual, "Alpha") < row_y(visual, "beta"));
    assert!(row_y(visual, "beta") < row_y(visual, "zeta"));

    click(visual, "object-mount-sort-bucket");
    assert_eq!(
        visual.update(|_, app| view.read(app).mount_sort),
        Some(MountSort {
            column: MountSortColumn::Bucket,
            ascending: false,
        })
    );
    assert!(row_y(visual, "zeta") < row_y(visual, "beta"));
    assert!(row_y(visual, "beta") < row_y(visual, "Alpha"));

    click(visual, "object-mount-sort-root-path");
    assert_eq!(
        visual.update(|_, app| {
            let view = view.read(app);
            (
                view.mount_sort,
                view.selected_mount.as_ref().map(|mount| mount.id.clone()),
            )
        }),
        (
            Some(MountSort {
                column: MountSortColumn::RootPath,
                ascending: true,
            }),
            Some(selected_id),
        )
    );
    assert!(row_y(visual, "zeta") < row_y(visual, "beta"));
    assert!(row_y(visual, "beta") < row_y(visual, "Alpha"));
}
