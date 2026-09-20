use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    ApiAssertion, ApiCollection, ApiCollectionId, ApiEnvironment, ApiProtocol, ApiRequestId,
    ApiRequestRecord, ApiResponseSnapshot, ApiResponseStatus, ApiVariableExtraction,
    ApiVariableSource, MAX_API_ASSERTIONS, MAX_API_ERROR_BYTES, MAX_API_HISTORY_BODY_BYTES,
    MAX_API_PARAMETER_VALUE_BYTES, MAX_API_REQUEST_NAME_BYTES, MAX_API_VARIABLE_NAME_BYTES,
    validate_text,
};

pub const MAX_API_HISTORY: usize = 200;
pub const MAX_API_HISTORY_LIST_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiAssertionResult {
    pub passed: bool,
    pub message: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiExtractedVariable {
    pub name: String,
    pub value: String,
    pub sensitive: bool,
}

impl std::fmt::Debug for ApiExtractedVariable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApiExtractedVariable")
            .field("name", &self.name)
            .field(
                "value",
                if self.sensitive {
                    &"[REDACTED]"
                } else {
                    &self.value
                },
            )
            .field("sensitive", &self.sensitive)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiExecutionResult {
    pub snapshot: ApiResponseSnapshot,
    pub assertions: Vec<ApiAssertionResult>,
    pub passed: bool,
    #[serde(default)]
    pub extracted_variables: Vec<ApiExtractedVariable>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiExecutionOutcome {
    pub result: Option<ApiExecutionResult>,
    pub error: Option<String>,
    #[serde(default)]
    pub cancelled: bool,
    pub history: ApiHistoryRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiCollectionRunResult {
    pub collection_id: ApiCollectionId,
    pub collection_name: String,
    pub outcomes: Vec<ApiExecutionOutcome>,
    pub passed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub stopped: bool,
}

impl ApiCollectionRunResult {
    pub fn empty(collection: &ApiCollection) -> Self {
        Self {
            collection_id: collection.id.clone(),
            collection_name: collection.name.clone(),
            outcomes: Vec::new(),
            passed: 0,
            failed: 0,
            cancelled: 0,
            stopped: false,
        }
    }

    pub fn push(&mut self, outcome: ApiExecutionOutcome) {
        if outcome.cancelled {
            self.cancelled += 1;
        } else if outcome.result.as_ref().is_some_and(|result| result.passed) {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        self.outcomes.push(outcome);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiHistoryRecord {
    pub id: Uuid,
    pub request_id: Option<ApiRequestId>,
    pub request_name: String,
    pub protocol: ApiProtocol,
    pub status: Option<ApiResponseStatus>,
    pub elapsed_millis: u64,
    pub size_bytes: u64,
    pub passed: bool,
    pub assertion_count: usize,
    pub assertion_failed: usize,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub body_preview: String,
    pub created_at: DateTime<Utc>,
}

impl ApiEnvironment {
    /// 返回执行驱动需要的变量副本；调用方不得把结果写入日志或历史。
    pub fn execution_variables(&self) -> BTreeMap<String, String> {
        self.variables.clone()
    }

    /// 原子写入响应变量；校验失败时保留旧环境，避免后续请求看到半套变量。
    pub fn apply_extracted_variables(
        &mut self,
        extracted: &[ApiExtractedVariable],
    ) -> Result<(), String> {
        let mut candidate = self.clone();
        for variable in extracted {
            candidate
                .variables
                .insert(variable.name.clone(), variable.value.clone());
            if variable.sensitive
                && !candidate
                    .sensitive_variable_refs
                    .iter()
                    .any(|name| name == &variable.name)
            {
                candidate
                    .sensitive_variable_refs
                    .push(variable.name.clone());
            }
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

impl ApiHistoryRecord {
    pub fn from_success(
        record: &ApiRequestRecord,
        result: &ApiExecutionResult,
        environment: &ApiEnvironment,
    ) -> Self {
        let failed = result.assertions.iter().filter(|item| !item.passed).count();
        let body = String::from_utf8_lossy(&result.snapshot.body);
        let body = redact_sensitive_text(&body, environment);
        Self {
            id: Uuid::new_v4(),
            request_id: Some(record.id.clone()),
            request_name: record.name.clone(),
            protocol: record.protocol,
            status: Some(result.snapshot.status.clone()),
            elapsed_millis: result.snapshot.elapsed_millis,
            size_bytes: result.snapshot.size_bytes,
            passed: result.passed,
            assertion_count: result.assertions.len(),
            assertion_failed: failed,
            error: None,
            body_preview: bound_text(&body, MAX_API_HISTORY_BODY_BYTES),
            created_at: Utc::now(),
        }
    }

    pub fn from_error(
        record: &ApiRequestRecord,
        error: &str,
        environment: &ApiEnvironment,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            request_id: Some(record.id.clone()),
            request_name: record.name.clone(),
            protocol: record.protocol,
            status: None,
            elapsed_millis: 0,
            size_bytes: 0,
            passed: false,
            assertion_count: record.assertions.len(),
            assertion_failed: record.assertions.len(),
            error: Some(bound_text(
                &redact_sensitive_text(error, environment),
                MAX_API_ERROR_BYTES,
            )),
            body_preview: String::new(),
            created_at: Utc::now(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.request_name.trim().is_empty()
            || self.request_name.len() > MAX_API_REQUEST_NAME_BYTES
        {
            return Err("API 历史请求名称无效".into());
        }
        if self.assertion_count > MAX_API_ASSERTIONS || self.assertion_failed > self.assertion_count
        {
            return Err("API 历史断言统计无效".into());
        }
        validate_text(
            "API 历史正文摘要",
            &self.body_preview,
            MAX_API_HISTORY_BODY_BYTES,
            false,
        )?;
        if let Some(error) = &self.error {
            validate_text("API 历史错误摘要", error, MAX_API_ERROR_BYTES, false)?;
        }
        Ok(())
    }
}

/// 按 {{name}} 规则展开模板；缺失变量和展开后超限都会被拒绝。
pub fn resolve_template(
    template: &str,
    variables: &BTreeMap<String, String>,
    label: &str,
    max_bytes: usize,
) -> Result<String, String> {
    let mut output = String::with_capacity(template.len());
    let mut cursor = 0usize;
    while let Some(relative_start) = template[cursor..].find("{{") {
        let start = cursor + relative_start;
        output.push_str(&template[cursor..start]);
        let value_start = start + 2;
        let relative_end = template[value_start..]
            .find("}}")
            .ok_or_else(|| format!("{label}变量占位符未闭合"))?;
        let end = value_start + relative_end;
        let name = template[value_start..end].trim();
        if name.is_empty() || name.len() > MAX_API_VARIABLE_NAME_BYTES {
            return Err(format!("{label}变量名无效"));
        }
        let value = variables
            .get(name)
            .ok_or_else(|| format!("缺少 API 环境变量：{name}"))?;
        output.push_str(value);
        cursor = end + 2;
    }
    output.push_str(&template[cursor..]);
    if output.len() > max_bytes {
        return Err(format!("{label}展开后超过 {max_bytes} bytes 上限"));
    }
    Ok(output)
}

/// 从有界响应中提取变量；所有规则先完成再写入 Environment，保证失败不产生部分结果。
pub fn extract_response_variables(
    rules: &[ApiVariableExtraction],
    snapshot: &ApiResponseSnapshot,
) -> Result<Vec<ApiExtractedVariable>, String> {
    let mut names = HashSet::with_capacity(rules.len());
    let mut extracted = Vec::with_capacity(rules.len());
    for rule in rules {
        if !names.insert(rule.name.clone()) {
            return Err(format!("响应变量名称重复：{}", rule.name));
        }
        let value = match &rule.source {
            ApiVariableSource::JsonPath { path } => {
                ensure_response_body_complete(snapshot)?;
                let body = std::str::from_utf8(&snapshot.body)
                    .map_err(|_| "响应正文不是 UTF-8，无法提取 JSON 变量".to_string())?;
                let json = serde_json::from_str::<serde_json::Value>(body)
                    .map_err(|_| "响应正文不是有效 JSON，无法提取 JSON 变量".to_string())?;
                let value = json_path_value(&json, path)
                    .ok_or_else(|| format!("响应 JSON Path 未找到值：{path}"))?;
                json_value_text(value)?
            }
            ApiVariableSource::Header { name } => snapshot
                .headers
                .iter()
                .find(|parameter| parameter.name.eq_ignore_ascii_case(name))
                .map(|parameter| parameter.value.clone())
                .ok_or_else(|| format!("响应 Header 不存在：{name}"))?,
            ApiVariableSource::Metadata { name } => snapshot
                .metadata
                .iter()
                .find(|parameter| parameter.name.eq_ignore_ascii_case(name))
                .map(|parameter| parameter.value.clone())
                .ok_or_else(|| format!("响应 Metadata 不存在：{name}"))?,
            ApiVariableSource::Body => {
                ensure_response_body_complete(snapshot)?;
                std::str::from_utf8(&snapshot.body)
                    .map(str::to_owned)
                    .map_err(|_| "响应正文不是 UTF-8，无法提取正文变量".to_string())?
            }
        };
        if value.len() > MAX_API_PARAMETER_VALUE_BYTES {
            return Err(format!(
                "响应变量 {} 超过 {MAX_API_PARAMETER_VALUE_BYTES} bytes 上限",
                rule.name
            ));
        }
        extracted.push(ApiExtractedVariable {
            name: rule.name.clone(),
            value,
            sensitive: rule.sensitive,
        });
    }
    Ok(extracted)
}

fn ensure_response_body_complete(snapshot: &ApiResponseSnapshot) -> Result<(), String> {
    if snapshot.truncated {
        Err("响应正文已截断，不能提取响应变量".into())
    } else {
        Ok(())
    }
}

fn json_value_text(value: &serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::String(text) => Ok(text.clone()),
        _ => serde_json::to_string(value).map_err(|_| "响应 JSON 值无法转换为变量".into()),
    }
}

/// 对一份响应执行所有声明式断言；断言失败返回结果而不是传输错误。
pub fn evaluate_assertions(
    assertions: &[ApiAssertion],
    snapshot: &ApiResponseSnapshot,
) -> Result<Vec<ApiAssertionResult>, String> {
    let mut results = Vec::with_capacity(assertions.len());
    for assertion in assertions {
        let (passed, message) = evaluate_assertion(assertion, snapshot)?;
        results.push(ApiAssertionResult { passed, message });
    }
    Ok(results)
}

fn evaluate_assertion(
    assertion: &ApiAssertion,
    snapshot: &ApiResponseSnapshot,
) -> Result<(bool, String), String> {
    let result = match assertion {
        ApiAssertion::HttpStatus { expected } => {
            let actual = match &snapshot.status {
                ApiResponseStatus::Http { code } => Some(*code),
                _ => None,
            };
            (
                actual == Some(*expected),
                format!("HTTP 状态应为 {expected}"),
            )
        }
        ApiAssertion::HeaderEquals { name, expected } => {
            let actual = snapshot
                .headers
                .iter()
                .find(|item| item.name.eq_ignore_ascii_case(name))
                .map(|item| item.value.as_str());
            (
                actual == Some(expected.as_str()),
                format!("Header {name} 应匹配"),
            )
        }
        ApiAssertion::MetadataEquals { name, expected } => {
            let actual = snapshot
                .metadata
                .iter()
                .find(|item| item.name.eq_ignore_ascii_case(name))
                .map(|item| item.value.as_str());
            (
                actual == Some(expected.as_str()),
                format!("Metadata {name} 应匹配"),
            )
        }
        ApiAssertion::BodyContains { expected } => {
            let body = String::from_utf8_lossy(&snapshot.body);
            (body.contains(expected), "响应正文应包含指定文本".into())
        }
        ApiAssertion::JsonPathEquals { path, expected } => {
            let body = String::from_utf8_lossy(&snapshot.body);
            let value = serde_json::from_str::<serde_json::Value>(&body)
                .map_err(|_| "响应正文不是有效 JSON，无法执行 JSON Path 断言".to_string())?;
            let actual = json_path_value(&value, path);
            let expected_value = serde_json::from_str::<serde_json::Value>(expected).ok();
            let passed = actual.is_some_and(|value| {
                value.as_str() == Some(expected)
                    || expected_value
                        .as_ref()
                        .is_some_and(|expected| value == expected)
            });
            (passed, format!("JSON Path {path} 应匹配"))
        }
        ApiAssertion::LatencyAtMostMillis { expected } => (
            snapshot.elapsed_millis <= *expected,
            format!("耗时应不超过 {expected} ms"),
        ),
    };
    Ok(result)
}

fn json_path_value<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let trimmed = path
        .strip_prefix("$.")
        .or_else(|| path.strip_prefix('$'))
        .unwrap_or(path);
    if trimmed.is_empty() {
        return Some(value);
    }
    trimmed.split('.').try_fold(value, |current, segment| {
        segment
            .parse::<usize>()
            .ok()
            .and_then(|index| current.get(index))
            .or_else(|| current.get(segment))
    })
}

fn redact_sensitive_text(text: &str, environment: &ApiEnvironment) -> String {
    let mut redacted = text.to_string();
    for name in &environment.sensitive_variable_refs {
        if let Some(value) = environment.variable(name)
            && !value.is_empty()
        {
            redacted = redacted.replace(value, "[REDACTED]");
        }
    }
    redacted
}

fn bound_text(value: &str, max_bytes: usize) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if output.len() + character.len_utf8() > max_bytes {
            break;
        }
        output.push(character);
    }
    output
}
