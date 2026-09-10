//! ksqlDB /query HTTP 适配器；只执行受限 SELECT，不执行外部写操作。

use std::{
    io::Read,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use async_trait::async_trait;
use ramag_domain::{
    entities::{
        KafkaClusterConfig, KafkaKsqlDbQuery, KafkaKsqlDbQueryResult,
        MAX_KAFKA_KSQLDB_RESULT_COLUMNS, MAX_KAFKA_KSQLDB_RESULT_ROWS,
    },
    error::{DomainError, KafkaError, KafkaErrorCategory, Result},
    traits::KafkaKsqlDbDriver,
};
use reqwest::{
    Url,
    blocking::{Client, Response},
};
use serde_json::{Deserializer, Value};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const KSQL_CONTENT_TYPE: &str = "application/vnd.ksql.v1+json";

#[derive(Clone)]
pub struct KsqlDbHttpDriver {
    client: Client,
}

impl KsqlDbHttpDriver {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .user_agent(concat!("Ramag/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| DomainError::Other("创建 ksqlDB HTTP 客户端失败".into()))?;
        Ok(Self { client })
    }

    fn execute_query_blocking(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaKsqlDbQuery,
        cancelled: &AtomicBool,
    ) -> Result<KafkaKsqlDbQueryResult> {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(cancelled_error());
        }
        let Some(endpoint) = config.ksqldb.endpoint.as_deref() else {
            return Err(invalid_config_error("未配置 ksqlDB 端点"));
        };
        let url = query_url(endpoint)?;
        let body = query_request_body(query);
        let response = self
            .client
            .post(url)
            .header("Accept", KSQL_CONTENT_TYPE)
            .header("Content-Type", KSQL_CONTENT_TYPE)
            .json(&body)
            .send()
            .map_err(request_error)?;
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(cancelled_error());
        }
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        let body = read_response_body(response, cancelled)?;
        parse_query_result(&body)
    }
}

/// Builds the bounded read request. Starting at the earliest offset lets a
/// finite UI query inspect existing stream data instead of waiting only for
/// records produced after the request was opened.
fn query_request_body(query: &KafkaKsqlDbQuery) -> Value {
    serde_json::json!({
        "ksql": query.sql,
        "streamsProperties": {
            "ksql.streams.auto.offset.reset": "earliest"
        }
    })
}

#[async_trait]
impl KafkaKsqlDbDriver for KsqlDbHttpDriver {
    async fn execute_query(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaKsqlDbQuery,
    ) -> Result<KafkaKsqlDbQueryResult> {
        self.execute_query_with_cancel(config, query, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn execute_query_with_cancel(
        &self,
        config: &KafkaClusterConfig,
        query: &KafkaKsqlDbQuery,
        cancelled: Arc<AtomicBool>,
    ) -> Result<KafkaKsqlDbQueryResult> {
        config.validate().map_err(DomainError::InvalidConfig)?;
        query.validate().map_err(DomainError::InvalidConfig)?;
        let driver = self.clone();
        let config = config.clone();
        let query = query.clone();
        smol::unblock(move || driver.execute_query_blocking(&config, &query, &cancelled)).await
    }
}

fn query_url(endpoint: &str) -> Result<Url> {
    let mut url = Url::parse(endpoint).map_err(|_| invalid_config_error("ksqlDB 端点地址无效"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_config_error(
            "ksqlDB 端点只支持不带认证参数的 http/https 地址",
        ));
    }
    let path = url.path().trim_end_matches('/');
    let path = if path.is_empty() {
        "/query".into()
    } else {
        format!("{path}/query")
    };
    url.set_path(&path);
    Ok(url)
}

fn parse_query_result(body: &[u8]) -> Result<KafkaKsqlDbQueryResult> {
    let trimmed = body.iter().copied().skip_while(u8::is_ascii_whitespace);
    let values = if trimmed.clone().next() == Some(b'[') {
        serde_json::from_slice::<Vec<Value>>(body)
            .map_err(|_| protocol_error("ksqlDB 响应 JSON 格式无效"))?
    } else {
        Deserializer::from_slice(body)
            .into_iter::<Value>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| protocol_error("ksqlDB 流式响应 JSON 格式无效"))?
    };

    let mut result = KafkaKsqlDbQueryResult {
        columns: Vec::new(),
        rows: Vec::new(),
        truncated: false,
        query_id: None,
        final_message: None,
    };
    for value in values {
        parse_query_chunk(value, &mut result)?;
        if result.rows.len() >= MAX_KAFKA_KSQLDB_RESULT_ROWS {
            result.truncated = true;
            break;
        }
    }
    result
        .validate()
        .map_err(DomainError::InvalidConfig)
        .map(|()| result)
}

fn parse_query_chunk(value: Value, result: &mut KafkaKsqlDbQueryResult) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| protocol_error("ksqlDB 响应块格式无效"))?;
    if object.contains_key("error_code") || object.contains_key("error") {
        return Err(protocol_error("ksqlDB 返回查询错误"));
    }
    if let Some(header) = object.get("header") {
        let header = header
            .as_object()
            .ok_or_else(|| protocol_error("ksqlDB 响应 Header 格式无效"))?;
        if let Some(query_id) = header.get("queryId").and_then(Value::as_str) {
            result.query_id = Some(query_id.to_owned());
        }
        if let Some(schema) = header.get("schema").and_then(Value::as_str) {
            result.columns = parse_schema_columns(schema)?;
        }
    }
    if let Some(row) = object.get("row")
        && !row.is_null()
    {
        let columns = row
            .get("columns")
            .and_then(Value::as_array)
            .ok_or_else(|| protocol_error("ksqlDB 响应行格式无效"))?;
        if columns.len() > MAX_KAFKA_KSQLDB_RESULT_COLUMNS {
            return Err(protocol_error("ksqlDB 响应字段数量超过限制"));
        }
        result.rows.push(
            columns
                .iter()
                .map(value_to_text)
                .collect::<Result<Vec<_>>>()?,
        );
    }
    if let Some(message) = object.get("finalMessage").and_then(Value::as_str) {
        result.final_message = Some(compact_message(message));
    }
    Ok(())
}

fn parse_schema_columns(schema: &str) -> Result<Vec<String>> {
    let columns = schema
        .split(',')
        .map(str::trim)
        .filter(|column| !column.is_empty())
        .map(|column| {
            if let Some(rest) = column.strip_prefix('\x60') {
                let end = rest
                    .find('\x60')
                    .ok_or_else(|| protocol_error("ksqlDB Schema 字段名称格式无效"))?;
                Ok(rest[..end].to_owned())
            } else {
                column
                    .split_whitespace()
                    .next()
                    .filter(|name| !name.is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| protocol_error("ksqlDB Schema 字段名称为空"))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    if columns.len() > MAX_KAFKA_KSQLDB_RESULT_COLUMNS {
        return Err(protocol_error("ksqlDB Schema 字段数量超过限制"));
    }
    Ok(columns)
}

fn value_to_text(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok("null".into()),
        Value::String(value) => Ok(value.clone()),
        value => serde_json::to_string(value)
            .map_err(|_| protocol_error("ksqlDB 响应单元格无法转换为文本")),
    }
}

fn compact_message(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn read_response_body(mut response: Response, cancelled: &AtomicBool) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(protocol_error("ksqlDB 响应超过 4 MiB 限制"));
    }
    let mut body = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(cancelled_error());
        }
        let read = response
            .read(&mut buffer)
            .map_err(|_| network_error("读取 ksqlDB 响应失败"))?;
        if read == 0 {
            break;
        }
        if body.len().saturating_add(read) > MAX_RESPONSE_BYTES {
            return Err(protocol_error("ksqlDB 响应超过 4 MiB 限制"));
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Ok(body)
}

fn status_error(status: reqwest::StatusCode) -> DomainError {
    let category = if matches!(
        status,
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        KafkaErrorCategory::PermissionDenied
    } else if status == reqwest::StatusCode::NOT_FOUND {
        KafkaErrorCategory::NotFound
    } else {
        KafkaErrorCategory::Protocol
    };
    DomainError::Kafka(
        KafkaError::new(
            category,
            "执行 ksqlDB 只读查询",
            format!("ksqlDB 返回 HTTP {}", status.as_u16()),
        )
        .retryable(matches!(category, KafkaErrorCategory::Protocol)),
    )
}

fn request_error(error: reqwest::Error) -> DomainError {
    let (category, message) = if error.is_timeout() {
        (KafkaErrorCategory::Timeout, "ksqlDB 请求超时")
    } else if error.is_connect() {
        (KafkaErrorCategory::Network, "无法连接 ksqlDB")
    } else {
        (KafkaErrorCategory::Network, "ksqlDB 请求失败")
    };
    DomainError::Kafka(KafkaError::new(category, "执行 ksqlDB 只读查询", message).retryable(true))
}

fn invalid_config_error(message: &str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::InvalidConfig,
        "执行 ksqlDB 只读查询",
        message,
    ))
}

fn protocol_error(message: &str) -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Protocol,
        "执行 ksqlDB 只读查询",
        message,
    ))
}

fn network_error(message: &str) -> DomainError {
    DomainError::Kafka(
        KafkaError::new(KafkaErrorCategory::Network, "执行 ksqlDB 只读查询", message)
            .retryable(true),
    )
}

fn cancelled_error() -> DomainError {
    DomainError::Kafka(KafkaError::new(
        KafkaErrorCategory::Cancelled,
        "执行 ksqlDB 只读查询",
        "ksqlDB 查询任务已取消",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_query_to_a_bounded_endpoint() -> Result<()> {
        assert_eq!(
            query_url("http://127.0.0.1:8088/")?.as_str(),
            "http://127.0.0.1:8088/query"
        );
        assert_eq!(
            query_url("https://ksql.example/api")?.as_str(),
            "https://ksql.example/api/query"
        );
        Ok(())
    }

    #[test]
    fn rejects_endpoint_credentials_and_query_parameters() {
        assert!(query_url("http://user:pass@localhost:8088").is_err());
        assert!(query_url("http://localhost:8088?token=secret").is_err());
        assert!(query_url("ftp://localhost:8088").is_err());
    }

    #[test]
    fn bounded_query_starts_at_the_earliest_stream_offset() {
        let body = query_request_body(&KafkaKsqlDbQuery::new(
            "SELECT * FROM stream EMIT CHANGES LIMIT 20;",
        ));
        assert_eq!(
            body["streamsProperties"]["ksql.streams.auto.offset.reset"],
            "earliest"
        );
    }

    #[test]
    fn parses_streaming_header_and_rows_into_bounded_text() -> Result<()> {
        let body = br#"{"header":{"queryId":"query-1","schema":"ID INT, NAME STRING"}}
{"row":{"columns":[1,"alice"]}}
{"row":{"columns":[2,null]}}
{"finalMessage":"done\n"}"#;
        let result = parse_query_result(body)?;
        assert_eq!(result.query_id.as_deref(), Some("query-1"));
        assert_eq!(result.columns, vec!["ID", "NAME"]);
        assert_eq!(result.rows, vec![vec!["1", "alice"], vec!["2", "null"]]);
        assert_eq!(result.final_message.as_deref(), Some("done"));
        Ok(())
    }
}
