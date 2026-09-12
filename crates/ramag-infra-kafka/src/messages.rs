use super::*;
use chrono::{DateTime, Utc};
use ramag_domain::entities::{
    KafkaMessageHeader, KafkaMessagePage, KafkaMessageQuery, KafkaMessageRecord,
    KafkaMessageSearchField, KafkaMessageSearchMode, KafkaMessageSearchQuery,
};
use ramag_domain::entities::{
    KafkaMessageTailEvent, KafkaMessageTailRequest, KafkaMessageTailStart,
};
use ramag_domain::traits::{KafkaMessageTailSink, KafkaMessageTailSinkResult};
use rdkafka::message::{Headers as _, Message};
use rdkafka::topic_partition_list::{Offset, TopicPartitionList};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration as StdDuration, Instant};

const MAX_TAIL_RECONNECT_ATTEMPTS: u32 = 3;
const TAIL_RECONNECT_DELAY: StdDuration = StdDuration::from_millis(500);

#[derive(Default)]
struct TailDropStats {
    records: u64,
    bytes: u64,
}

impl TailDropStats {
    fn add(&mut self, bytes: u64) {
        self.records = self.records.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
    }

    fn event(&self) -> Option<KafkaMessageTailEvent> {
        (self.records > 0).then_some(KafkaMessageTailEvent::Dropped {
            records: self.records,
            bytes: self.bytes,
        })
    }

    fn clear(&mut self) {
        self.records = 0;
        self.bytes = 0;
    }
}

#[derive(Default)]
struct PartitionScan {
    records: Vec<KafkaMessageRecord>,
    scanned_records: usize,
    scanned_bytes: u64,
    truncated: bool,
}

struct MessageSearchMatcher {
    query: String,
    fields: Vec<KafkaMessageSearchField>,
    regex: Option<regex::Regex>,
}

impl MessageSearchMatcher {
    fn new(query: &KafkaMessageSearchQuery) -> Result<Self> {
        query.validate().map_err(DomainError::InvalidConfig)?;
        let regex = (query.mode == KafkaMessageSearchMode::Regex)
            .then(|| {
                regex::RegexBuilder::new(&query.query)
                    .case_insensitive(true)
                    .size_limit(1024 * 1024)
                    .dfa_size_limit(1024 * 1024)
                    .build()
            })
            .transpose()
            .map_err(|error| DomainError::InvalidConfig(format!("消息正则表达式无效：{error}")))?;
        Ok(Self {
            query: query.query.to_lowercase(),
            fields: query.fields.clone(),
            regex,
        })
    }
}

impl RdkafkaTransport {
    /// 在独立消费者上按 Partition 顺序扫描，返回结果和扫描预算统计。
    pub(super) fn scan_messages_blocking(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaMessageQuery,
        search: Option<&KafkaMessageSearchQuery>,
    ) -> Result<KafkaMessagePage> {
        let cancelled = AtomicBool::new(false);
        self.scan_messages_blocking_with_cancel(config, query, search, &cancelled)
    }

    /// 扫描有限消息范围并在分区切换、网络等待和消息轮询之间检查取消信号。
    pub(super) fn scan_messages_blocking_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaMessageQuery,
        search: Option<&KafkaMessageSearchQuery>,
        cancelled: &AtomicBool,
    ) -> Result<KafkaMessagePage> {
        Self::ensure_build_features(config)?;
        if cancelled.load(Ordering::Acquire) {
            return Err(message_read_cancelled());
        }
        let matcher = search.map(MessageSearchMatcher::new).transpose()?;
        let deadline = Instant::now() + StdDuration::from_secs(u64::from(query.max_scan_seconds));
        let mut page = KafkaMessagePage::empty();

        for &partition in &query.partitions {
            if cancelled.load(Ordering::Acquire) {
                return Err(message_read_cancelled());
            }
            if page.scanned_records >= query.max_records
                || page.scanned_bytes >= query.max_bytes
                || Instant::now() >= deadline
            {
                page.truncated = true;
                break;
            }
            let remaining_records = query.max_records - page.scanned_records;
            let remaining_bytes = query.max_bytes - page.scanned_bytes;
            let scanned = self.scan_partition_blocking(
                config,
                query,
                (
                    partition,
                    remaining_records,
                    remaining_bytes,
                    deadline,
                    matcher.as_ref(),
                ),
                cancelled,
            )?;
            page.scanned_records = page.scanned_records.saturating_add(scanned.scanned_records);
            page.scanned_bytes = page.scanned_bytes.saturating_add(scanned.scanned_bytes);
            page.records.extend(scanned.records);
            if scanned.truncated {
                page.truncated = true;
                break;
            }
        }
        if Instant::now() >= deadline {
            page.truncated = true;
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(message_read_cancelled());
        }
        page.validate().map_err(DomainError::InvalidConfig)?;
        Ok(page)
    }

    fn scan_partition_blocking(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaMessageQuery,
        scan: (i32, usize, u64, Instant, Option<&MessageSearchMatcher>),
        cancelled: &AtomicBool,
    ) -> Result<PartitionScan> {
        let (partition, max_records, max_bytes, deadline, search) = scan;
        if cancelled.load(Ordering::Acquire) {
            return Err(message_read_cancelled());
        }
        let consumer = self.create_consumer(config)?;
        let (low, high) = consumer
            .fetch_watermarks(&query.topic, partition, self.request_timeout)
            .map_err(|error| errors::map_kafka_error(error, "读取 Kafka Partition 水位"))?;
        let low = low.max(0);
        let high = high.max(low);
        let start = resolve_query_offset(
            &consumer,
            query,
            partition,
            (
                query.start_offset,
                query.start_time,
                low,
                high,
                self.request_timeout,
                "解析 Kafka 消息起始位置",
            ),
        )?
        .unwrap_or(low)
        .clamp(low, high);
        let end = resolve_query_offset(
            &consumer,
            query,
            partition,
            (
                query.end_offset,
                query.end_time,
                low,
                high,
                self.request_timeout,
                "解析 Kafka 消息结束位置",
            ),
        )?
        .unwrap_or(high)
        .clamp(low, high);
        if cancelled.load(Ordering::Acquire) {
            return Err(message_read_cancelled());
        }
        if start >= end {
            return Ok(PartitionScan::default());
        }

        let mut assignment = TopicPartitionList::new();
        assignment
            .add_partition_offset(&query.topic, partition, Offset::Offset(start))
            .map_err(|error| errors::map_kafka_error(error, "分配 Kafka Partition"))?;
        consumer
            .assign(&assignment)
            .map_err(|error| errors::map_kafka_error(error, "分配 Kafka Partition"))?;

        let mut scanned = PartitionScan::default();
        let mut empty_polls = 0u8;
        while Instant::now() < deadline {
            if cancelled.load(Ordering::Acquire) {
                return Err(message_read_cancelled());
            }
            if scanned.scanned_records >= max_records || scanned.scanned_bytes >= max_bytes {
                scanned.truncated = true;
                break;
            }
            let message = match consumer.poll(StdDuration::from_millis(250)) {
                None => {
                    empty_polls = empty_polls.saturating_add(1);
                    if empty_polls >= 8 {
                        break;
                    }
                    continue;
                }
                Some(result) => {
                    result.map_err(|error| errors::map_kafka_error(error, "读取 Kafka 消息"))?
                }
            };
            if cancelled.load(Ordering::Acquire) {
                return Err(message_read_cancelled());
            }
            empty_polls = 0;
            if message.offset() < start {
                continue;
            }
            if message.offset() >= end {
                break;
            }

            let record = record_from_message(&message);
            let record_bytes = record.retained_bytes();
            if scanned.scanned_bytes.saturating_add(record_bytes) > max_bytes {
                scanned.truncated = true;
                break;
            }
            scanned.scanned_records = scanned.scanned_records.saturating_add(1);
            scanned.scanned_bytes = scanned.scanned_bytes.saturating_add(record_bytes);
            if search.is_none_or(|query| message_matches(&record, query)) {
                scanned.records.push(record);
            }
        }
        if Instant::now() >= deadline {
            scanned.truncated = true;
        }
        Ok(scanned)
    }

    /// 通过独立消费者持续读取指定分区；事件通道满时丢弃新消息并报告统计，避免无界堆积。
    pub(super) fn tail_messages_blocking(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaMessageTailRequest,
        sink: KafkaMessageTailSink,
        cancelled: Arc<AtomicBool>,
    ) -> Result<()> {
        Self::ensure_build_features(config)?;
        let mut next_offsets = HashMap::with_capacity(request.partitions.len());
        let mut consumer = self.create_tail_consumer(config, request, &mut next_offsets)?;
        let mut reconnect_attempt = 0;
        let mut dropped = TailDropStats::default();
        if matches!(
            sink(KafkaMessageTailEvent::Connected { attempt: 0 }),
            KafkaMessageTailSinkResult::Closed
        ) {
            return Ok(());
        }

        while !cancelled.load(Ordering::Acquire) {
            match consumer.poll(StdDuration::from_millis(u64::from(
                request.poll_timeout_millis,
            ))) {
                None => {}
                Some(Ok(message)) => {
                    reconnect_attempt = 0;
                    let partition = message.partition();
                    let offset = message.offset();
                    if offset >= 0 {
                        next_offsets.insert(partition, offset.saturating_add(1));
                    }
                    let retained_bytes = message_retained_bytes(&message);
                    if retained_bytes > request.max_message_bytes as u64 {
                        dropped.add(retained_bytes);
                        continue;
                    }
                    let record = record_from_message(&message);
                    record.validate().map_err(DomainError::InvalidConfig)?;
                    if !emit_tail_message(&sink, &mut dropped, record) {
                        return Ok(());
                    }
                }
                Some(Err(error)) => {
                    let mapped = errors::map_kafka_error(error, "读取 Kafka 实时消息");
                    if !is_retryable_tail_error(&mapped) {
                        return Err(mapped);
                    }
                    let Some(reconnected) = self.reconnect_tail_consumer(
                        config,
                        request,
                        &mut next_offsets,
                        &sink,
                        &cancelled,
                        &mut reconnect_attempt,
                    )?
                    else {
                        return Ok(());
                    };
                    consumer = reconnected;
                }
            }
        }
        if let Some(event) = dropped.event() {
            match sink(event) {
                KafkaMessageTailSinkResult::Accepted
                | KafkaMessageTailSinkResult::Backpressured
                | KafkaMessageTailSinkResult::Closed => {}
            }
        }
        Ok(())
    }

    fn create_tail_consumer(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaMessageTailRequest,
        next_offsets: &mut HashMap<i32, i64>,
    ) -> Result<BaseConsumer> {
        let consumer = self.create_consumer(config)?;
        let mut assignment = TopicPartitionList::new();
        for &partition in &request.partitions {
            let offset = if let Some(offset) = next_offsets.get(&partition).copied() {
                offset
            } else {
                let offset =
                    tail_start_offset(&consumer, request, partition, self.request_timeout)?;
                next_offsets.insert(partition, offset);
                offset
            };
            assignment
                .add_partition_offset(&request.topic, partition, Offset::Offset(offset))
                .map_err(|error| errors::map_kafka_error(error, "分配 Kafka 实时消息 Partition"))?;
        }
        consumer
            .assign(&assignment)
            .map_err(|error| errors::map_kafka_error(error, "分配 Kafka 实时消息 Partition"))?;
        Ok(consumer)
    }

    fn reconnect_tail_consumer(
        &self,
        config: &KafkaClusterConfig,
        request: &KafkaMessageTailRequest,
        next_offsets: &mut HashMap<i32, i64>,
        sink: &KafkaMessageTailSink,
        cancelled: &Arc<AtomicBool>,
        attempt: &mut u32,
    ) -> Result<Option<BaseConsumer>> {
        while *attempt < MAX_TAIL_RECONNECT_ATTEMPTS {
            *attempt = (*attempt).saturating_add(1);
            if matches!(
                sink(KafkaMessageTailEvent::Reconnecting { attempt: *attempt }),
                KafkaMessageTailSinkResult::Closed
            ) {
                return Ok(None);
            }
            if wait_for_tail_reconnect(cancelled) {
                return Ok(None);
            }
            match self.create_tail_consumer(config, request, next_offsets) {
                Ok(consumer) => {
                    if matches!(
                        sink(KafkaMessageTailEvent::Connected { attempt: *attempt }),
                        KafkaMessageTailSinkResult::Closed
                    ) {
                        return Ok(None);
                    }
                    return Ok(Some(consumer));
                }
                Err(error) if is_retryable_tail_error(&error) => {}
                Err(error) => return Err(error),
            }
        }
        Err(DomainError::Kafka(
            ramag_domain::error::KafkaError::new(
                ramag_domain::error::KafkaErrorCategory::Network,
                "重连 Kafka 实时消息",
                "Kafka 实时消息流重连失败",
            )
            .retryable(true),
        ))
    }
}

fn tail_start_offset(
    consumer: &BaseConsumer,
    request: &KafkaMessageTailRequest,
    partition: i32,
    timeout: StdDuration,
) -> Result<i64> {
    match request.start {
        KafkaMessageTailStart::Offset(offset) => Ok(offset),
        KafkaMessageTailStart::Earliest | KafkaMessageTailStart::Latest => {
            let (low, high) = consumer
                .fetch_watermarks(&request.topic, partition, timeout)
                .map_err(|error| errors::map_kafka_error(error, "读取 Kafka 实时消息起始位置"))?;
            Ok(match request.start {
                KafkaMessageTailStart::Earliest => low.max(0),
                KafkaMessageTailStart::Latest => high.max(low).max(0),
                KafkaMessageTailStart::Offset(_) => unreachable!(),
            })
        }
    }
}

fn message_retained_bytes(message: &rdkafka::message::BorrowedMessage<'_>) -> u64 {
    let headers = message.headers().map_or(0usize, |headers| {
        headers.iter().fold(0usize, |total, header| {
            total
                .saturating_add(header.key.len())
                .saturating_add(header.value.map_or(0, <[u8]>::len))
        })
    });
    let total = message
        .topic()
        .len()
        .saturating_add(message.key().map_or(0, <[u8]>::len))
        .saturating_add(message.payload().map_or(0, <[u8]>::len))
        .saturating_add(headers);
    u64::try_from(total).unwrap_or(u64::MAX)
}

fn emit_tail_message(
    sink: &KafkaMessageTailSink,
    dropped: &mut TailDropStats,
    record: KafkaMessageRecord,
) -> bool {
    if let Some(event) = dropped.event() {
        match sink(event) {
            KafkaMessageTailSinkResult::Accepted => dropped.clear(),
            KafkaMessageTailSinkResult::Backpressured => {
                dropped.add(record.retained_bytes());
                return true;
            }
            KafkaMessageTailSinkResult::Closed => return false,
        }
    }
    let retained_bytes = record.retained_bytes();
    match sink(KafkaMessageTailEvent::Message(record)) {
        KafkaMessageTailSinkResult::Accepted => true,
        KafkaMessageTailSinkResult::Backpressured => {
            dropped.add(retained_bytes);
            true
        }
        KafkaMessageTailSinkResult::Closed => false,
    }
}

fn wait_for_tail_reconnect(cancelled: &AtomicBool) -> bool {
    let started = Instant::now();
    while started.elapsed() < TAIL_RECONNECT_DELAY {
        if cancelled.load(Ordering::Acquire) {
            return true;
        }
        std::thread::sleep(StdDuration::from_millis(50));
    }
    false
}

fn is_retryable_tail_error(error: &DomainError) -> bool {
    matches!(error, DomainError::Kafka(error) if error.retryable)
}

fn message_read_cancelled() -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Cancelled,
        "读取 Kafka 消息",
        "Kafka 消息读取已取消",
    ))
}

fn resolve_query_offset(
    consumer: &BaseConsumer,
    query: &KafkaMessageQuery,
    partition: i32,
    options: (
        Option<i64>,
        Option<DateTime<Utc>>,
        i64,
        i64,
        Duration,
        &'static str,
    ),
) -> Result<Option<i64>> {
    let (offset, timestamp, low, high, timeout, operation) = options;
    if let Some(offset) = offset {
        return Ok(Some(offset));
    }
    let Some(timestamp) = timestamp else {
        return Ok(None);
    };
    let mut timestamps = TopicPartitionList::new();
    timestamps
        .add_partition_offset(
            &query.topic,
            partition,
            Offset::Offset(timestamp.timestamp_millis().max(0)),
        )
        .map_err(|error| errors::map_kafka_error(error, operation))?;
    let offsets = consumer
        .offsets_for_times(timestamps, timeout)
        .map_err(|error| errors::map_kafka_error(error, operation))?;
    let Some(entry) = offsets.find_partition(&query.topic, partition) else {
        return Ok(Some(high));
    };
    entry
        .error()
        .map_err(|error| errors::map_kafka_error(error, operation))?;
    Ok(match entry.offset() {
        Offset::Offset(value) if value >= 0 => Some(value),
        Offset::Beginning => Some(low),
        Offset::End | Offset::Invalid => Some(high),
        Offset::Stored | Offset::OffsetTail(_) => Some(low),
        Offset::Offset(value) => Some(value.max(low)),
    })
}

fn record_from_message(message: &rdkafka::message::BorrowedMessage<'_>) -> KafkaMessageRecord {
    let headers = message
        .headers()
        .map(|headers| {
            headers
                .iter()
                .map(|header| KafkaMessageHeader {
                    key: header.key.to_owned(),
                    value: header.value.map(ToOwned::to_owned),
                })
                .collect()
        })
        .unwrap_or_default();
    KafkaMessageRecord {
        topic: message.topic().to_owned(),
        partition: message.partition(),
        offset: message.offset(),
        timestamp: message
            .timestamp()
            .to_millis()
            .and_then(DateTime::<Utc>::from_timestamp_millis),
        key: message.key().map(ToOwned::to_owned),
        value: message.payload().map(ToOwned::to_owned),
        headers,
    }
}

fn message_matches(record: &KafkaMessageRecord, matcher: &MessageSearchMatcher) -> bool {
    matcher.fields.iter().any(|field| match field {
        KafkaMessageSearchField::Key => record
            .key
            .as_deref()
            .is_some_and(|bytes| text_matches(bytes, matcher)),
        KafkaMessageSearchField::Value => record
            .value
            .as_deref()
            .is_some_and(|bytes| text_matches(bytes, matcher)),
        KafkaMessageSearchField::Headers => record.headers.iter().any(|header| {
            text_matches(header.key.as_bytes(), matcher)
                || header
                    .value
                    .as_deref()
                    .is_some_and(|bytes| text_matches(bytes, matcher))
        }),
    })
}

include!("messages/text_matches.rs");

#[cfg(test)]
mod tests {
    include!("messages/tests.rs");
}
