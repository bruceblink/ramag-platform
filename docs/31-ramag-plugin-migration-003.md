# RAMAG-PLUGIN-MIGRATE-003：版本管理第一迁移批次

> 状态：已完成静态适配与代码验证。
>
> 日期：2026-10-28。

本批次把 `ramag-tool-vcs` 登记为显式第一方静态插件。`VcsTool`、Git 驱动、仓库选择和 GPUI 工作台保持原有路径。

插件 ID 和入口 ID 为 `vcs`，API 为 `1.0`，只声明 `ui.entry`。仓库操作仍由用户显式触发，不新增网络、凭据或插件设置能力。

验证结果：`cargo fmt --all -- --check`、`cargo test --locked -p ramag-bin`（20 项）、workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。
