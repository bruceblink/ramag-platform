//! 有界 JSON5 输入上的 JSON Path 提取核心；不依赖 UI 或插件运行时。

use serde_json::Value;
use thiserror::Error;

/// JSON Path 文本允许的最大 UTF-8 字节数。
pub const MAX_JSON_PATH_BYTES: usize = 1024;
/// JSON/JSON5 输入允许的最大字节数；入口清单还会应用更小的具体上限。
pub const MAX_JSON_INPUT_BYTES: usize = 4 * 1024 * 1024;
/// 单次查询允许产生的最大匹配数，防止通配符展开无界增长。
pub const MAX_JSON_PATH_MATCHES: usize = 65_536;
/// 格式化结果允许的最大字节数。
pub const MAX_JSON_PATH_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_ERROR_BYTES: usize = 512;

/// JSON Path 的最小可移植片段；键名、数组索引和通配符都保持独立可测试。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonPathSegment {
    Key(String),
    Index(i64),
    Wildcard,
}

/// JSON Path 解析、查询和有界格式化失败时返回的诊断。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JsonPathError {
    #[error("JSON Path 不能为空")]
    EmptyPath,
    #[error("JSON Path 超过 {max} 字节（实际 {actual} 字节）")]
    PathTooLarge { actual: usize, max: usize },
    #[error("JSON 输入超过 {max} 字节（实际 {actual} 字节）")]
    InputTooLarge { actual: usize, max: usize },
    #[error("JSON Path 包含空片段")]
    EmptySegment,
    #[error("JSON Path 缺少右方括号")]
    MissingClosingBracket,
    #[error("JSON 输入无效：{message}")]
    InvalidJson { message: String },
    #[error("JSON Path 匹配数量超过 {max}")]
    TooManyMatches { max: usize },
    #[error("JSON Path 输出超过 {max} 字节（实际 {actual} 字节）")]
    OutputTooLarge { actual: usize, max: usize },
}

/// 把 JSON Path 文本解析为可执行片段；支持点号、方括号、通配符和负数组索引。
pub fn tokenize_json_path(path: &str) -> Result<Vec<JsonPathSegment>, JsonPathError> {
    if path.trim().is_empty() {
        return Err(JsonPathError::EmptyPath);
    }
    if path.len() > MAX_JSON_PATH_BYTES {
        return Err(JsonPathError::PathTooLarge {
            actual: path.len(),
            max: MAX_JSON_PATH_BYTES,
        });
    }

    let path = path.trim();
    let mut segments = Vec::new();
    let mut index = usize::from(path.starts_with('$'));
    while index < path.len() {
        let character = path.as_bytes()[index] as char;
        if character == '.' {
            index += 1;
            if path.as_bytes().get(index).copied() == Some(b'*') {
                segments.push(JsonPathSegment::Wildcard);
                index += 1;
                continue;
            }
            let start = index;
            while index < path.len() && !matches!(path.as_bytes()[index], b'.' | b'[') {
                index += 1;
            }
            push_key(&mut segments, path[start..index].trim())?;
            continue;
        }

        if character == '[' {
            let (value, end) = read_bracket_segment(path, index)?;
            let value = value.trim();
            if value.is_empty() {
                return Err(JsonPathError::EmptySegment);
            }
            if value == "*" {
                segments.push(JsonPathSegment::Wildcard);
            } else if let Ok(number) = value.parse::<i64>() {
                segments.push(JsonPathSegment::Index(number));
            } else {
                segments.push(JsonPathSegment::Key(parse_quoted_segment(value)));
            }
            index = end + 1;
            continue;
        }

        let start = index;
        while index < path.len() && !matches!(path.as_bytes()[index], b'.' | b'[') {
            index += 1;
        }
        push_key(&mut segments, path[start..index].trim())?;
    }
    Ok(segments)
}

/// 在已经解析的 JSON 值上执行 JSON Path；结果顺序与输入数组和查询片段一致。
pub fn query_json_path(value: &Value, path: &str) -> Result<Vec<Value>, JsonPathError> {
    let segments = tokenize_json_path(path)?;
    let mut current = vec![value.clone()];
    for segment in segments {
        let mut next = Vec::new();
        for candidate in current {
            match &segment {
                JsonPathSegment::Wildcard => match candidate {
                    Value::Array(values) => next.extend(values),
                    Value::Object(values) => next.extend(values.into_values()),
                    _ => {}
                },
                JsonPathSegment::Index(index) => match candidate {
                    Value::Array(values) => {
                        if let Some(value) = array_value(values, *index) {
                            next.push(value);
                        }
                    }
                    Value::Object(values) => {
                        if let Some(value) = values.get(&index.to_string()) {
                            next.push(value.clone());
                        }
                    }
                    _ => {}
                },
                JsonPathSegment::Key(key) => {
                    if let Value::Object(values) = candidate
                        && let Some(value) = values.get(key)
                    {
                        next.push(value.clone());
                    }
                }
            }
            if next.len() > MAX_JSON_PATH_MATCHES {
                return Err(JsonPathError::TooManyMatches {
                    max: MAX_JSON_PATH_MATCHES,
                });
            }
        }
        current = next;
    }
    Ok(current)
}

/// 解析 JSON5、执行查询并生成与 Web 工具一致的 JSON 缩进结果。
pub fn extract_json_path(raw_json: &str, path: &str) -> Result<String, JsonPathError> {
    if raw_json.is_empty() {
        return Ok(String::new());
    }
    if raw_json.len() > MAX_JSON_INPUT_BYTES {
        return Err(JsonPathError::InputTooLarge {
            actual: raw_json.len(),
            max: MAX_JSON_INPUT_BYTES,
        });
    }
    let segments = tokenize_json_path(path)?;
    let value = json5::from_str::<Value>(raw_json).map_err(|error| JsonPathError::InvalidJson {
        message: bound_error(error.to_string()),
    })?;
    let matches = query_json_path(&value, path)?;
    if matches.is_empty() {
        return Ok(String::new());
    }
    let result = if segments
        .iter()
        .any(|segment| matches!(segment, JsonPathSegment::Wildcard))
    {
        Value::Array(matches)
    } else {
        matches[0].clone()
    };
    let output =
        serde_json::to_string_pretty(&result).map_err(|error| JsonPathError::InvalidJson {
            message: bound_error(error.to_string()),
        })?;
    if output.len() > MAX_JSON_PATH_OUTPUT_BYTES {
        return Err(JsonPathError::OutputTooLarge {
            actual: output.len(),
            max: MAX_JSON_PATH_OUTPUT_BYTES,
        });
    }
    Ok(output)
}

fn push_key(segments: &mut Vec<JsonPathSegment>, value: &str) -> Result<(), JsonPathError> {
    if value.is_empty() {
        return Err(JsonPathError::EmptySegment);
    }
    segments.push(JsonPathSegment::Key(value.to_owned()));
    Ok(())
}

fn read_bracket_segment(path: &str, start: usize) -> Result<(&str, usize), JsonPathError> {
    let mut quote = None;
    let mut escaped = false;
    for index in start + 1..path.len() {
        let character = path.as_bytes()[index] as char;
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(expected) = quote {
            if character == expected {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'') {
            quote = Some(character);
            continue;
        }
        if character == ']' {
            return Ok((&path[start + 1..index], index));
        }
    }
    Err(JsonPathError::MissingClosingBracket)
}

fn parse_quoted_segment(value: &str) -> String {
    let Some(quote) = value.chars().next() else {
        return String::new();
    };
    if !matches!(quote, '"' | '\'') || !value.ends_with(quote) {
        return value.to_owned();
    }
    let inner = &value[quote.len_utf8()..value.len() - quote.len_utf8()];
    let mut result = String::with_capacity(inner.len());
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(next) = characters.next() {
                result.push(next);
            }
        } else {
            result.push(character);
        }
    }
    result
}

fn array_value(values: Vec<Value>, index: i64) -> Option<Value> {
    let index = if index < 0 {
        values
            .len()
            .checked_sub(usize::try_from(index.unsigned_abs()).ok()?)?
    } else {
        usize::try_from(index).ok()?
    };
    values.get(index).cloned()
}

fn bound_error(message: String) -> String {
    if message.len() <= MAX_ERROR_BYTES {
        return message;
    }
    let mut end = MAX_ERROR_BYTES - 3;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &message[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        users: [
            { name: 'Alice', active: true },
            { name: 'Bob', active: false },
        ],
        meta: { 'request-id': 'abc-123' },
    }"#;

    #[test]
    fn tokenizes_dot_bracket_wildcard_and_index_segments() -> Result<(), JsonPathError> {
        assert_eq!(
            tokenize_json_path("$.users[0].name")?,
            vec![
                JsonPathSegment::Key("users".into()),
                JsonPathSegment::Index(0),
                JsonPathSegment::Key("name".into())
            ]
        );
        assert_eq!(
            tokenize_json_path("$[\"meta\"][\"request-id\"]")?,
            vec![
                JsonPathSegment::Key("meta".into()),
                JsonPathSegment::Key("request-id".into())
            ]
        );
        assert_eq!(
            tokenize_json_path("users[*].name")?,
            vec![
                JsonPathSegment::Key("users".into()),
                JsonPathSegment::Wildcard,
                JsonPathSegment::Key("name".into())
            ]
        );
        Ok(())
    }

    #[test]
    fn extracts_nested_and_wildcard_values_from_json5() -> Result<(), JsonPathError> {
        assert_eq!(
            extract_json_path(SAMPLE, "$.users[0]")?,
            "{\n  \"name\": \"Alice\",\n  \"active\": true\n}"
        );
        assert_eq!(
            extract_json_path(SAMPLE, "$.users[*].name")?,
            "[\n  \"Alice\",\n  \"Bob\"\n]"
        );
        Ok(())
    }

    #[test]
    fn supports_negative_indexes_and_missing_matches() -> Result<(), JsonPathError> {
        let value =
            json5::from_str::<Value>(SAMPLE).map_err(|error| JsonPathError::InvalidJson {
                message: error.to_string(),
            })?;
        assert_eq!(
            query_json_path(&value, "$.users[-1].name")?,
            vec![Value::String("Bob".into())]
        );
        assert!(extract_json_path(SAMPLE, "$.users[9].name")?.is_empty());
        Ok(())
    }

    #[test]
    fn wildcard_keeps_object_insertion_order() -> Result<(), JsonPathError> {
        let value = json5::from_str::<Value>("{ first: 1, second: 2 }").map_err(|error| {
            JsonPathError::InvalidJson {
                message: error.to_string(),
            }
        })?;
        assert_eq!(
            query_json_path(&value, "$.*")?,
            vec![Value::from(1), Value::from(2)]
        );
        Ok(())
    }

    #[test]
    fn rejects_malformed_and_oversized_inputs() {
        assert!(matches!(
            tokenize_json_path("$.users["),
            Err(JsonPathError::MissingClosingBracket)
        ));
        assert!(matches!(
            tokenize_json_path(" "),
            Err(JsonPathError::EmptyPath)
        ));
        assert!(matches!(
            extract_json_path(&"x".repeat(MAX_JSON_INPUT_BYTES + 1), "$"),
            Err(JsonPathError::InputTooLarge { .. })
        ));
    }
}
