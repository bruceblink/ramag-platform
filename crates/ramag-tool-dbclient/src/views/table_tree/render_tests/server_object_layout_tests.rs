use std::collections::HashSet;

use gpui_kit::{TestAppContext, px, size};
use ramag_domain::entities::{ServerObject, ServerObjectGroup, VirtualView};

use super::*;

/// Keep metadata descriptions inline within fixed-height server-object rows.
#[gpui_kit::test]
fn server_object_details_stay_inline_without_overlapping_following_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (service, redis_service, mongo_service) = build_services();
    let mut panel_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let connection_list = cx.new(|cx| {
            ConnectionListPanel::new(
                service.clone(),
                redis_service.clone(),
                mongo_service.clone(),
                window,
                cx,
            )
        });
        let panel = cx.new(|cx| {
            TableTreePanel::new(
                service.clone(),
                SchemaCache::new_shared(),
                connection_list,
                window,
                cx,
            )
        });
        panel_entity = Some(panel.clone());
        gpui_kit::component::Root::new(panel, window, cx)
    });
    let panel = panel_entity.expect("表树面板应创建");

    panel.update(cx, |panel, _| {
        panel.connection = Some(ConnectionConfig::new_mysql(
            "测试连接",
            "127.0.0.1",
            3306,
            "root",
        ));
        panel.server_objects.is_expanded = true;
        panel.server_objects.groups = vec![
            ServerObjectGroup {
                name: "collations".into(),
                items: vec![
                    ServerObject {
                        name: "armscii8_bin".into(),
                        detail: Some("armscii8".into()),
                    },
                    ServerObject {
                        name: "armscii8_general_ci".into(),
                        detail: Some("armscii8".into()),
                    },
                ],
            },
            ServerObjectGroup {
                name: "users".into(),
                items: vec![ServerObject {
                    name: "root@localhost".into(),
                    detail: None,
                }],
            },
        ];
        panel.server_objects.open_groups = HashSet::from(["collations".into(), "users".into()]);
        panel.virtual_views.is_expanded = true;
        panel.virtual_views.views = vec![VirtualView {
            name: "sessions".into(),
            detail: Some("只读会话快照".into()),
            read_only: true,
        }];
        panel.invalidate_tree_rows();
    });

    for width in [180.0, 280.0, 360.0] {
        cx.simulate_resize(size(px(width), px(640.0)));
        panel.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();

        for (name, row_selector, label_selector, detail_selector) in [
            (
                "armscii8_bin",
                "server-object-row-Item-0-armscii8_bin",
                "server-object-label-Item-0-armscii8_bin",
                "server-object-detail-Item-0-armscii8_bin",
            ),
            (
                "armscii8_general_ci",
                "server-object-row-Item-0-armscii8_general_ci",
                "server-object-label-Item-0-armscii8_general_ci",
                "server-object-detail-Item-0-armscii8_general_ci",
            ),
        ] {
            let row = cx
                .debug_bounds(row_selector)
                .unwrap_or_else(|| panic!("collation 行应渲染：{name}"));
            let label = cx
                .debug_bounds(label_selector)
                .unwrap_or_else(|| panic!("collation 名称应渲染：{name}"));
            let detail = cx
                .debug_bounds(detail_selector)
                .unwrap_or_else(|| panic!("collation 字符集说明应渲染：{name}"));
            assert_eq!(row.size.height, px(28.0));
            assert!(
                label.origin.x >= row.origin.x
                    && label.right() <= row.right()
                    && detail.right() <= row.right()
                    && label.origin.y >= row.origin.y
                    && label.bottom() <= row.bottom()
                    && detail.origin.y >= row.origin.y
                    && detail.bottom() <= row.bottom(),
                "{name} 的名称和说明都必须留在同一行：row={row:?}, label={label:?}, detail={detail:?}"
            );
            assert!(
                detail.origin.x >= label.right(),
                "{name} 的说明不能覆盖名称：label={label:?}, detail={detail:?}"
            );
        }

        let first_row = cx
            .debug_bounds("server-object-row-Item-0-armscii8_bin")
            .expect("第一条 collation 应渲染");
        let second_row = cx
            .debug_bounds("server-object-row-Item-0-armscii8_general_ci")
            .expect("第二条 collation 应渲染");
        assert!(
            second_row.origin.y >= first_row.bottom(),
            "相邻 collation 行不能重叠：first={first_row:?}, second={second_row:?}"
        );

        let virtual_detail = cx
            .debug_bounds("server-object-detail-VirtualItem-0-sessions")
            .expect("Virtual views 说明应渲染");
        let virtual_label = cx
            .debug_bounds("server-object-label-VirtualItem-0-sessions")
            .expect("Virtual views 名称应渲染");
        let virtual_row = cx
            .debug_bounds("server-object-row-VirtualItem-0-sessions")
            .expect("Virtual views 行应渲染");
        assert!(
            virtual_detail.origin.x >= virtual_label.right()
                && virtual_label.origin.x >= virtual_row.origin.x
                && virtual_detail.right() <= virtual_row.right()
                && virtual_detail.bottom() <= virtual_row.bottom(),
            "Virtual views 名称和说明必须水平分离并留在行内：row={virtual_row:?}, label={virtual_label:?}, detail={virtual_detail:?}"
        );
    }
}
