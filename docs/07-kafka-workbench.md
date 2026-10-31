# Kafka 工作台说明

本文汇总 Kafka 工作台、传输适配器、Broker 指标和验证边界。当前任务与执行顺序见[主线开发计划](01-development-plan.md)。
历史测试只证明当时的环境与范围；运行时能力、真实服务和原生窗口分别验证。

## 术语与边界

| 名称 | English / Acronym | 职责与限制 |
|---|---|---|
| 集群 | Kafka Cluster | 一组 Broker 及其接入配置，不等同于单个 Broker |
| 主题 | Topic | 消息的逻辑分类，由一个或多个 Partition 组成 |
| 分区 | Partition | 主题内有序的消息日志；不同分区不保证全局顺序 |
| 消费者组 | Consumer Group | 维护消费者成员、分配与已提交 Offset，不是界面用户权限组 |
| 消息浏览上下文 | Message Browse Context | 当前选择的 Topic、Partition、Offset 或时间范围和读取预算；选择本身不触发读取 |
| 协议指标 | Kafka Protocol Metrics | 从元数据、Offset 和消费者组查询得到的健康与进度，不包含 Broker 进程 CPU 或内存 |
| Broker 运行指标 | Broker Runtime Metrics | 外部 Prometheus/OpenMetrics 文本端点提供的进程指标，不由 Kafka Admin API 返回 |

## 工作台能力

| 能力 | 已记录的实现与边界 |
|---|---|
| 集群与 Broker | 保存配置、测试连接、读取元数据和健康状态；来源没有提供的字段保持未知 |
| Topic 与 Partition | 查询、搜索和浏览分区；创建、删除和扩容使用受权限约束的管理操作 |
| 消息读取 | 按分区、Offset 或时间读取有限范围；查看 Key、Value、Headers 和元数据 |
| 定位 | Topic、消费者组或概览页只更新消息浏览上下文；用户显式读取后才访问 Broker |
| 搜索、导出与实时流 | 在有界范围内搜索、取消和导出；Tail 支持暂停与停止，不代替业务消费者 |
| 单条生产 | 管理模式下确认目标后发送单条消息并展示结果；批量生产与重放不在当前范围 |
| 消费者组 | 查询成员、分配、已提交 Offset 与 Lag；重置 Offset 属于显式管理操作 |
| 配置与 ACL | 查询资源配置和 ACL；只对受支持的动态配置及精确 ACL 执行确认后的修改 |
| Schema Registry | 浏览 Subject、版本和 Schema 内容；HTTP 服务证据与原生窗口证据分开 |
| Kafka Connect | 只读浏览连接器和 Task 状态，不提供写操作 |
| ksqlDB | 有界只读查询，支持取消；不执行 DDL、写入或查询管理命令 |

读取、搜索、Tail、生产、消费者组管理和指标刷新各自保留请求状态、错误和资源预算。
普通消息浏览不提交业务 Consumer Group Offset。写入失败必须显示实际错误，不能按成功展示。

## 传输与构建

传输层（Kafka Transport）封装客户端请求、预算、取消和错误转换；领域层、应用层和 UI 不直接依赖 `rdkafka` 类型。

| 路径 | 构建方式 | 能力与限制 |
|---|---|---|
| native | `ramag-bin` 显式启用 `ramag-infra-kafka/cmake-build`，构建 `rdkafka` / `librdkafka` | 覆盖元数据、消息、消费者组和管理入口；需要 CMake 及目标平台原生工具链 |
| 纯 Rust | 显式启用 `pure-rust`，使用 `rskafka`；不会自动替换 native 路径 | 可列主题、读取分区 Offset/时间范围及有界搜索；不提供完整 Broker/Controller 元数据、消费者组、管理或 TLS |

`ramag-infra-kafka` 自身的默认 feature 为空。未启用对应客户端时，相关入口返回明确的不支持错误。
`kafka-tls` 启用 native TLS 构建，`kafka-sasl` 同时启用 TLS 与 SASL；这只表示存在编译路径。
纯 Rust 路径有 `PLAIN`、`SCRAM-SHA-256` 和 `SCRAM-SHA-512` 配置代码，但没有对应认证服务验收证据。

两个读取路径均限制记录数、字节数、时间和请求超时。错误映射为 `KafkaErrorCategory`；密码、证书路径和密钥不进入普通 Debug 或错误文本。
纯 Rust 路径取消时丢弃当前请求 future，服务端是否立即停止处理尚未单独验证。

## Broker 运行指标

每个 Kafka 配置可选填 HTTP/HTTPS 指标端点。`ExternalBrokerMetrics` 与协议快照分别保存、渲染和报告错误；一方失败不会覆盖另一方的成功结果。
Ramag 不直接连接 JMX。部署侧 exporter 需要提供 UTF-8 Prometheus/OpenMetrics 文本；适配器只读取下列指标：

| 指标名 | 单位 | 必需标签 | 允许值 |
|---|---|---|---|
| `ramag_kafka_broker_cpu_usage_percent` | 百分比 | `broker_id` | `0` 至 `100` |
| `ramag_kafka_broker_memory_used_bytes` | 字节 | `broker_id` | 非负有限数 |
| `ramag_kafka_broker_disk_used_bytes` | 字节 | `broker_id` | 非负有限数 |
| `ramag_kafka_broker_request_latency_ms` | 毫秒 | `broker_id` | 非负有限数 |

样本时间戳可省略；存在时支持 Unix 秒或毫秒。缺少某项指标时显示 `Partial` 和未知字段；没有已知 Broker 样本时显示 `NoData`，不把缺失值写成零。
同一 Broker 的多条磁盘样本求和，请求延迟取最大值。端点返回的 `cluster_id` 不被推断为 Kafka 集群 ID。

可选 Basic Auth 的用户名与密码必须同时配置。密码保存在本机加密存储，回填时留空表示沿用已保存值；日志和错误不包含认证信息或响应正文。
生产端点应使用 HTTPS 并由部署系统注入凭据。HTTP `401`/`403` 显示 `PermissionDenied`；超时、网络错误、响应过大或无效 UTF-8 显示 `CollectionFailed`。
未配置端点显示 `SourceNotConfigured`。单次请求超时 `5s`，响应最多 `4 MiB`，不跟随重定向或读取系统代理。

## 验证与证据

- 2026-09 的本机 Docker KRaft 测试记录覆盖元数据、消息读取与搜索、消费者组及 Topic 管理；纯 Rust 候选另有本机读取和时间范围测试记录。
- 本地静态 OpenMetrics 测试响应验证 HTTP 接入；`apache/kafka:4.0.0` 与 JMX Exporter `1.6.0` 的本机测试另外验证 JVM 指标与认证响应。
- 上述结果不证明生产 exporter 配置、多 Broker 部署、TLS/SASL/ACL 服务兼容或真实窗口交互。缺少对应环境时保持未验证。
- 仓库的本机服务启动、端口、健康检查和清理方法见[开发入门指南](03-development-guide.md#96-本地-kafka-服务与指标验收)。
- 修改功能时分别运行受影响 crate 的目标测试、适用的本机 Docker 服务测试和 UI 验收；headless 结果不能代替原生窗口操作。

接口与测试入口：[`kafka_driver.rs`](../crates/ramag-domain/src/traits/kafka_driver.rs)、[`ramag-infra-kafka`](../crates/ramag-infra-kafka)、[`Docker 集成测试`](../crates/ramag-infra-kafka/tests/docker_kafka.rs)和[测试脚本](../scripts/kafka-test/kafka-test.ps1)。
