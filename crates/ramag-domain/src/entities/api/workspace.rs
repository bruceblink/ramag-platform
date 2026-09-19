use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
    ApiCollectionId, ApiEnvironmentId, ApiRequestRecord, MAX_API_COLLECTION_NAME_BYTES,
    MAX_API_COLLECTIONS, MAX_API_ENVIRONMENT_NAME_BYTES, MAX_API_ENVIRONMENT_VARIABLES,
    MAX_API_ENVIRONMENTS, MAX_API_PARAMETER_VALUE_BYTES, MAX_API_REQUESTS,
    MAX_API_VARIABLE_NAME_BYTES, validate_protocol_name, validate_required_text, validate_text,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiCollection {
    pub id: ApiCollectionId,
    pub name: String,
    #[serde(default)]
    pub requests: Vec<ApiRequestRecord>,
}

impl ApiCollection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: ApiCollectionId::new(),
            name: name.into(),
            requests: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_required_text(
            "API Collection 名称",
            &self.name,
            MAX_API_COLLECTION_NAME_BYTES,
        )?;
        if self.requests.len() > MAX_API_REQUESTS {
            return Err(format!(
                "单个 API Collection 请求数量超过 {MAX_API_REQUESTS} 条上限"
            ));
        }
        for request in &self.requests {
            request.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiEnvironment {
    pub id: ApiEnvironmentId,
    pub name: String,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub sensitive_variable_refs: Vec<String>,
}

impl fmt::Debug for ApiEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiEnvironment")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("variable_names", &self.variables.keys().collect::<Vec<_>>())
            .field("sensitive_variable_refs", &self.sensitive_variable_refs)
            .finish()
    }
}

impl ApiEnvironment {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: ApiEnvironmentId::new(),
            name: name.into(),
            variables: BTreeMap::new(),
            sensitive_variable_refs: Vec::new(),
        }
    }

    /// 返回变量值供执行驱动使用；调用方不得把返回值写入普通日志或响应历史。
    pub fn variable(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(String::as_str)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_required_text(
            "API Environment 名称",
            &self.name,
            MAX_API_ENVIRONMENT_NAME_BYTES,
        )?;
        if self.variables.len() > MAX_API_ENVIRONMENT_VARIABLES {
            return Err(format!(
                "API Environment 变量数量超过 {MAX_API_ENVIRONMENT_VARIABLES} 个上限"
            ));
        }
        for (name, value) in &self.variables {
            validate_protocol_name("API Environment 变量名", name, MAX_API_VARIABLE_NAME_BYTES)?;
            validate_text(
                "API Environment 变量值",
                value,
                MAX_API_PARAMETER_VALUE_BYTES,
                false,
            )?;
        }
        let mut refs = HashSet::with_capacity(self.sensitive_variable_refs.len());
        for name in &self.sensitive_variable_refs {
            validate_protocol_name(
                "敏感 API Environment 变量引用",
                name,
                MAX_API_VARIABLE_NAME_BYTES,
            )?;
            if !self.variables.contains_key(name) {
                return Err(format!("敏感变量引用不存在：{name}"));
            }
            if !refs.insert(name) {
                return Err(format!("敏感变量引用重复：{name}"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiWorkspace {
    pub id: super::ApiWorkspaceId,
    pub name: String,
    #[serde(default)]
    pub collections: Vec<ApiCollection>,
    #[serde(default)]
    pub environments: Vec<ApiEnvironment>,
    #[serde(default)]
    pub default_environment_id: Option<ApiEnvironmentId>,
}

impl ApiWorkspace {
    /// 创建空工作区；Collection 和 Environment 由 UI/应用层逐步添加。
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: super::ApiWorkspaceId::new(),
            name: name.into(),
            collections: Vec::new(),
            environments: Vec::new(),
            default_environment_id: None,
        }
    }

    /// 校验聚合内的数量、ID 引用和请求资源限制，Storage 与执行入口共用此规则。
    pub fn validate(&self) -> Result<(), String> {
        validate_required_text(
            "API Workspace 名称",
            &self.name,
            super::MAX_API_WORKSPACE_NAME_BYTES,
        )?;
        if self.collections.len() > MAX_API_COLLECTIONS {
            return Err(format!(
                "API Collection 数量超过 {MAX_API_COLLECTIONS} 个上限"
            ));
        }
        if self.environments.len() > MAX_API_ENVIRONMENTS {
            return Err(format!(
                "API Environment 数量超过 {MAX_API_ENVIRONMENTS} 个上限"
            ));
        }

        let mut collection_ids = HashSet::with_capacity(self.collections.len());
        let mut request_ids = HashSet::new();
        let mut request_count = 0usize;
        for collection in &self.collections {
            if !collection_ids.insert(collection.id.clone()) {
                return Err(format!("API Collection ID 重复：{}", collection.id));
            }
            collection.validate()?;
            request_count = request_count
                .checked_add(collection.requests.len())
                .ok_or_else(|| "API 请求数量溢出".to_string())?;
            for request in &collection.requests {
                if !request_ids.insert(request.id.clone()) {
                    return Err(format!("API 请求 ID 重复：{}", request.id));
                }
            }
        }
        if request_count > MAX_API_REQUESTS {
            return Err(format!(
                "API Workspace 请求数量超过 {MAX_API_REQUESTS} 条上限"
            ));
        }

        let mut environment_ids = HashSet::with_capacity(self.environments.len());
        for environment in &self.environments {
            if !environment_ids.insert(environment.id.clone()) {
                return Err(format!("API Environment ID 重复：{}", environment.id));
            }
            environment.validate()?;
        }
        if let Some(default_id) = &self.default_environment_id
            && !environment_ids.contains(default_id)
        {
            return Err("默认 API Environment 不存在".into());
        }
        Ok(())
    }
}
