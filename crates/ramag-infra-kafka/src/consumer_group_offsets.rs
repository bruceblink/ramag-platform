use ramag_domain::entities::KafkaConsumerGroupOffsetResetRequest;
use ramag_domain::error::{DomainError, KafkaError, KafkaErrorCategory, Result};
use rdkafka::admin::AdminClient;
use rdkafka::client::DefaultClientContext;
use rdkafka::error::IsError;
use rdkafka::topic_partition_list::{Offset, TopicPartitionList};
use std::ffi::CString;
use std::time::{Duration, Instant};

use super::{NativeAdminOptions, NativeEvent, NativeQueue, timeout_millis};

const ADMIN_POLL_INTERVAL: Duration = Duration::from_millis(100);
const OPERATION: &str = "重置 Kafka 消费者组 Offset";

/// 使用 librdkafka Admin API 原子提交本次请求中列出的 Topic/Partition 目标。
pub(super) fn alter_consumer_group_offsets_native(
    admin: &AdminClient<DefaultClientContext>,
    request: &KafkaConsumerGroupOffsetResetRequest,
    request_timeout: Duration,
) -> Result<()> {
    let group_id = CString::new(request.group_id.as_str())
        .map_err(|_| DomainError::InvalidConfig("消费者组 ID 包含 NUL 字符".into()))?;
    let mut partitions = TopicPartitionList::with_capacity(request.offsets.len());
    for target in &request.offsets {
        partitions
            .add_partition_offset(
                &target.topic,
                target.partition,
                Offset::Offset(target.offset),
            )
            .map_err(|error| super::super::errors::map_kafka_error(error, OPERATION))?;
    }

    let native_request = unsafe {
        rdkafka::bindings::rd_kafka_AlterConsumerGroupOffsets_new(
            group_id.as_ptr(),
            partitions.ptr(),
        )
    };
    if native_request.is_null() {
        return Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Protocol,
            OPERATION,
            "Kafka 无法创建消费者组 Offset 重置请求",
        )));
    }
    let native_request = NativeAlterConsumerGroupOffsets(native_request);
    let client = admin.inner().native_ptr();
    let options = native_admin_options(client, request_timeout)?;
    let queue = unsafe { rdkafka::bindings::rd_kafka_queue_get_main(client) };
    if queue.is_null() {
        return Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Protocol,
            OPERATION,
            "Kafka 无法获取消费者组 Offset 管理事件队列",
        )));
    }
    let queue = NativeQueue(queue);
    let mut requests = [native_request.0];
    unsafe {
        rdkafka::bindings::rd_kafka_AlterConsumerGroupOffsets(
            client,
            requests.as_mut_ptr(),
            requests.len(),
            options.0,
            queue.0,
        );
    }

    poll_alter_result(&queue, request_timeout)
}

fn native_admin_options(
    client: *mut rdkafka::types::RDKafka,
    request_timeout: Duration,
) -> Result<NativeAdminOptions> {
    let options = unsafe {
        rdkafka::bindings::rd_kafka_AdminOptions_new(
            client,
            rdkafka::types::RDKafkaAdminOp::RD_KAFKA_ADMIN_OP_ALTERCONSUMERGROUPOFFSETS,
        )
    };
    if options.is_null() {
        return Err(DomainError::Kafka(KafkaError::new(
            KafkaErrorCategory::Protocol,
            OPERATION,
            "Kafka 无法创建消费者组 Offset 管理选项",
        )));
    }
    let options = NativeAdminOptions(options);
    let mut error_text = vec![0; 512];
    let result = unsafe {
        rdkafka::bindings::rd_kafka_AdminOptions_set_request_timeout(
            options.0,
            timeout_millis(request_timeout),
            error_text.as_mut_ptr(),
            error_text.len(),
        )
    };
    if result.is_error() {
        return Err(super::super::errors::map_kafka_error(
            rdkafka::error::KafkaError::AdminOp(result.into()),
            OPERATION,
        ));
    }
    Ok(options)
}

fn poll_alter_result(queue: &NativeQueue, request_timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + request_timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(timeout_error());
        }
        let event = unsafe {
            rdkafka::bindings::rd_kafka_queue_poll(
                queue.0,
                timeout_millis(remaining.min(ADMIN_POLL_INTERVAL)),
            )
        };
        if event.is_null() {
            return Err(timeout_error());
        }
        let event = NativeEvent(event);
        let event_type = unsafe { rdkafka::bindings::rd_kafka_event_type(event.0) };
        if event_type != rdkafka::bindings::RD_KAFKA_EVENT_ALTERCONSUMERGROUPOFFSETS_RESULT {
            continue;
        }
        let event_error = unsafe { rdkafka::bindings::rd_kafka_event_error(event.0) };
        if event_error.is_error() {
            return Err(super::super::errors::map_kafka_error(
                rdkafka::error::KafkaError::AdminOp(event_error.into()),
                OPERATION,
            ));
        }
        let result =
            unsafe { rdkafka::bindings::rd_kafka_event_AlterConsumerGroupOffsets_result(event.0) };
        if result.is_null() {
            return Err(protocol_error("Kafka 返回了空的消费者组 Offset 重置结果"));
        }
        let mut group_count = 0usize;
        let groups = unsafe {
            rdkafka::bindings::rd_kafka_AlterConsumerGroupOffsets_result_groups(
                result,
                &mut group_count,
            )
        };
        if groups.is_null() || group_count != 1 {
            return Err(protocol_error("Kafka 未返回唯一的消费者组 Offset 重置结果"));
        }
        let group = unsafe { *groups };
        if group.is_null() {
            return Err(protocol_error("Kafka 返回了空的消费者组 Offset 重置结果"));
        }
        let group_error = unsafe { rdkafka::bindings::rd_kafka_group_result_error(group) };
        if !group_error.is_null() {
            let code = unsafe { rdkafka::bindings::rd_kafka_error_code(group_error) };
            if code.is_error() {
                return Err(super::super::errors::map_kafka_error(
                    rdkafka::error::KafkaError::AdminOp(code.into()),
                    OPERATION,
                ));
            }
        }
        let partitions = unsafe { rdkafka::bindings::rd_kafka_group_result_partitions(group) };
        if partitions.is_null() {
            return Err(protocol_error("Kafka 返回了空的 Partition Offset 重置结果"));
        }
        let partitions = unsafe { &*partitions };
        if partitions.cnt < 0 || (partitions.cnt > 0 && partitions.elems.is_null()) {
            return Err(protocol_error(
                "Kafka 返回了无效的 Partition Offset 重置结果",
            ));
        }
        for index in 0..partitions.cnt as usize {
            let partition = unsafe { &*partitions.elems.add(index) };
            if partition.err.is_error() {
                return Err(super::super::errors::map_kafka_error(
                    rdkafka::error::KafkaError::AdminOp(partition.err.into()),
                    OPERATION,
                ));
            }
        }
        return Ok(());
    }
}

fn timeout_error() -> DomainError {
    DomainError::Kafka(
        KafkaError::new(
            KafkaErrorCategory::Timeout,
            OPERATION,
            "Kafka 消费者组 Offset 重置超时",
        )
        .retryable(true),
    )
}

fn protocol_error(message: &'static str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Protocol,
        OPERATION,
        message,
    ))
}

struct NativeAlterConsumerGroupOffsets(
    *mut rdkafka::bindings::rd_kafka_AlterConsumerGroupOffsets_t,
);

impl Drop for NativeAlterConsumerGroupOffsets {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { rdkafka::bindings::rd_kafka_AlterConsumerGroupOffsets_destroy(self.0) };
        }
    }
}
