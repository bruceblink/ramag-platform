# RAMAG-PLUGIN-MIGRATE-004：SSH 第一迁移批次

> 状态：已完成静态适配与代码验证。
>
> 日期：2026-10-28。

本批次把 `ramag-tool-ssh` 登记为显式第一方静态插件。`SshTool`、`SshService`、终端、SFTP 和传输队列保持原有组合路径。

插件 ID 和入口 ID 为 `ssh`，API 为 `1.0`，只声明 `ui.entry`。远程连接、凭据和文件操作继续由现有宿主服务控制，不新增插件设置或其他能力。

验证结果：`cargo fmt --all -- --check`、`cargo test --locked -p ramag-bin`（21 项）、workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。
