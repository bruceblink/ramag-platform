use super::*;

/// 渲染实际 Tooltip 容器，检查完整长错误按窗口宽度换行且高度有界。
#[gpui_kit::test]
fn long_error_tooltip_stays_inside_supported_and_defensive_windows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let message = gpui_kit::SharedString::from("完整错误原因 ".repeat(150));
        let tooltip = gpui_kit::component::tooltip::Tooltip::element(move |window, _| {
            super::super::render::error_tooltip_content(message.clone(), window)
        })
        .build(window, cx);
        gpui_kit::component::Root::new(tooltip, window, cx)
    });
    for text_size in [
        ramag_ui::InterfaceTextSize::Standard,
        ramag_ui::InterfaceTextSize::Large,
    ] {
        cx.update(|_, app| {
            ramag_ui::set_system_settings(
                ramag_ui::SystemSettings {
                    text_size,
                    ..Default::default()
                },
                app,
            )
        });
        for (width, height) in [
            (360.0, 640.0),
            (1008.0, 672.0),
            (1024.0, 768.0),
            (1440.0, 900.0),
        ] {
            cx.simulate_resize(size(px(width), px(height)));
            cx.run_until_parked();
            let bounds = cx
                .debug_bounds("connection-list-error-tooltip-content")
                .expect("完整错误提示应渲染");
            assert!(bounds.size.width <= px(width - 72.0));
            assert!(bounds.size.height <= px(height - 72.0));
            assert!(bounds.size.height > px(40.0), "长错误应换行而非单行溢出");
            assert!(bounds.right() <= px(width) && bounds.bottom() <= px(height));
        }
    }
}

#[gpui_kit::test]
fn connection_list_loading_and_failures_use_pulse_status_notices(cx: &mut TestAppContext) {
    cx.update(gpui_kit::component::init);
    let test_storage = Arc::new(NoopStorage {
        fail_load: AtomicBool::new(true),
    });
    let storage: Arc<dyn Storage> = test_storage.clone();
    let service = Arc::new(ConnectionService::new(HashMap::new(), storage.clone()));
    let redis_service = Arc::new(RedisService::new(
        Arc::new(ramag_infra_redis::RedisDriver::new()),
        storage.clone(),
    ));
    let mongo_service = Arc::new(MongoService::new(
        Arc::new(ramag_infra_mongodb::MongoDriver::new()),
        storage,
    ));
    let mut panel_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let entity = cx.new(|panel_cx| {
            let mut panel =
                ConnectionListPanel::new(service, redis_service, mongo_service, window, panel_cx);
            panel.refresh_generation = panel.refresh_generation.wrapping_add(1);
            panel.loading = true;
            panel.loaded_revision = panel.service.revision();
            panel
        });
        panel_entity = Some(entity.clone());
        ConnectionListTestHost::new(entity, cx)
    });
    let panel = panel_entity.expect("连接列表面板应创建");

    cx.simulate_resize(size(px(360.0), px(640.0)));
    cx.run_until_parked();
    let loading = cx
        .debug_bounds("connection-list-loading-notice")
        .expect("加载态应使用 Pulse 状态通知");
    assert!(loading.size.width > px(0.0) && loading.size.height > px(0.0));

    panel.update(cx, |panel, cx| {
        panel.loading = false;
        panel.load_error = Some("读取连接列表失败：存储暂时不可用".into());
        panel.connections = Arc::new(Vec::new());
        cx.notify();
    });
    cx.run_until_parked();
    let error = cx
        .debug_bounds("connection-list-error-notice")
        .expect("空列表失败态应使用 Pulse 状态通知");
    let retry = cx
        .debug_bounds("conn-list-retry")
        .expect("空列表失败态应保留重试入口");
    assert!(error.right() <= px(360.0));
    assert!(retry.right() <= px(360.0) && retry.bottom() <= px(640.0));

    panel.update(cx, |panel, cx| {
        panel.connections = Arc::new(vec![ConnectionConfig::new_mysql(
            "stale connection",
            "db.internal.example",
            3306,
            "ramag",
        )]);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("connection-list-error-notice").is_some(),
        "保留旧列表时也必须显示加载失败原因"
    );
    assert!(
        cx.debug_bounds("connection-list-panel").is_some(),
        "加载失败不应隐藏仍可用的旧列表"
    );

    for mode in [ramag_ui::Mode::Dark, ramag_ui::Mode::Light] {
        for text_size in [
            ramag_ui::InterfaceTextSize::Standard,
            ramag_ui::InterfaceTextSize::Large,
        ] {
            cx.update(|_, app| {
                ramag_ui::set_system_settings(
                    ramag_ui::SystemSettings {
                        text_size,
                        ..Default::default()
                    },
                    app,
                );
                ramag_ui::apply_theme(mode, app);
            });
            for (width, height) in [(360.0, 640.0), (1024.0, 768.0), (1440.0, 900.0)] {
                cx.simulate_resize(size(px(width), px(height)));
                cx.update(|window, app| {
                    panel.update(app, |panel, cx| {
                        panel.search.update(cx, |search, cx| {
                            search.set_value("missing", window, cx);
                        });
                        cx.notify();
                    });
                });
                cx.run_until_parked();
                let error = cx
                    .debug_bounds("connection-list-error-notice")
                    .expect("搜索无匹配时仍须保留读取失败原因");
                let retry = cx
                    .debug_bounds("conn-list-retry")
                    .expect("搜索无匹配时仍须提供重试入口");
                let no_matches = cx.debug_bounds("connection-list-no-matches").unwrap();
                let search = cx.debug_bounds("connection-search-field").unwrap();
                assert!(error.origin.y >= search.bottom());
                assert!(no_matches.origin.y >= error.bottom());
                assert!(no_matches.size.height > px(0.0));
                assert!(error.right() <= px(width) && error.bottom() <= px(height));
                assert!(retry.right() <= px(width) && retry.bottom() <= px(height));
                panel.read_with(cx, |panel, _| {
                    assert_eq!(panel.query, "missing");
                    assert_eq!(panel.connections.len(), 1);
                    assert!(panel.filtered_indices().is_empty());
                });
            }
        }
    }
    cx.update(|window, app| {
        panel.update(app, |panel, cx| {
            panel
                .search
                .update(cx, |search, cx| search.set_value("stale", window, cx));
        });
    });
    cx.run_until_parked();
    let error = cx.debug_bounds("connection-list-error-notice").unwrap();
    let heading = cx.debug_bounds("connection-list-column-header").unwrap();
    assert!(heading.origin.y >= error.bottom());
    let id = panel.read_with(cx, |panel, _| panel.connections[0].id.clone());
    let edit_selector: &'static str =
        Box::leak(format!("connection-row-edit-{id}").into_boxed_str());
    let edit = cx.debug_bounds(edit_selector).unwrap();
    assert!(edit.origin.y >= heading.bottom());
    panel.read_with(cx, |panel, _| assert_eq!(panel.filtered_indices().len(), 1));

    cx.simulate_resize(size(px(360.0), px(640.0)));
    cx.update(|_, app| {
        ramag_ui::set_system_settings(
            ramag_ui::SystemSettings {
                text_size: ramag_ui::InterfaceTextSize::Large,
                ..Default::default()
            },
            app,
        );
        ramag_ui::apply_theme(ramag_ui::Mode::Light, app);
    });
    panel.update(cx, |panel, cx| {
        panel.load_error = Some("详细错误信息 ".repeat(100));
        cx.notify();
    });
    cx.run_until_parked();
    let error = cx.debug_bounds("connection-list-error-notice").unwrap();
    let retry = cx.debug_bounds("conn-list-retry").unwrap();
    let heading = cx.debug_bounds("connection-list-column-header").unwrap();
    assert!(retry.right() <= px(360.0) && retry.bottom() <= px(640.0));
    assert!(heading.origin.y >= error.bottom());

    for _ in 0..2 {
        let generation = panel.read_with(cx, |panel, _| panel.refresh_generation);
        let retry = cx.debug_bounds("conn-list-retry").unwrap();
        cx.simulate_click(retry.center(), Modifiers::default());
        cx.run_until_parked();
        panel.read_with(cx, |panel, _| {
            assert_eq!(panel.refresh_generation, generation.wrapping_add(1));
            assert_eq!(panel.query, "stale");
            assert_eq!(panel.connections.len(), 1);
            assert!(
                panel
                    .load_error
                    .as_deref()
                    .is_some_and(|error| error.contains("测试读取失败"))
            );
            assert!(!panel.loading);
        });
        assert!(cx.debug_bounds("conn-list-retry").is_some());
    }
    test_storage.fail_load.store(false, Ordering::Release);
    let generation = panel.read_with(cx, |panel, _| panel.refresh_generation);
    let retry = cx.debug_bounds("conn-list-retry").unwrap();
    cx.simulate_click(retry.center(), Modifiers::default());
    cx.run_until_parked();
    panel.read_with(cx, |panel, _| {
        assert_eq!(panel.refresh_generation, generation.wrapping_add(1));
        assert_eq!(panel.query, "stale");
        assert!(panel.load_error.is_none());
        assert!(!panel.loading);
    });
    assert!(cx.debug_bounds("connection-list-error-notice").is_none());
}
