use ramag_domain::entities::{ApiImportSummary, ApiWorkspace, import_api_json};
use ramag_domain::error::{DomainError, Result};

use super::ApiService;

impl ApiService {
    /// 解析 Ramag/Postman JSON、合并到当前工作区并一次性持久化；失败时不写入部分结果。
    pub async fn import_workspace_json(
        &self,
        current: &ApiWorkspace,
        raw: &str,
    ) -> Result<(ApiWorkspace, ApiImportSummary)> {
        let bundle = import_api_json(raw).map_err(DomainError::InvalidConfig)?;
        let mut workspace = current.clone();
        let summary = bundle
            .merge_into(&mut workspace)
            .map_err(DomainError::InvalidConfig)?;
        self.save_workspace(&workspace).await?;
        Ok((workspace, summary))
    }
}
