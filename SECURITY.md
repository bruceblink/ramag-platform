# 安全策略

## 支持范围

安全修复优先面向本仓库 `main` 分支和本项目最新公开 Release。当前 `bruceblink/ramag-platform` 尚无独立 Release；继承的上游 `0.0.5` 安装包不属于本项目发布物。平台化开发中的问题请直接报告到本仓库，不要默认转发到上游。

## 报告漏洞

请不要在公开 Issue、Pull Request、讨论区或社区群中披露以下问题的可利用细节：

- 凭据、主密钥或本地加密存储泄露；
- SSH、SFTP、数据库、云存储或更新器中的认证绕过、任意文件读写、远程代码执行；
- 更新包来源、哈希校验、签名或安装流程绕过；
- 可能暴露用户连接信息、剪贴板内容或业务数据的问题。

仓库启用 GitHub **Private vulnerability reporting** 后，请优先使用该入口提交报告。若该入口暂不可用，请通过维护者的 GitHub 联系方式请求私密沟通渠道，且在公开内容中只说明发现了安全问题，不披露细节。报告应包含：

1. 受影响的版本、平台和功能模块；
2. 复现步骤与预期/实际结果；
3. 影响范围和可能的缓解方式；
4. 已脱敏的日志、截图或最小示例。

请勿发送真实密码、私钥、访问令牌、生产数据库导出、服务器地址或客户数据。我们会尽力确认报告、评估影响并在修复后协调披露。

## 安全边界

Ramag Platform 将数据库、SSH、JumpServer 和云存储的敏感配置加密保存于本机；主密钥由操作系统凭据库管理。Ramag Platform 不提供托管服务，也不主动上传数据库内容、SSH 凭据、云存储凭据或剪贴板历史。

本项目尚无独立发布包。未来发布包应提供 SHA-256 校验清单；Windows Authenticode 签名、macOS Developer ID 签名和 Apple 公证完成前，不应把未签名构建当作正式发布物。

## English summary

Do not disclose security-sensitive details in public issues. Report issues for this downstream project through its repository security channel; when enabled, use GitHub Private vulnerability reporting for credential exposure, authentication bypass, arbitrary file access, remote code execution, update-integrity, or data-exposure reports. Never include real credentials or production data.
