use super::*;

struct KafkaRetryTestHost {
    view: gpui::Entity<KafkaView>,
}

impl Render for KafkaRetryTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = gpui_component::Root::render_dialog_layer(window, cx);
        gpui::div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(dialog_layer)
    }
}

#[gpui::test]
fn kafka_cluster_load_failure_has_retry_path(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let cluster = KafkaClusterConfig::new("Retry Kafka", vec!["127.0.0.1:19092".into()]);
    let service = Arc::new(KafkaService::new(
        Arc::new(FakeKafkaDriver),
        Arc::new(FakeStorage {
            cluster: cluster.clone(),
        }),
    ));
    let mut kafka_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let kafka = cx.new(|cx| KafkaView::new(service, window, cx));
        kafka_entity = Some(kafka.clone());
        let host = cx.new(|_| KafkaRetryTestHost { view: kafka });
        gpui_component::Root::new(host, window, cx)
    });
    let Some(kafka_entity) = kafka_entity else {
        return;
    };
    visual_cx.simulate_resize(size(px(900.0), px(700.0)));
    visual_cx.run_until_parked();

    // 失败状态必须保留错误原因，并提供一条可恢复的本地配置重试路径。
    kafka_entity.update(visual_cx, |view, cx| {
        view.loading_clusters = false;
        view.clusters.clear();
        view.cluster_load_error = Some("模拟本地存储不可用".into());
        view.selected_cluster_id = None;
        view.section = KafkaSection::Overview;
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("kafka-retry-clusters").is_some());
    super::click(visual_cx, "kafka-retry-clusters");
    visual_cx.run_until_parked();
    assert!(
        kafka_entity.read_with(visual_cx, |view, _| {
            !view.loading_clusters && view.cluster_load_error.is_none() && view.clusters.len() == 1
        }),
        "重试后应恢复本地配置列表并清除错误状态"
    );
}

#[gpui::test]
fn kafka_welcome_action_stays_reachable_in_a_short_compact_window(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let cluster = KafkaClusterConfig::new("Compact Kafka", vec!["127.0.0.1:19092".into()]);
    let service = Arc::new(KafkaService::new(
        Arc::new(FakeKafkaDriver),
        Arc::new(FakeStorage {
            cluster: cluster.clone(),
        }),
    ));
    let mut kafka_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let kafka = cx.new(|cx| KafkaView::new(service, window, cx));
        kafka_entity = Some(kafka.clone());
        let host = cx.new(|_| KafkaRetryTestHost { view: kafka });
        gpui_component::Root::new(host, window, cx)
    });
    let Some(kafka_entity) = kafka_entity else {
        return;
    };

    visual_cx.simulate_resize(size(px(360.0), px(240.0)));
    kafka_entity.update(visual_cx, |view, cx| {
        view.clusters.clear();
        view.cluster_load_error = None;
        view.loading_clusters = false;
        view.selected_cluster_id = None;
        view.section = KafkaSection::Overview;
        cx.notify();
    });
    visual_cx.run_until_parked();

    let main = visual_cx.debug_bounds("kafka-main");
    let add = visual_cx.debug_bounds("kafka-welcome-add");
    assert!(main.is_some(), "Kafka 主区应渲染");
    assert!(add.is_some(), "紧凑欢迎页应保留新建入口");
    let Some(main) = main else {
        return;
    };
    let Some(add) = add else {
        return;
    };
    assert!(add.origin.x >= main.origin.x);
    assert!(add.right() <= main.right());
    assert!(add.origin.y >= main.origin.y);
    assert!(add.bottom() <= main.bottom());

    super::click(visual_cx, "kafka-welcome-add");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("kafka-config").is_some());

    let main = visual_cx.debug_bounds("kafka-main");
    let workspace = visual_cx.debug_bounds("kafka-workspace-tabs");
    let config_tab = visual_cx.debug_bounds("kafka-section-Config");
    let save = visual_cx.debug_bounds("kafka-save-profile");
    assert!(workspace.is_some(), "配置页应渲染工作区标签栏");
    assert!(config_tab.is_some(), "紧凑标签栏应保留配置标签");
    assert!(save.is_some(), "配置页应保留保存入口");
    let Some(main) = main else {
        return;
    };
    let Some(workspace) = workspace else {
        return;
    };
    let Some(config_tab) = config_tab else {
        return;
    };
    let Some(save) = save else {
        return;
    };
    assert!(config_tab.origin.x >= workspace.origin.x);
    assert!(config_tab.right() <= workspace.right());
    assert!(config_tab.origin.y >= workspace.origin.y);
    assert!(config_tab.bottom() <= workspace.bottom());
    assert!(save.origin.x >= main.origin.x);
    assert!(save.right() <= main.right());
    assert!(save.origin.y >= main.origin.y);
    assert!(save.bottom() <= main.bottom());

    visual_cx.simulate_resize(size(px(1024.0), px(720.0)));
    visual_cx.run_until_parked();
    let Some(main) = visual_cx.debug_bounds("kafka-main") else {
        return;
    };
    let Some(actions) = visual_cx.debug_bounds("kafka-config-actions") else {
        return;
    };
    let Some(save) = visual_cx.debug_bounds("kafka-save-profile") else {
        return;
    };
    assert!(actions.origin.x >= main.origin.x);
    assert!(actions.right() <= main.right());
    assert!(save.origin.x >= main.origin.x);
    assert!(save.right() <= main.right());
    assert!(save.origin.y >= main.origin.y);
    assert!(save.bottom() <= main.bottom());
}
