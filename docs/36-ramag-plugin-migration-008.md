# RAMAG-PLUGIN-MIGRATE-008：Kafka 插件迁移

> 状态：已完成静态适配与代码验证。
>
> 日期：2026-10-29。

本批次把现有 `ramag-tool-kafka` 登记为显式第一方静态插件。`KafkaTool`、`KafkaService`、Kafka 传输层、
Broker/Topic/Partition/消息工作区和现有 GPUI 视图保持原有组合路径。

## 固定边界

- 插件 ID 和入口 ID 保持为 `kafka`，显示名称保持为“Kafka”。
- 插件 API 版本为 `1.0`，只声明 `ui.entry`，不新增插件设置、文件、网络或凭据能力。
- Broker 连接、Topic 和消息读取、生产、消费组、Schema Registry、Kafka Connect、ksqlDB、ACL 与配置管理
  仍由现有宿主服务与用户操作控制；本批次不改变协议、认证、请求上限、取消或错误恢复行为。
- 注册或初始化失败仍由 `StaticPluginHost` 隔离；其他工具注册、初始化和工具顺序保持不变。

## 实现

- 在 `ramag-bin` 组合根中使用显式 Kafka 插件描述替换通用 `StaticPluginAdapter::from_tool` 路径。
- 描述固定 Kafka 插件和入口的稳定 ID、名称、用途说明与最小 `ui.entry` 能力。
- 增加描述模型回归测试，确认入口、能力和设置列表不会意外变化。

## 验证

- `cargo fmt --all -- --check`
- `cargo test --locked -p ramag-bin`
- `cargo test --locked -p ramag-tool-kafka`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

本批次只改变静态插件描述和组合根登记，不增加页面入口或改变 Kafka 服务行为；不需要新增真实窗口流程。
Kafka 本机 Docker、Broker 指标和真实窗口证据继续按 Kafka 专项矩阵记录。

## 回滚边界

只回滚 Kafka 插件描述、组合根登记、专项测试和本记录；不回滚 Kafka 驱动、集群配置、凭据、用户数据
或其他插件迁移批次。
