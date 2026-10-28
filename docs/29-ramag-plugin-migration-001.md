# RAMAG-PLUGIN-MIGRATE-001：API 工作台第一迁移批次

> 状态：已完成静态适配与代码验证。
>
> 日期：2026-10-28。

本批次把现有 `ramag-tool-api` 登记为显式第一方静态插件。`ApiTool`、`ApiService`、HTTP/gRPC 驱动、请求执行边界和 `create_api_view` 保持原有组合路径。

## 固定边界

- 插件 ID 和入口 ID 保持为 `api`，显示名称保持为“API 测试”。
- API 版本为 `1.0`，只声明 `ui.entry`，不新增插件设置、文件、网络或凭据能力。
- 请求仍由用户在 API 工作台中触发；本批次不改变协议、认证、历史记录或请求数据。
- 失败仍由 `StaticPluginHost` 隔离；其他工具注册、初始化和工具顺序保持不变。

## 验证

- `cargo fmt --all -- --check`
- `cargo test --locked -p ramag-bin`：18 项通过。
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

本批次只改变静态插件描述和组合根登记，不增加新的页面入口，因此不需要新增真实窗口流程；既有 API 工作台真实窗口和 Headless 证据继续有效。
