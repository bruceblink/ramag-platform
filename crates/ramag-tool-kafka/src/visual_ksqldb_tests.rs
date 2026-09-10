use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use gpui::{Context, IntoElement, Render, TestAppContext, Window, px, size};
use ramag_domain::entities::{KafkaClusterConfig, KafkaKsqlDbQuery, KafkaKsqlDbQueryResult};
use ramag_domain::error::Result;
use ramag_domain::traits::KafkaKsqlDbDriver;

use super::*;

struct KafkaKsqlDbTestHost {
    view: gpui::Entity<KafkaView>,
}

impl Render for KafkaKsqlDbTestHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = gpui_component::Root::render_dialog_layer(window, cx);
        gpui::div()
            .relative()
            .size_full()
            .child(self.view.clone())
            .children(dialog_layer)
    }
}

struct VisualKsqlDbDriver {
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl KafkaKsqlDbDriver for VisualKsqlDbDriver {
    async fn execute_query(
        &self,
        _config: &KafkaClusterConfig,
        query: &KafkaKsqlDbQuery,
    ) -> Result<KafkaKsqlDbQueryResult> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(query.sql.clone());
        Ok(KafkaKsqlDbQueryResult {
            columns: vec!["EVENT".into(), "SEQUENCE".into(), "SOURCE".into()],
            rows: vec![vec!["created".into(), "1".into(), "visual".into()]],
            truncated: false,
            query_id: Some("visual-query-1".into()),
            final_message: Some("done".into()),
        })
    }
}

/// Exercises the read-only query lifecycle, bounded result scrolling, and the
/// compact layout without contacting a live ksqlDB service.
#[gpui::test]
fn kafka_ksqldb_query_is_read_only_bounded_and_responsive(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let mut cluster = KafkaClusterConfig::new("ksqlDB UI Kafka", vec!["127.0.0.1:19092".into()]);
    cluster.ksqldb.endpoint = Some("http://127.0.0.1:18088".into());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = Arc::new(
        KafkaService::new(
            Arc::new(FakeKafkaDriver),
            Arc::new(FakeStorage {
                cluster: cluster.clone(),
            }),
        )
        .with_ksqldb_driver(Arc::new(VisualKsqlDbDriver {
            calls: calls.clone(),
        })),
    );
    let mut kafka_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let kafka = cx.new(|cx| KafkaView::new(service, window, cx));
        kafka_entity = Some(kafka.clone());
        let host = cx.new(|_| KafkaKsqlDbTestHost { view: kafka });
        gpui_component::Root::new(host, window, cx)
    });
    let Some(kafka_entity) = kafka_entity else {
        return;
    };
    visual_cx.run_until_parked();

    visual_cx.update(|window, app| {
        kafka_entity.update(app, |view, cx| {
            view.clusters = vec![cluster.clone()];
            view.selected_cluster_id = Some(cluster.id.clone());
            view.loading_clusters = false;
            view.loading_runtime = false;
            view.section = KafkaSection::KsqlDb;
            view.set_form_from_config(&cluster, window, cx);
            view.ksqldb.query.update(cx, |input, cx| {
                input.set_value(
                    "SELECT * FROM RAMAG_INTEGRATION_STREAM EMIT CHANGES LIMIT 20;",
                    window,
                    cx,
                )
            });
            cx.notify();
        });
    });
    visual_cx.simulate_resize(size(px(1200.0), px(900.0)));
    visual_cx.run_until_parked();

    assert!(kafka_entity.read_with(visual_cx, |view, _| {
        view.read_only == KafkaReadOnlyState::ReadOnly
            && view.ksqldb.endpoint.read_with(visual_cx, |input, _| {
                input.value() == "http://127.0.0.1:18088"
            })
    }));
    for selector in [
        "kafka-ksqldb-query",
        "kafka-ksqldb-query-controls",
        "kafka-ksqldb-query-input",
        "kafka-ksqldb-query-run",
        "kafka-ksqldb-result-empty",
    ] {
        assert!(
            visual_cx.debug_bounds(selector).is_some(),
            "控件应参与布局: {selector}"
        );
        super::assert_within_width(visual_cx, selector, 1200.0);
    }

    super::click(visual_cx, "kafka-ksqldb-query-run");
    visual_cx.run_until_parked();
    assert_eq!(
        calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_slice(),
        &[String::from(
            "SELECT * FROM RAMAG_INTEGRATION_STREAM EMIT CHANGES LIMIT 20;",
        )]
    );
    assert!(kafka_entity.read_with(visual_cx, |view, _| {
        view.ksqldb
            .result
            .as_ref()
            .is_some_and(|result| !result.rows.is_empty())
    }));

    visual_cx.update(|_, app| {
        kafka_entity.update(app, |view, cx| {
            view.ksqldb.result = Some(KafkaKsqlDbQueryResult {
                columns: vec!["EVENT".into(), "SEQUENCE".into(), "SOURCE".into()],
                rows: (0..80)
                    .map(|index| vec![format!("event-{index}"), index.to_string(), "visual".into()])
                    .collect(),
                truncated: false,
                query_id: Some("visual-query-2".into()),
                final_message: Some("bounded".into()),
            });
            cx.notify();
        });
    });
    visual_cx.run_until_parked();
    let max_result_offset =
        kafka_entity.read_with(visual_cx, |view, _| view.ksqldb.scroll.max_offset());
    assert!(
        max_result_offset.y > px(0.0),
        "有足够结果行时应提供 ksqlDB 结果滚动范围: {max_result_offset:?}"
    );
    kafka_entity.update(visual_cx, |view, cx| {
        view.ksqldb
            .scroll
            .set_offset(gpui::point(px(0.0), -max_result_offset.y));
        cx.notify();
    });
    visual_cx.run_until_parked();
    assert!(kafka_entity.read_with(visual_cx, |view, _| {
        view.ksqldb.scroll.offset().y < px(0.0)
    }));

    kafka_entity.update(visual_cx, |view, cx| {
        view.ksqldb.result = None;
        view.ksqldb.loading = true;
        cx.notify();
    });
    visual_cx.run_until_parked();
    let call_count_before_disabled_click = calls
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len();
    assert!(
        visual_cx
            .debug_bounds("kafka-ksqldb-query-cancel")
            .is_some()
    );
    super::click(visual_cx, "kafka-ksqldb-query-run");
    visual_cx.run_until_parked();
    assert_eq!(
        calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len(),
        call_count_before_disabled_click,
        "执行中的 ksqlDB 查询不能重复提交"
    );
    super::click(visual_cx, "kafka-ksqldb-query-cancel");
    visual_cx.run_until_parked();
    assert!(kafka_entity.read_with(visual_cx, |view, _| {
        !view.ksqldb.loading && view.ksqldb.error.as_deref() == Some("ksqlDB 查询已取消")
    }));

    kafka_entity.update(visual_cx, |view, cx| {
        view.ksqldb.error = None;
        view.ksqldb.result = Some(KafkaKsqlDbQueryResult {
            columns: vec!["EVENT".into(), "SEQUENCE".into(), "SOURCE".into()],
            rows: vec![vec!["created".into(), "1".into(), "visual".into()]],
            truncated: false,
            query_id: Some("visual-query-3".into()),
            final_message: None,
        });
        cx.notify();
    });
    visual_cx.simulate_resize(size(px(360.0), px(900.0)));
    visual_cx.run_until_parked();
    for selector in [
        "kafka-ksqldb-query",
        "kafka-ksqldb-query-controls",
        "kafka-ksqldb-query-input",
        "kafka-ksqldb-query-run",
        "kafka-ksqldb-result",
    ] {
        super::assert_within_width(visual_cx, selector, 360.0);
    }
    let input = visual_cx.debug_bounds("kafka-ksqldb-query-input");
    let run = visual_cx.debug_bounds("kafka-ksqldb-query-run");
    assert!(
        input
            .zip(run)
            .is_some_and(|(input, run)| run.origin.y >= input.bottom()),
        "窄窗口应将 ksqlDB 执行按钮放到查询输入下方: input={input:?}, run={run:?}"
    );
}
