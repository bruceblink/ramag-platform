//! ksqlDB 的只读查询配置、请求和有界文本结果。

use std::fmt;

use serde::{Deserialize, Serialize};

use super::kafka_validation::{validate_optional_single_line, validate_protocol_text};
use super::{
    MAX_KAFKA_KSQLDB_CELL_BYTES, MAX_KAFKA_KSQLDB_ENDPOINT_BYTES, MAX_KAFKA_KSQLDB_QUERY_BYTES,
    MAX_KAFKA_KSQLDB_QUERY_ID_BYTES, MAX_KAFKA_KSQLDB_RESULT_COLUMNS, MAX_KAFKA_KSQLDB_RESULT_ROWS,
};

/// 可选的 ksqlDB Server 地址；它是独立 HTTP 服务，不是 Kafka Broker 地址。
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaKsqlDbConfig {
    #[serde(default)]
    pub endpoint: Option<String>,
}

impl fmt::Debug for KafkaKsqlDbConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KafkaKsqlDbConfig")
            .field("endpoint", &self.endpoint.as_ref().map(|_| "[CONFIGURED]"))
            .finish()
    }
}

impl KafkaKsqlDbConfig {
    pub fn validate(&self) -> Result<(), String> {
        validate_optional_single_line(
            "ksqlDB 端点",
            self.endpoint.as_deref(),
            MAX_KAFKA_KSQLDB_ENDPOINT_BYTES,
        )?;
        if let Some(endpoint) = self.endpoint.as_deref()
            && !endpoint.starts_with("http://")
            && !endpoint.starts_with("https://")
        {
            return Err("ksqlDB 端点必须使用 http:// 或 https://".into());
        }
        Ok(())
    }
}

/// 只接受 SELECT 查询；写入、DDL、查询管理和删除语句在领域层直接拒绝。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaKsqlDbQuery {
    pub sql: String,
}

impl KafkaKsqlDbQuery {
    pub fn new(sql: impl Into<String>) -> Self {
        Self { sql: sql.into() }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_protocol_text("ksqlDB 查询", &self.sql, MAX_KAFKA_KSQLDB_QUERY_BYTES)?;
        if self.sql.trim().is_empty() {
            return Err("ksqlDB 查询不能为空".into());
        }
        if self
            .sql
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        {
            return Err("ksqlDB 查询不能包含控制字符".into());
        }

        let tokens = self
            .sql
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .filter(|token| !token.is_empty())
            .map(|token| token.to_ascii_uppercase())
            .collect::<Vec<_>>();
        if tokens.first().map(String::as_str) != Some("SELECT") {
            return Err("ksqlDB 查询只能以 SELECT 开始".into());
        }
        const FORBIDDEN_TOKENS: &[&str] =
            &["CREATE", "DELETE", "DROP", "PAUSE", "RESUME", "TERMINATE"];
        if let Some(token) = tokens
            .iter()
            .find(|token| FORBIDDEN_TOKENS.contains(&token.as_str()))
        {
            return Err(format!("ksqlDB 只读查询禁止关键字：{token}"));
        }
        if tokens
            .windows(2)
            .any(|window| window[0] == "INSERT" && window[1] == "INTO")
        {
            return Err("ksqlDB 只读查询禁止 INSERT INTO".into());
        }
        Ok(())
    }
}

/// ksqlDB `/query` 返回的有限文本结果；单元格已由 Infra 层转为安全展示文本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KafkaKsqlDbQueryResult {
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub query_id: Option<String>,
    #[serde(default)]
    pub final_message: Option<String>,
}

impl KafkaKsqlDbQueryResult {
    pub fn validate(&self) -> Result<(), String> {
        if self.columns.len() > MAX_KAFKA_KSQLDB_RESULT_COLUMNS {
            return Err(format!(
                "ksqlDB 返回字段超过 {MAX_KAFKA_KSQLDB_RESULT_COLUMNS} 个上限"
            ));
        }
        if self.rows.len() > MAX_KAFKA_KSQLDB_RESULT_ROWS {
            return Err(format!(
                "ksqlDB 返回行数超过 {MAX_KAFKA_KSQLDB_RESULT_ROWS} 个上限"
            ));
        }
        for row in &self.rows {
            if !self.columns.is_empty() && row.len() != self.columns.len() {
                return Err("ksqlDB 返回行的字段数量与 Schema 不一致".into());
            }
            for cell in row {
                validate_protocol_text("ksqlDB 返回单元格", cell, MAX_KAFKA_KSQLDB_CELL_BYTES)?;
            }
        }
        if let Some(query_id) = self.query_id.as_deref() {
            validate_optional_single_line(
                "ksqlDB Query ID",
                Some(query_id),
                MAX_KAFKA_KSQLDB_QUERY_ID_BYTES,
            )?;
        }
        if let Some(final_message) = self.final_message.as_deref() {
            validate_optional_single_line(
                "ksqlDB 完成消息",
                Some(final_message),
                MAX_KAFKA_KSQLDB_CELL_BYTES,
            )?;
        }
        Ok(())
    }
}
