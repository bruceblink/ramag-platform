//! 首页分类菜单的真实 GPUI 无障碍交互测试。

use gpui_kit::{TestAppContext, px, size, test::TestQueryExt as _};

use super::visual_tests::{
    HomeTestHost, click, click_accessible_label, registry_with_catalog_tools,
};

/// 打开真实分类菜单，核验六个选项都有 MenuItem 角色和可读名称。
#[gpui_kit::test]
fn home_category_menu_exposes_six_accessible_menu_items(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, visual) =
        cx.add_window_view(move |_, cx| HomeTestHost::new(registry_with_catalog_tools(), cx));
    visual.simulate_resize(size(px(720.0), px(640.0)));
    visual.run_until_parked();

    click(visual, "home-category");
    let labels = visual.update(|window, _| {
        window
            .find_all_by_role(gpui_kit::Role::MenuItem)
            .into_iter()
            .map(|item| item.label().map(str::to_owned))
            .collect::<Vec<_>>()
    });
    assert_eq!(labels.len(), 6, "分类菜单应暴露全部六个选项");
    assert!(
        labels.iter().all(Option::is_some),
        "每个分类菜单项都应有辅助名称：{labels:?}"
    );
    assert_eq!(
        labels.into_iter().flatten().collect::<Vec<_>>(),
        ["全部分类", "数据", "接口与消息", "运维", "本机", "其他"]
    );

    click_accessible_label(visual, "数据");
    assert_eq!(
        visual.update(|window, _| window.find_all_by_label("工具分类：数据").len()),
        1,
        "选择分类后，触发按钮应读出当前分类"
    );
}
