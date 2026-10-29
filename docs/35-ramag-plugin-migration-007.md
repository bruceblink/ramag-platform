# RAMAG-PLUGIN-MIGRATE-007：数据库客户端插件迁移

> 状态：已完成静态适配与代码验证。
>
> 日期：2026-10-29。

本批次把现有 `ramag-tool-dbclient` 登记为显式第一方静态插件。`DbClientTool`、
`ConnectionService`、MySQL/PostgreSQL/SQLite 驱动、Redis/MongoDB 工作区、数据同步服务和现有 GPUI 视图
保持原有组合路径。

## 固定边界

- 插件 ID 和入口 ID 保持为 `dbclient`，显示名称保持为“数据库客户端”。
- 插件 API 版本为 `1.0`，只声明 `ui.entry`，不新增插件设置、文件、网络或凭据能力。
- 连接配置、凭据、SQL/Redis/MongoDB 查询、事务、结果导出和数据同步仍由现有宿主服务与用户操作控制；
  本批次不改变驱动、协议、事务、取消、分页、资源上限或错误恢复行为。
- 注册或初始化失败仍由 `StaticPluginHost` 隔离；其他工具注册、初始化和工具顺序保持不变。

## 实现

- 在 `ramag-bin` 组合根中使用显式数据库客户端插件描述替换通用 `StaticPluginAdapter::from_tool` 路径。
- 描述固定数据库客户端插件和入口的稳定 ID、中文名称、用途说明与最小 `ui.entry` 能力。
- 增加描述模型回归测试，确认入口、能力和设置列表不会意外变化。

## 验证

- `cargo fmt --all -- --check`
- `cargo test --locked -p ramag-bin`
- `cargo test --locked -p ramag-tool-dbclient`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

本批次只改变静态插件描述和组合根登记，不增加页面入口或改变数据库服务行为；不需要新增真实窗口流程。
数据库连接、查询和同步的真实窗口与本机服务证据继续按数据库专项计划记录。

## 回滚边界

只回滚数据库客户端插件描述、组合根登记、专项测试和本记录；不回滚数据库驱动、连接配置、凭据、
用户数据或其他插件迁移批次。
