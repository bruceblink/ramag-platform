use super::*;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use ramag_domain::entities::{KafkaMessageProduceRequest, KafkaMessageProduceResult};
use ramag_domain::error::{DomainError, KafkaError, KafkaErrorCategory, Result};
use ramag_domain::traits::KafkaProducerDriver;

struct KafkaMessageTestHost {
    view: gpui::Entity<KafkaView>,
}

impl Render for KafkaMessageTestHost {
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
fn kafka_message_table_and_detail_fit_three_window_widths(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let cluster = KafkaClusterConfig::new("消息布局 Kafka", vec!["127.0.0.1:19092".into()]);
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
        let host = cx.new(|_| KafkaMessageTestHost { view: kafka });
        gpui_component::Root::new(host, window, cx)
    });
    let Some(kafka_entity) = kafka_entity else {
        return;
    };
    visual_cx.simulate_resize(size(px(1024.0), px(900.0)));
    visual_cx.run_until_parked();

    kafka_entity.update(visual_cx, |view, cx| {
        view.clusters = vec![cluster.clone()];
        view.selected_cluster_id = Some(cluster.id.clone());
        view.metadata = Some(KafkaClusterMetadata {
            cluster_id: Some("message-layout-cluster".into()),
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
        view.section = KafkaSection::Messages;
        view.loading_clusters = false;
        view.loading_runtime = false;
        view.loading_messages = false;
        let record = KafkaMessageRecord {
            topic: "ramag.integration.messages".into(),
            partition: 1,
            offset: 42,
            timestamp: None,
            key: Some(b"message-key".to_vec()),
            value: Some(vec![b'v'; 256]),
            headers: vec![ramag_domain::entities::KafkaMessageHeader {
                key: "trace-id".into(),
                value: Some(b"header-value".to_vec()),
            }],
        };
        view.message_page = Some(KafkaMessagePage {
            records: vec![record; 5_000],
            scanned_records: 5_000,
            scanned_bytes: 512 * 5_000,
            truncated: false,
        });
        view.message_page_index = 0;
        view.selected_message = None;
        cx.notify();
    });
    visual_cx.run_until_parked();
    let row_bounds = visual_cx.debug_bounds("kafka-message-row-0");
    assert!(row_bounds.is_some(), "消息行应参与布局");
    let Some(row_bounds) = row_bounds else {
        return;
    };
    visual_cx.simulate_click(row_bounds.center(), Modifiers::default());
    let selected_message = kafka_entity.read_with(visual_cx, |view, _| view.selected_message);
    assert_eq!(selected_message, Some(0));

    for (width, height) in [
        (360.0, 900.0),
        (800.0, 500.0),
        (1024.0, 900.0),
        (1440.0, 900.0),
    ] {
        visual_cx.simulate_resize(size(px(width), px(height)));
        visual_cx.run_until_parked();
        let messages = visual_cx.debug_bounds("kafka-messages");
        let table = visual_cx.debug_bounds("kafka-message-table");
        let detail = visual_cx.debug_bounds("kafka-message-detail");
        let detail_scroll = visual_cx.debug_bounds("kafka-message-detail-scroll");
        let horizontal_scrollbar = visual_cx.debug_bounds("kafka-message-h-scrollbar");
        let pagination = visual_cx.debug_bounds("kafka-message-pagination");
        let query_row = visual_cx.debug_bounds("kafka-message-query-row");
        let range_inputs = visual_cx.debug_bounds("kafka-range-inputs");
        let range_start = visual_cx.debug_bounds("kafka-range-start-field");
        let range_end = visual_cx.debug_bounds("kafka-range-end-field");
        let previous_page = visual_cx.debug_bounds("kafka-message-page-previous");
        let page_indicator = visual_cx.debug_bounds("kafka-message-page-indicator");
        let next_page = visual_cx.debug_bounds("kafka-message-page-next");
        assert!(
            messages.is_some()
                && table.is_some()
                && detail.is_some()
                && detail_scroll.is_some()
                && horizontal_scrollbar.is_some()
                && pagination.is_some()
                && query_row.is_some()
                && range_inputs.is_some()
                && range_start.is_some()
                && range_end.is_some()
                && previous_page.is_some()
                && page_indicator.is_some()
                && next_page.is_some(),
            "消息表、详情、滚动条和分页控件都应参与布局"
        );
        let (
            Some(messages),
            Some(table),
            Some(detail),
            Some(detail_scroll),
            Some(horizontal_scrollbar),
            Some(pagination),
            Some(query_row),
            Some(range_inputs),
            Some(range_start),
            Some(range_end),
            Some(previous_page),
            Some(page_indicator),
            Some(next_page),
        ) = (
            messages,
            table,
            detail,
            detail_scroll,
            horizontal_scrollbar,
            pagination,
            query_row,
            range_inputs,
            range_start,
            range_end,
            previous_page,
            page_indicator,
            next_page,
        )
        else {
            return;
        };

        assert!(
            messages.right() <= px(width),
            "消息页面不能横向溢出: {messages:?}"
        );
        assert!(
            messages.bottom() <= px(height),
            "消息页面不能纵向越出可用窗口: {messages:?} / {width}x{height}"
        );
        assert!(
            table.right() <= messages.right(),
            "消息表不能越出消息页面: {table:?}"
        );
        assert!(
            detail.right() <= messages.right(),
            "详情不能越出消息页面: {detail:?}"
        );
        assert!(
            detail_scroll.right() <= detail.right(),
            "详情滚动区不能越出详情面板: {detail_scroll:?} / {detail:?}"
        );
        assert!(
            horizontal_scrollbar.right() <= messages.right(),
            "横向滚动条不能越出消息页面: {horizontal_scrollbar:?}"
        );
        assert!(
            pagination.right() <= messages.right(),
            "分页状态栏不能越出消息页面: {pagination:?}"
        );
        assert!(
            table.size.height > px(0.0) && detail.size.height > px(0.0),
            "结果区及表格、详情应保留可用高度: table={table:?}, detail={detail:?}"
        );
        assert!(
            query_row.right() <= messages.right()
                && range_inputs.right() <= query_row.right()
                && range_start.right() <= range_inputs.right()
                && range_end.right() <= range_inputs.right(),
            "消息范围控件不能横向溢出父容器: query={query_row:?}, range={range_inputs:?}, start={range_start:?}, end={range_end:?}"
        );
        assert!(
            previous_page.right() <= pagination.right()
                && page_indicator.right() <= pagination.right()
                && next_page.right() <= pagination.right()
                && previous_page.origin.y >= pagination.origin.y
                && page_indicator.origin.y >= pagination.origin.y
                && next_page.origin.y >= pagination.origin.y
                && previous_page.bottom() <= pagination.bottom()
                && page_indicator.bottom() <= pagination.bottom()
                && next_page.bottom() <= pagination.bottom(),
            "分页子控件不能越出分页栏: pagination={pagination:?}, previous={previous_page:?}, indicator={page_indicator:?}, next={next_page:?}"
        );
        if width < 1280.0 {
            assert!(
                detail.origin.y >= table.origin.y + table.size.height,
                "窄窗口应将详情放到消息表下方: {table:?} / {detail:?}"
            );
        } else {
            assert!(
                detail.origin.x >= table.origin.x,
                "常规和宽窗口应保留表格与详情的横向工作区: {table:?} / {detail:?}"
            );
        }
        if width < 1080.0 {
            assert!(
                range_start.origin.y + range_start.size.height <= range_end.origin.y,
                "紧凑窗口应将起止范围字段上下排列: start={range_start:?}, end={range_end:?}"
            );
        } else {
            assert!(
                range_start.origin.y + range_start.size.height > range_end.origin.y,
                "宽窗口应保留起止范围字段的横向布局: start={range_start:?}, end={range_end:?}"
            );
        }

        if width < 900.0 {
            let max_page_offset =
                kafka_entity.read_with(visual_cx, |view, _| view.message_page_scroll.max_offset());
            assert!(
                max_page_offset.y > px(0.0),
                "窄窗口消息页内容超出视口时应提供纵向滚动范围: {max_page_offset:?}"
            );
            kafka_entity.update(visual_cx, |view, cx| {
                view.message_page_scroll
                    .set_offset(gpui::point(-max_page_offset.x, -max_page_offset.y));
                cx.notify();
            });
            visual_cx.run_until_parked();
            let scrolled_table = visual_cx.debug_bounds("kafka-message-table");
            assert!(
                scrolled_table.is_some_and(|bounds| bounds.origin.y < messages.bottom()),
                "滚动到页面底部后应能看到消息结果区: {scrolled_table:?} / {messages:?}"
            );
            kafka_entity.update(visual_cx, |view, cx| {
                view.message_page_scroll
                    .set_offset(gpui::point(px(0.0), px(0.0)));
                cx.notify();
            });
            visual_cx.run_until_parked();
        }
    }
}

struct VisualProducerDriver {
    calls: Arc<Mutex<Vec<KafkaMessageProduceRequest>>>,
    fail: Arc<AtomicBool>,
}

impl VisualProducerDriver {
    /// Records producer calls without turning a poisoned test mutex into an unrelated failure.
    fn record_call(&self, request: KafkaMessageProduceRequest) {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request);
    }
}

#[async_trait]
impl KafkaProducerDriver for VisualProducerDriver {
    async fn produce_message(
        &self,
        _config: &KafkaClusterConfig,
        request: &KafkaMessageProduceRequest,
    ) -> Result<KafkaMessageProduceResult> {
        self.record_call(request.clone());
        if self.fail.load(Ordering::Acquire) {
            return Err(DomainError::Kafka(KafkaError::new(
                KafkaErrorCategory::PermissionDenied,
                "写入 Kafka 消息",
                "测试生产器拒绝写入",
            )));
        }
        Ok(KafkaMessageProduceResult::new(
            request.topic.clone(),
            request.partition.unwrap_or(0),
            42,
            None,
        ))
    }
}

/// Covers the write guard, confirmation boundary, success feedback, and error
/// recovery behavior without connecting the UI test to a live Kafka service.
#[gpui::test]
fn kafka_message_production_requires_confirmation_and_preserves_failure_input(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_component::init);
    let cluster = KafkaClusterConfig::new("生产验证 Kafka", vec!["127.0.0.1:19092".into()]);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let fail = Arc::new(AtomicBool::new(false));
    let service = Arc::new(
        KafkaService::new(
            Arc::new(FakeKafkaDriver),
            Arc::new(FakeStorage {
                cluster: cluster.clone(),
            }),
        )
        .with_producer_driver(Arc::new(VisualProducerDriver {
            calls: calls.clone(),
            fail: fail.clone(),
        })),
    );
    let mut kafka_entity = None;
    let (_, visual_cx) = cx.add_window_view(|window, cx| {
        let kafka = cx.new(|cx| KafkaView::new(service, window, cx));
        kafka_entity = Some(kafka.clone());
        let host = cx.new(|_| KafkaMessageTestHost { view: kafka });
        gpui_component::Root::new(host, window, cx)
    });
    let Some(kafka_entity) = kafka_entity else {
        return;
    };

    visual_cx.simulate_resize(size(px(1200.0), px(900.0)));
    visual_cx.run_until_parked();
    visual_cx.update(|window, app| {
        kafka_entity.update(app, |view, cx| {
            view.clusters = vec![cluster.clone()];
            view.selected_cluster_id = Some(cluster.id.clone());
            view.section = KafkaSection::Messages;
            view.loading_clusters = false;
            view.loading_runtime = false;
            view.loading_messages = false;
            view.message_page = Some(KafkaMessagePage::empty());
            view.set_form_from_config(&cluster, window, cx);
            view.produce_topic_input
                .update(cx, |input, cx| input.set_value("events", window, cx));
            view.produce_partition_input
                .update(cx, |input, cx| input.set_value("2", window, cx));
            view.produce_key_input
                .update(cx, |input, cx| input.set_value("key", window, cx));
            view.produce_value_input
                .update(cx, |input, cx| input.set_value("payload", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    assert!(visual_cx.debug_bounds("kafka-message-producer").is_some());
    click(visual_cx, "kafka-produce-message");
    visual_cx.run_until_parked();
    assert!(
        calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty(),
        "只读模式下点击发送不能调用生产器"
    );
    assert!(
        visual_cx.debug_bounds("ramag-confirm-ok").is_none(),
        "只读模式下不能打开发送确认"
    );

    visual_cx.update(|_, app| {
        kafka_entity.update(app, |view, cx| {
            view.read_only = KafkaReadOnlyState::ReadWrite;
            cx.notify();
        });
    });
    visual_cx.run_until_parked();

    click(visual_cx, "kafka-produce-message");
    visual_cx.run_until_parked();
    assert!(
        visual_cx.debug_bounds("ramag-confirm-ok").is_some(),
        "管理模式发送前必须显示确认对话框"
    );
    assert!(
        calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty(),
        "确认前不能调用生产器"
    );
    click(visual_cx, "ramag-confirm-cancel");
    visual_cx.run_until_parked();
    assert!(visual_cx.debug_bounds("ramag-confirm-ok").is_none());

    click(visual_cx, "kafka-produce-message");
    visual_cx.run_until_parked();
    click(visual_cx, "ramag-confirm-ok");
    visual_cx.run_until_parked();
    let successful_calls = calls
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(successful_calls.len(), 1);
    assert_eq!(successful_calls[0].topic, "events");
    assert_eq!(successful_calls[0].partition, Some(2));
    drop(successful_calls);
    assert!(kafka_entity.read_with(visual_cx, |view, _| {
        view.notice
            .as_ref()
            .is_some_and(|(message, is_error)| !is_error && message.contains("Offset 42"))
    }));
    assert!(kafka_entity.read_with(visual_cx, |view, cx| {
        view.produce_value_input.read(cx).value().is_empty()
    }));

    fail.store(true, Ordering::Release);
    visual_cx.update(|window, app| {
        kafka_entity.update(app, |view, cx| {
            view.produce_value_input
                .update(cx, |input, cx| input.set_value("keep-on-error", window, cx));
            cx.notify();
        });
    });
    visual_cx.run_until_parked();
    click(visual_cx, "kafka-produce-message");
    visual_cx.run_until_parked();
    click(visual_cx, "ramag-confirm-ok");
    visual_cx.run_until_parked();
    assert_eq!(
        calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len(),
        2
    );
    assert!(kafka_entity.read_with(visual_cx, |view, _| {
        view.notice
            .as_ref()
            .is_some_and(|(message, is_error)| *is_error && message.contains("发送 Kafka 消息失败"))
    }));
    assert!(kafka_entity.read_with(visual_cx, |view, cx| {
        view.produce_value_input.read(cx).value() == "keep-on-error"
    }));
}
