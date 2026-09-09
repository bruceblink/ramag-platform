use super::*;

/// Build the varied topic snapshot shared by the workspace interaction checks.
pub(super) fn topics() -> Vec<KafkaTopic> {
    let mut topics = vec![KafkaTopic {
        name: "ramag.integration.messages".into(),
        internal: false,
        partitions: (0..50)
            .map(|id| KafkaPartition {
                id,
                leader: Some(0),
                replicas: vec![0],
                isr: vec![0],
                low_watermark: Some(0),
                high_watermark: Some(1),
            })
            .collect(),
    }];
    for (name, partition_count) in [
        ("ramag.ui.empty", 1_i32),
        ("ramag.ui.short", 2_i32),
        ("ramag.ui.partition-heavy", 12_i32),
        (
            "ramag.ui.long-topic-name-for-responsive-layout-check",
            3_i32,
        ),
    ] {
        topics.push(KafkaTopic {
            name: name.into(),
            internal: false,
            partitions: (0..partition_count)
                .map(|id| KafkaPartition {
                    id,
                    leader: Some(0),
                    replicas: vec![0],
                    isr: vec![0],
                    low_watermark: Some(0),
                    high_watermark: Some(if name == "ramag.ui.empty" { 0 } else { 12 }),
                })
                .collect(),
        });
    }
    topics.extend((0..118).map(|index| KafkaTopic {
        name: format!("ramag.integration.topic-{index:03}"),
        internal: false,
        partitions: vec![KafkaPartition {
            id: 0,
            leader: Some(0),
            replicas: vec![0],
            isr: vec![0],
            low_watermark: Some(0),
            high_watermark: Some(1),
        }],
    }));
    topics
}
