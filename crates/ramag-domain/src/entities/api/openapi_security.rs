use serde_json::{Map, Value};

use super::super::super::{
    ApiAuth, ApiKeyLocation, HttpRequestSpec, MAX_API_ENVIRONMENT_VARIABLES,
    MAX_API_VARIABLE_NAME_BYTES,
};
use super::super::{format_error, required_object, required_string};
use super::support::sanitize_name;
use super::{OpenApiParser, resolve_ref, validate_openapi_name};

impl<'a> OpenApiParser<'a> {
    /// 把首个安全的 security requirement 映射到现有认证模型。
    pub(super) fn apply_security(
        &mut self,
        operation: &Map<String, Value>,
        path: &str,
        spec: &mut HttpRequestSpec,
    ) -> Result<(), String> {
        let security = operation
            .get("security")
            .cloned()
            .or_else(|| self.root.get("security").cloned());
        let Some(security) = security else {
            return Ok(());
        };
        let requirements = security
            .as_array()
            .ok_or_else(|| format_error("$.security", "必须是数组"))?;
        let Some(first) = requirements.first() else {
            return Ok(());
        };
        if requirements.len() > 1 {
            self.warn(path, "存在多个安全方案，仅导入第一个方案");
        }
        let first = required_object(first, &format!("{path}.security[0]"))?;
        let Some((scheme_name, scopes)) = first.iter().next() else {
            return Ok(());
        };
        if !scopes.as_array().is_some_and(Vec::is_empty) {
            self.warn(
                &format!("{path}.security[0].{scheme_name}"),
                "OAuth scopes 未映射",
            );
        }
        let scheme_path = format!("$.components.securitySchemes.{scheme_name}");
        let schemes = self
            .root
            .get("components")
            .and_then(Value::as_object)
            .and_then(|components| components.get("securitySchemes"))
            .ok_or_else(|| format_error(&scheme_path, "字段缺失"))?;
        let schemes = required_object(schemes, "$.components.securitySchemes")?;
        let raw_scheme = schemes
            .get(scheme_name)
            .ok_or_else(|| format_error(&scheme_path, "安全方案不存在"))?;
        let scheme = resolve_ref(self.root, raw_scheme, &scheme_path)?;
        let scheme = required_object(&scheme, &scheme_path)?;
        let kind = required_string(scheme, "type", &scheme_path)?;
        match kind.as_str() {
            "apiKey" => {
                let name = required_string(scheme, "name", &scheme_path)?;
                let location = match required_string(scheme, "in", &scheme_path)?.as_str() {
                    "header" => ApiKeyLocation::Header,
                    "query" => ApiKeyLocation::Query,
                    _ => {
                        return Err(format_error(
                            &format!("{scheme_path}.in"),
                            "API Key 只支持 header 或 query",
                        ));
                    }
                };
                let variable = self.security_variable(scheme_name, "key", &scheme_path)?;
                spec.auth = ApiAuth::ApiKey {
                    name,
                    value: format!("{{{{{variable}}}}}"),
                    location,
                };
            }
            "http" => {
                let scheme_kind =
                    required_string(scheme, "scheme", &scheme_path)?.to_ascii_lowercase();
                match scheme_kind.as_str() {
                    "basic" => {
                        let username =
                            self.security_variable(scheme_name, "username", &scheme_path)?;
                        let password =
                            self.security_variable(scheme_name, "password", &scheme_path)?;
                        spec.auth = ApiAuth::Basic {
                            username: format!("{{{{{username}}}}}"),
                            password: format!("{{{{{password}}}}}"),
                        };
                    }
                    "bearer" => {
                        let variable =
                            self.security_variable(scheme_name, "token", &scheme_path)?;
                        spec.auth = ApiAuth::Bearer {
                            token: format!("{{{{{variable}}}}}"),
                        };
                    }
                    _ => self.warn(&format!("{scheme_path}.scheme"), "HTTP 认证方案未映射"),
                }
            }
            "oauth2" | "openIdConnect" => {
                self.warn(&scheme_path, "OAuth2/OpenID Connect 需要运行时授权，未映射")
            }
            _ => {
                return Err(format_error(
                    &format!("{scheme_path}.type"),
                    "不支持的安全方案类型",
                ));
            }
        }
        Ok(())
    }

    /// 创建认证占位变量并标记为敏感，避免把凭据写入普通导出或日志。
    fn security_variable(
        &mut self,
        scheme_name: &str,
        suffix: &str,
        path: &str,
    ) -> Result<String, String> {
        let name = format!("openapi_{}_{}", sanitize_name(scheme_name), suffix);
        if name.len() > MAX_API_VARIABLE_NAME_BYTES {
            return Err(format_error(path, "认证方案名称生成的变量名过长"));
        }
        self.ensure_variable(&name, "", path, true)?;
        Ok(name)
    }

    /// 在环境中写入变量；同名不同默认值保留先出现的值并给出路径提示。
    pub(super) fn ensure_variable(
        &mut self,
        name: &str,
        value: &str,
        path: &str,
        sensitive: bool,
    ) -> Result<(), String> {
        validate_openapi_name(name, path)?;
        if let Some(existing) = self.environment.variables.get(name) {
            if existing != value {
                self.warn(path, "同名环境变量已有不同默认值，保留先出现的值");
            }
        } else {
            if self.environment.variables.len() >= MAX_API_ENVIRONMENT_VARIABLES {
                return Err(format_error(path, "环境变量数量超过上限"));
            }
            self.environment
                .variables
                .insert(name.to_string(), value.to_string());
        }
        if sensitive
            && !self
                .environment
                .sensitive_variable_refs
                .iter()
                .any(|item| item == name)
        {
            self.environment
                .sensitive_variable_refs
                .push(name.to_string());
        }
        Ok(())
    }
}
