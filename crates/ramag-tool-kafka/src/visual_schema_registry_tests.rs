use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ramag_domain::entities::KafkaSchemaRegistryVersion;
use ramag_domain::error::Result;
use ramag_domain::traits::KafkaSchemaRegistryDriver;

use super::*;
use crate::KAFKA_SCHEMA_SUBJECT_SCROLLBAR_WIDTH;
use ramag_domain::entities::KafkaSchemaRegistrySubject;

struct KafkaSchemaRegistryTestHost {
    view: gpui::Entity<KafkaView>,
}

struct SchemaVersionDriver {
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl KafkaSchemaRegistryDriver for SchemaVersionDriver {
    async fn list_versions(&self, _config: &KafkaClusterConfig, subject: &str) -> Result<Vec<i32>> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(format!("versions:{subject}"));
        Ok(vec![1, 3])
    }

    async fn get_version(
        &self,
        _config: &KafkaClusterConfig,
        subject: &str,
        version: i32,
    ) -> Result<KafkaSchemaRegistryVersion> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(format!("version:{subject}:{version}"));
        Ok(KafkaSchemaRegistryVersion {
            subject: subject.into(),
            version,
            id: i64::from(version) + 10,
            schema_type: Some("JSON".into()),
            schema: format!(r#"{{"type":"object","version":{version}}}"#),
        })
    }
}

impl Render for KafkaSchemaRegistryTestHost {
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
fn schema_registry_subject_list_keeps_content_clear_of_vertical_scrollbar(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut cluster =
        KafkaClusterConfig::new("Schema Registry 布局 Kafka", vec!["127.0.0.1:19092".into()]);
    cluster.schema_registry.endpoint = Some("http://127.0.0.1:8081".into());
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
        let host = cx.new(|_| KafkaSchemaRegistryTestHost { view: kafka });
        gpui_component::Root::new(host, window, cx)
    });
    visual_cx.run_until_parked();
    let Some(kafka_entity) = kafka_entity else {
        return;
    };

    kafka_entity.update(visual_cx, |view, cx| {
        view.clusters = vec![cluster.clone()];
        view.selected_cluster_id = Some(cluster.id.clone());
        view.schema_subjects = (0..80)
            .map(|index| KafkaSchemaRegistrySubject {
                name: format!("orders-value-{index:02}"),
            })
            .collect();
        view.schema_subjects_loaded = true;
        view.loading_clusters = false;
        view.loading_runtime = false;
        view.section = KafkaSection::SchemaRegistry;
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(1200.0), px(780.0)));
    visual_cx.run_until_parked();

    let Some(table) = visual_cx.debug_bounds("kafka-schema-subject-table") else {
        return;
    };
    let Some(content) = visual_cx.debug_bounds("kafka-schema-subject-list-content") else {
        return;
    };
    let Some(scrollbar) = visual_cx.debug_bounds("kafka-schema-subject-v-scrollbar") else {
        return;
    };
    assert!(
        content.origin.x >= table.origin.x
            && content.right() <= scrollbar.origin.x
            && scrollbar.right() <= table.right()
            && scrollbar.size.width == px(KAFKA_SCHEMA_SUBJECT_SCROLLBAR_WIDTH),
        "Schema Registry 列表内容不能被垂直滚动条覆盖: table={table:?}, content={content:?}, scrollbar={scrollbar:?}"
    );
    assert!(
        visual_cx
            .debug_bounds("kafka-schema-subject-row-0")
            .is_some(),
        "Schema Registry Subject 列表应渲染首行"
    );
}

#[gpui::test]
fn schema_registry_subject_selection_loads_versions_and_details_responsively(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_component::init);
    let mut cluster =
        KafkaClusterConfig::new("Schema Registry 版本 Kafka", vec!["127.0.0.1:19092".into()]);
    cluster.schema_registry.endpoint = Some("http://127.0.0.1:18081".into());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = Arc::new(
        KafkaService::new(
            Arc::new(FakeKafkaDriver),
            Arc::new(FakeStorage {
                cluster: cluster.clone(),
            }),
        )
        .with_schema_registry_driver(Arc::new(SchemaVersionDriver {
            calls: calls.clone(),
        })),
    );
    let mut kafka_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let kafka = cx.new(|cx| KafkaView::new(service, window, cx));
        kafka_entity = Some(kafka.clone());
        let host = cx.new(|_| KafkaSchemaRegistryTestHost { view: kafka });
        gpui_component::Root::new(host, window, cx)
    });
    let Some(kafka_entity) = kafka_entity else {
        return;
    };

    kafka_entity.update(visual_cx, |view, cx| {
        view.clusters = vec![cluster.clone()];
        view.selected_cluster_id = Some(cluster.id.clone());
        view.schema_subjects = vec![KafkaSchemaRegistrySubject {
            name: "orders-value".into(),
        }];
        view.schema_subjects_loaded = true;
        view.loading_clusters = false;
        view.loading_runtime = false;
        view.section = KafkaSection::SchemaRegistry;
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(1200.0), px(800.0)));
    visual_cx.run_until_parked();

    click(visual_cx, "kafka-schema-subject-row-0");
    visual_cx.run_until_parked();
    let selected = kafka_entity.read_with(visual_cx, |view, _| {
        (
            view.schema_selected_subject.clone(),
            view.schema_versions.clone(),
            view.schema_selected_version,
            view.schema_version_detail
                .as_ref()
                .map(|detail| detail.version),
        )
    });
    assert_eq!(selected.0.as_deref(), Some("orders-value"));
    assert_eq!(selected.1, vec![1, 3]);
    assert_eq!(selected.2, Some(3));
    assert_eq!(selected.3, Some(3));
    assert_eq!(
        calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_slice(),
        &["versions:orders-value", "version:orders-value:3"]
    );

    click(visual_cx, "kafka-schema-version-1");
    visual_cx.run_until_parked();
    assert_eq!(
        kafka_entity.read_with(visual_cx, |view, _| {
            view.schema_version_detail
                .as_ref()
                .map(|detail| detail.version)
        }),
        Some(1)
    );

    for width in [360.0, 1200.0] {
        visual_cx.simulate_resize(size(px(width), px(800.0)));
        visual_cx.run_until_parked();
        for selector in [
            "kafka-schema-registry-content",
            "kafka-schema-registry-list-panel",
            "kafka-schema-registry-detail",
            "kafka-schema-version-workspace",
            "kafka-schema-version-content",
            "kafka-schema-version-schema-scroll",
        ] {
            super::assert_within_width(visual_cx, selector, width);
        }
    }
}
