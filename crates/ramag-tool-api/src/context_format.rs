//! API 工作台导入回填用的文本格式化规则。

use super::*;

/// 将领域参数转换为编辑器的逐行 `name: value` 格式，保持 UI 回填可逆且稳定。
pub(super) fn format_parameters(parameters: &[ApiParameter]) -> String {
    parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, parameter.value))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 将查询参数转换为逐行 `name=value` 格式，避免把 URL 编码职责混入 UI 文本层。
pub(super) fn format_query_parameters(parameters: &[ApiParameter]) -> String {
    parameters
        .iter()
        .map(|parameter| format!("{}={}", parameter.name, parameter.value))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 将断言领域枚举转换为工作台约定的短语法，未知字段不会被静默丢弃。
pub(super) fn format_assertions(assertions: &[ApiAssertion]) -> String {
    assertions
        .iter()
        .map(|assertion| match assertion {
            ApiAssertion::HttpStatus { expected } => format!("status={expected}"),
            ApiAssertion::HeaderEquals { name, expected } => {
                format!("header={name}:{expected}")
            }
            ApiAssertion::MetadataEquals { name, expected } => {
                format!("metadata={name}:{expected}")
            }
            ApiAssertion::BodyContains { expected } => format!("body={expected}"),
            ApiAssertion::JsonPathEquals { path, expected } => format!("json={path}:{expected}"),
            ApiAssertion::LatencyAtMostMillis { expected } => format!("latency={expected}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 将响应变量提取规则序列化回编辑器文本，并保留 `secret` 标记和来源类型。
pub(super) fn format_response_variables(extractions: &[ApiVariableExtraction]) -> String {
    extractions
        .iter()
        .map(|extraction| {
            let prefix = if extraction.sensitive { "secret " } else { "" };
            let source = match &extraction.source {
                ApiVariableSource::JsonPath { path } => format!("json:{path}"),
                ApiVariableSource::Header { name } => format!("header:{name}"),
                ApiVariableSource::Metadata { name } => format!("metadata:{name}"),
                ApiVariableSource::Body => "body".into(),
            };
            format!("{prefix}{}={source}", extraction.name)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
