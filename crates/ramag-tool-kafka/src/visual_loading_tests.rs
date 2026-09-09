use std::sync::Arc;

use gpui::{Context, IntoElement, Render, TestAppContext, Window, px, size};
use ramag_app::KafkaService;
use ramag_domain::entities::{KafkaBroker, KafkaClusterConfig, KafkaClusterMetadata};

use super::*;

struct KafkaLoadingTestHost {
    view: gpui::Entity<KafkaView>,
}

impl Render for KafkaLoadingTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = gpui_component::Root::render_dialog_layer(window, cx);
        gpui::div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(dialog_layer)
    }
}

fn assert_present(cx: &mut gpui::VisualTestContext, selectors: &[&'static str]) {
    for selector in selectors {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "加载态控件应参与布局: {selector}"
        );
    }
}

#[gpui::test]
fn kafka_loading_tables_keep_stable_geometry(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut cluster = KafkaClusterConfig::new("Loading Kafka", vec!["127.0.0.1:19092".into()]);
    cluster.schema_registry.endpoint = Some("http://127.0.0.1:8081".into());
    cluster.connect.endpoint = Some("http://127.0.0.1:8083".into());
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
        let host = cx.new(|_| KafkaLoadingTestHost { view: kafka });
        gpui_component::Root::new(host, window, cx)
    });
    let Some(kafka_entity) = kafka_entity else {
        return;
    };
    visual_cx.simulate_resize(size(px(1200.0), px(780.0)));

    kafka_entity.update(visual_cx, |view, cx| {
        view.clusters = vec![cluster.clone()];
        view.selected_cluster_id = Some(cluster.id.clone());
        view.metadata = None;
        view.topics.clear();
        view.loading_clusters = false;
        view.loading_runtime = true;
        view.section = KafkaSection::Overview;
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert_present(
        visual_cx,
        &[
            "kafka-overview-scroll-viewport",
            "kafka-overview-scroll",
            "kafka-overview-v-scrollbar",
            "kafka-overview-loading-metrics",
            "kafka-overview-metrics-snapshot",
            "kafka-overview-loading-status",
            "kafka-overview-loading-broker",
            "kafka-overview-loading-topic",
            "kafka-overview-loading-cluster",
        ],
    );
    assert!(
        visual_cx.debug_bounds("kafka-overview-broker").is_none()
            && visual_cx.debug_bounds("kafka-overview-topic").is_none(),
        "概览加载期间不应先显示没有数据的正式内容"
    );
    for selector in [
        "kafka-overview-scroll-viewport",
        "kafka-overview-scroll",
        "kafka-overview-loading-metrics",
        "kafka-overview-metrics-snapshot",
        "kafka-overview-loading-sections",
    ] {
        super::assert_within_width(visual_cx, selector, 1200.0);
    }
    let Some(viewport) = visual_cx.debug_bounds("kafka-overview-scroll-viewport") else {
        return;
    };
    let Some(scrollbar) = visual_cx.debug_bounds("kafka-overview-v-scrollbar") else {
        return;
    };
    assert!(
        scrollbar.origin.x >= viewport.origin.x
            && scrollbar.right() <= viewport.right()
            && scrollbar.size.width == px(16.0),
        "概览加载态滚动条应保留在独立轨道中: viewport={viewport:?}, scrollbar={scrollbar:?}"
    );
    visual_cx.simulate_resize(size(px(360.0), px(900.0)));
    visual_cx.run_until_parked();
    for selector in [
        "kafka-overview-scroll-viewport",
        "kafka-overview-scroll",
        "kafka-overview-loading-metrics",
        "kafka-overview-metrics-snapshot",
        "kafka-overview-loading-sections",
    ] {
        super::assert_within_width(visual_cx, selector, 360.0);
    }
    visual_cx.simulate_resize(size(px(1200.0), px(780.0)));
    visual_cx.run_until_parked();

    kafka_entity.update(visual_cx, |view, cx| {
        view.metadata = Some(KafkaClusterMetadata {
            cluster_id: Some("loading-metrics-cluster".into()),
            controller_id: Some(0),
            brokers: vec![KafkaBroker {
                id: 0,
                host: "127.0.0.1".into(),
                port: 19092,
                rack: None,
                version: Some("4.0.0".into()),
                is_controller: true,
            }],
            kafka_version: Some("4.0.0".into()),
        });
        view.loading_runtime = false;
        view.metrics_loading = true;
        view.section = KafkaSection::Overview;
        cx.notify();
    });
    for (width, height) in [(360.0, 900.0), (1200.0, 780.0)] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        assert_present(
            visual_cx,
            &[
                "kafka-metrics-loading-summary",
                "kafka-metrics-topics",
                "kafka-metrics-partition-health",
                "kafka-metrics-consumer-groups",
                "kafka-broker-metrics-loading",
            ],
        );
        for selector in [
            "kafka-overview-metrics-snapshot",
            "kafka-metrics-loading-summary",
            "kafka-metrics-topics",
            "kafka-metrics-partition-health",
            "kafka-metrics-consumer-groups",
            "kafka-broker-metrics-loading",
        ] {
            super::assert_within_width(visual_cx, selector, width);
        }
    }
    visual_cx.simulate_resize(size(px(1200.0), px(780.0)));
    visual_cx.run_until_parked();

    kafka_entity.update(visual_cx, |view, cx| {
        view.section = KafkaSection::Topics;
        view.loading_runtime = true;
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert_present(
        visual_cx,
        &[
            "kafka-topic-list-viewport",
            "kafka-topic-table",
            "kafka-topic-list-content",
            "kafka-topic-v-scrollbar",
        ],
    );
    for selector in [
        "kafka-topic-list-viewport",
        "kafka-topic-table",
        "kafka-topic-list-content",
        "kafka-topic-v-scrollbar",
    ] {
        super::assert_within_width(visual_cx, selector, 1200.0);
    }

    kafka_entity.update(visual_cx, |view, cx| {
        view.section = KafkaSection::Messages;
        view.loading_runtime = false;
        view.loading_messages = true;
        view.message_page = None;
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert_present(
        visual_cx,
        &[
            "kafka-message-loading",
            "kafka-message-table",
            "kafka-message-h-scroll",
            "kafka-message-v-scrollbar",
            "kafka-message-h-scrollbar",
        ],
    );
    for selector in [
        "kafka-message-loading",
        "kafka-message-table",
        "kafka-message-v-scrollbar",
        "kafka-message-h-scrollbar",
    ] {
        super::assert_within_width(visual_cx, selector, 1200.0);
    }

    kafka_entity.update(visual_cx, |view, cx| {
        view.section = KafkaSection::ConsumerGroups;
        view.loading_messages = false;
        view.loading_consumer_groups = true;
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert_present(
        visual_cx,
        &[
            "kafka-consumer-group-list",
            "kafka-consumer-group-loading",
            "kafka-consumer-group-v-scrollbar",
        ],
    );

    kafka_entity.update(visual_cx, |view, cx| {
        view.section = KafkaSection::SchemaRegistry;
        view.loading_consumer_groups = false;
        view.loading_schema_subjects = true;
        view.schema_subjects_loaded = false;
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert_present(
        visual_cx,
        &[
            "kafka-schema-registry-list-panel",
            "kafka-schema-subject-table",
            "kafka-schema-subject-list-content",
            "kafka-schema-subject-v-scrollbar",
        ],
    );

    kafka_entity.update(visual_cx, |view, cx| {
        view.section = KafkaSection::Acls;
        view.loading_schema_subjects = false;
        view.connectors_loaded = false;
        view.loading_connectors = false;
        view.loading_acls = true;
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert_present(visual_cx, &["kafka-acl-list-panel", "kafka-acl-loading"]);

    kafka_entity.update(visual_cx, |view, cx| {
        view.section = KafkaSection::Connect;
        view.loading_acls = false;
        view.loading_connectors = true;
        view.connectors_loaded = false;
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert_present(
        visual_cx,
        &[
            "kafka-connect-list-panel",
            "kafka-connect-table",
            "kafka-connect-list-content",
            "kafka-connect-v-scrollbar",
        ],
    );

    kafka_entity.update(visual_cx, |view, cx| {
        view.section = KafkaSection::Config;
        view.loading_connectors = false;
        view.loading_configs = true;
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert_present(
        visual_cx,
        &[
            "kafka-remote-config",
            "kafka-config-list-header",
            "kafka-config-loading",
        ],
    );
}
