use super::*;
use crate::KAFKA_SCHEMA_SUBJECT_SCROLLBAR_WIDTH;
use ramag_domain::entities::KafkaSchemaRegistrySubject;

struct KafkaSchemaRegistryTestHost {
    view: gpui::Entity<KafkaView>,
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
