use ramag_domain::entities::{
    ApiEnvironment, ApiExecutionOutcome, ApiExecutionResult, ApiHistoryRecord, ApiRequestRecord,
    ApiResponseSnapshot, evaluate_assertions, extract_response_variables,
};
use ramag_domain::error::{DomainError, Result};

/// 处理成功响应、断言和变量提取；变量全部验证通过后才写入当前环境。
pub(super) fn build_success_outcome(
    record: &ApiRequestRecord,
    environment: &mut ApiEnvironment,
    snapshot: ApiResponseSnapshot,
) -> Result<ApiExecutionOutcome> {
    let assertions =
        evaluate_assertions(&record.assertions, &snapshot).map_err(DomainError::InvalidConfig)?;
    let extracted_variables = extract_response_variables(&record.response_variables, &snapshot)
        .map_err(DomainError::InvalidConfig)?;
    environment
        .apply_extracted_variables(&extracted_variables)
        .map_err(DomainError::InvalidConfig)?;
    let passed = assertions.iter().all(|assertion| assertion.passed);
    let result = ApiExecutionResult {
        snapshot,
        assertions,
        passed,
        extracted_variables,
    };
    let history = ApiHistoryRecord::from_success(record, &result, environment);
    Ok(ApiExecutionOutcome {
        result: Some(result),
        error: None,
        cancelled: false,
        history,
    })
}

/// 为传输、取消或响应处理失败创建统一结果，并生成脱敏历史记录。
pub(super) fn failure_outcome(
    record: &ApiRequestRecord,
    environment: &ApiEnvironment,
    error: &str,
    cancelled: bool,
) -> ApiExecutionOutcome {
    ApiExecutionOutcome {
        result: None,
        error: Some(error.to_string()),
        cancelled,
        history: ApiHistoryRecord::from_error(record, error, environment),
    }
}
