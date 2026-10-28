# RAMAG-PLUGIN-MIGRATE-002：剪贴板第一迁移批次

> 状态：已完成静态适配与代码验证。
>
> 日期：2026-10-28。

本批次把 Windows/macOS 上的 `ramag-tool-clipboard` 登记为显式第一方静态插件。`ClipboardTool`、`ClipboardService`、本地加密历史、全局热键和浮层视图路径保持不变。

固定边界：插件 ID 和入口 ID 为 `clipboard`，API 为 `1.0`，只声明 `ui.entry`，不新增插件设置、文件、网络或凭据能力。剪贴板采集、历史清理和粘贴动作继续使用现有宿主服务。

验证结果：`cargo fmt --all -- --check`、`cargo test --locked -p ramag-bin`（19 项）、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。
