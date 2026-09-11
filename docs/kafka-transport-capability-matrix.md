# Kafka 传输能力矩阵

> 文档状态：`KAFKA-001` 纯 Rust 读取候选阶段性增补稿；当前仅记录现有实现和可复核证据，不表示纯 Rust 客户端已经具备完整替换能力
> 更新日期：2026-09-11
> 适用范围：`ramag-domain`、`ramag-app`、`ramag-infra-kafka`、`ramag-tool-kafka` 和 `ramag-bin`
> 代码基线：`dev` 阶段 18-28 和 `KAFKA-001` 纯 Rust 读取候选代码；当前实施分支：`dev`（只保留并同步 `main`、`dev`）

本文把 Kafka 协议能力、客户端构建方式和验证证据放在同一张表中。表中的“已实现”只表示当前代码存在对应入口；只有同时具备目标测试、真实 Kafka 服务或明确记录的环境限制，才能把能力写成已验收。

## 术语与命名规则

| 名称 | English / Acronym | 本文中的职责 | 不表示什么 |
|---|---|---|---|
| Kafka 传输层 | Kafka Transport | 隔离 Kafka 网络客户端、请求预算、错误转换和能力检测的基础设施边界 | 不表示 Kafka UI、领域实体或某一个客户端库 |
| native 客户端 | `rdkafka` / `librdkafka` | 当前工作区已经接入的 C/C++ 客户端适配路径 | 不表示默认构建不需要本机编译工具链 |
| 纯 Rust 客户端 | Pure Rust Client | 由 Rust 代码直接实现 Kafka 协议和网络请求，不链接 `librdkafka` 的候选路径 | 不表示给 `rdkafka` 再包一层 Rust 类型 |
| 代码证据 | Code Evidence | 源码、trait、feature gate 和单元测试证明某入口存在 | 不表示真实 Broker 已接受该请求 |
| 服务证据 | Service Evidence | Docker KRaft、测试 Broker 或其他实际 Kafka 服务返回的结果 | 不表示真实生产集群已经验证 |
| 构建能力 | Build Capability | 当前 Cargo feature 是否包含对应客户端和 TLS/SASL 编译支持 | 不表示认证参数和远端权限一定正确 |

## 评估边界

本矩阵只比较当前 native 客户端基线和纯 Rust 客户端候选路径。应用层已经通过 `KafkaDriver`、`KafkaAdminDriver` 和 `KafkaMonitoringDriver` 分开读取、管理和观测操作；本阶段不把 `rdkafka` 类型暴露到领域层、应用层或 UI 层。

当前构建事实：

- `ramag-infra-kafka` 的默认 feature 为空；不启用 `cmake-build` 时，连接、读取和管理入口会返回结构化的“不支持 native 客户端”错误，不访问网络。
- `ramag-bin` 当前显式依赖 `ramag-infra-kafka` 的 `cmake-build` feature，因此桌面二进制的 workspace 构建仍会进入 `librdkafka` 的 CMake 构建路径。
- `KafkaMonitoringDriver::metrics_snapshot` 当前使用协议层 Metadata、Topic/Partition 末尾 Offset 和 Consumer Group/Offset 查询；它只提供 Kafka 协议快照，不代替 JMX、Prometheus 或 exporter 的 Broker 运行指标。
- `kafka-tls` 依赖 `cmake-build` 和 `rdkafka/ssl-vendored`；`kafka-sasl` 依赖 `kafka-tls` 和 `rdkafka/sasl`。这两个 feature 只说明本地代码具备编译路径，不等于 TLS/SASL 服务已验收。
- `ramag-infra-kafka` 的显式 `pure-rust` feature 使用 `rskafka`，提供不链接 `librdkafka` 的 Topic 列表、Partition Offset/时间范围读取和客户端消息搜索；默认构建和现有 native 用户流程不变。
- 纯 Rust 路径当前不提供完整 Broker/Controller Metadata、Consumer Group、Topic/Config/ACL 管理、Broker 运行指标或 TLS；PLAIN、SCRAM-SHA-256 和 SCRAM-SHA-512 只有代码级配置支持，尚未取得认证服务证据。

## 能力矩阵

| 能力 | 领域/应用入口 | 当前 native 实现 | 构建和运行条件 | 纯 Rust 路径 | 当前证据和缺口 |
|---|---|---|---|---|---|
| Metadata | `KafkaDriver::cluster_metadata` | `RdkafkaDriver::cluster_metadata_blocking` 使用 `fetch_metadata` 和 `fetch_cluster_id`；Broker 地址和副本信息映射到领域实体 | `cmake-build`；明文 Docker KRaft 已有历史集成记录 | 未支持 | native 代码和默认 feature 测试通过；纯 Rust 客户端当前没有完整 Broker/Controller 快照接口。`controller_id` 和 `kafka_version` 当前没有可靠来源，保持空值，不能写成已提供 |
| Fetch | `KafkaDriver::read_messages`、`search_messages` | `messages.rs` 创建独立 `BaseConsumer`，手动分配 Partition 后调用 `poll`，不提交 Offset | native 使用 `cmake-build`；查询必须通过 Topic、Partition、Offset/时间和记录/字节/时间预算校验 | 已实现（显式 `pure-rust`） | `PureRustTransport` 使用 `PartitionClient` 读取多个 Partition，不提交业务 Offset；Windows GNU 单元测试和本机 Docker KRaft 读取、时间范围、搜索测试已通过 |
| ListOffsets | 读取路径内部的 `fetch_watermarks`、`offsets_for_times` | 使用 librdkafka 查询 Partition 首尾 Offset，并把时间转换为起始/结束 Offset | native 使用 `cmake-build`；需要可访问目标 Topic/Partition | 已实现（显式 `pure-rust`） | 纯 Rust 路径使用 `OffsetAt::Earliest`、`Latest` 和 `Timestamp`，并把无匹配时间边界映射到保留起点或当前末尾；本机 Docker KRaft Offset/时间读取已通过 |
| Consumer Group | `KafkaDriver::list_consumer_groups` | `fetch_group_list`、`committed_offsets` 和有界 `ConsumerProtocolAssignment` 解码 | `cmake-build`；需要 Broker 对 Group 查询和 Offset 查询授权 | 未实现 | 领域、解码器和历史 Docker 组成员/Offset/Lag 测试已有；纯 Rust Group/Offset 请求未实现 |
| Metrics Snapshot | `KafkaMonitoringDriver::metrics_snapshot`、`KafkaService::metrics_snapshot` | `RdkafkaTransport::metrics_snapshot_blocking` 复用只读 Metadata、Topic/Partition 末尾 Offset 和 Consumer Group/Offset 查询；领域层按连续 `high watermark` 样本计算 Topic/集群速率 | `cmake-build`；需要可访问元数据、Topic 末尾 Offset、Consumer Group 和 Offset；不读取消息正文、不提交业务 Offset | 未实现 | Domain/App/UI 测试和 native feature 编译通过；当前没有本次 Docker Broker 复核 |
| Broker Runtime Metrics | `KafkaBrokerMetricsDriver::broker_metrics_snapshot`、`KafkaService::broker_metrics_snapshot` | `PrometheusBrokerMetricsDriver` 通过 HTTP/HTTPS 读取固定 Prometheus/OpenMetrics 指标，解析 `broker_id`、数值和样本时间；重复磁盘样本求和、重复延迟样本取最大值；不与协议快照合并 | 可选 exporter 文本端点；5 秒超时、4 MiB 响应上限；JMX 由部署侧 exporter 暴露 | 本机静态 HTTP fixture 和真实 Kafka JMX Exporter 已验证；生产安全配置和多 Broker 部署仍未完成 | Domain/App/Infra/UI 定向测试、本机 `nginx:1.27-alpine` fixture、真实 `apache/kafka:4.0.0` JMX Exporter 和 `docker_kafka_reads_real_broker_jmx_exporter` 通过；端点权限/断开请求和真实 Windows 截图仍未完成 |
| Topic | `KafkaAdminDriver::create_topic`、`delete_topic`、`increase_topic_partitions`；读取侧复用 Metadata | `AdminClient`、`NewTopic`、`NewPartitions` 和 Kafka Admin API | `cmake-build`；变更还需要 `ReadWrite` 配置和 Broker 权限 | 未实现 | 代码、输入校验和历史 Docker 管理测试已有；纯 Rust Topic Admin 请求未实现 |
| Config | `KafkaAdminDriver::describe_configs`、`update_config` | 读取使用 `describe_configs`；修改通过显式 `IncrementalAlterConfigs` FFI，静态/只读项在发送前拒绝 | `cmake-build`；修改需要 `ReadWrite` 配置、动态配置支持和 Broker 权限 | 未实现 | 代码和配置项校验测试已有；当前 Docker fixture 未覆盖静态配置拒绝、动态配置回读和纯 Rust 实现 |
| ACL | `KafkaAdminDriver::list_acls`、`create_acl`、`delete_acl` | 通过 librdkafka Admin FFI 映射 ACL 查询、创建和精确删除；错误转换不保留凭据或消息正文 | `cmake-build`；需要 Broker Authorizer 和对应 ACL 权限 | 未实现 | 代码和领域输入校验已有；当前 Docker fixture 未启用 Authorizer，不能把 ACL 真实服务验收写成通过 |
| TLS | `KafkaSecurityProtocol::Ssl/SaslSsl`、`KafkaTlsConfig` | `ssl.ca.location`、客户端证书/密钥路径和 `TlsVerify` 转换为 librdkafka 属性 | `kafka-tls`；需要证书链、Broker TLS listener 和正确主机名 | 未实现 | feature gate 和属性映射测试通过；没有 TLS Broker fixture、证书轮换或错误证书服务证据 |
| SASL | `KafkaSecurityProtocol::SaslPlaintext/SaslSsl`、`KafkaSaslMechanism` | 支持 `PLAIN`、SCRAM、GSSAPI、OAUTHBEARER 枚举映射；用户名/密码由配置校验后传给 native 客户端 | native 使用 `kafka-sasl`；纯 Rust 仅支持明文 `PLAIN`/SCRAM 代码路径；都需要匹配的 Broker 认证配置 | 代码级支持 `PLAIN`/SCRAM | native feature gate、机制映射和敏感字段脱敏测试通过；纯 Rust 机制拒绝测试通过；没有 SASL Broker fixture、认证成功或拒绝服务证据 |

### 跨能力约束

| 约束 | 当前实现 | 评审结论 |
|---|---|---|
| 超时 | `RdkafkaDriver::with_request_timeout` 和 `PureRustTransport::with_request_timeout` 校验 1 毫秒到 60 秒预算；纯 Rust 请求由 Tokio 超时包住 | 代码级可用；需要在不同 Broker 版本和慢请求场景补服务测试 |
| 取消 | native 读取由应用层的 `smol::unblock` 承载；纯 Rust 读取在独立 Tokio 运行时中用取消信号和 `select!` 停止 | 两条读取路径都有记录数、字节数和时间上限；纯 Rust 取消后丢弃当前请求 future，远端服务取消行为仍未单独验收 |
| 错误 | native 和纯 Rust 错误统一映射为 `KafkaErrorCategory`，重试属性只对网络和超时开放 | 纯 Rust 网络、协议、认证、权限、找不到资源和超时分类已有代码与测试；服务端错误矩阵仍需补充 |
| 敏感数据 | 客户端配置使用 allowlist；密码、证书路径和密钥字段不进入普通 Debug 输出或安全错误正文 | 当前边界应保留到后续适配器，不允许开放任意属性 Map |
| 业务消费进度 | 浏览客户端关闭自动提交和自动 Offset 存储，并使用临时 group ID | native 和纯 Rust 读取都只按 Partition Offset 拉取，不提交业务 Offset；纯 Rust 当前没有 Consumer Group 操作 |

## 证据状态

| 证据 | 结果 | 说明 |
|---|---|---|
| 默认 feature 静态测试 | 已通过 | 本次 Windows MSVC 目标下 `ramag-infra-kafka` 默认 feature 7 项通过；此前 Windows GNU 基线为 5 项；覆盖默认不启用 native、TLS/SASL feature gate、输入校验和错误分类 |
| 阶段 21-22 Domain/App/UI 测试 | 部分通过 | 阶段 21 的 Windows MSVC 证据为 `ramag-domain` 159 项、`ramag-app` 191 项、`ramag-infra-kafka` 默认 feature 7 项、`ramag-tool-kafka` 20 项；阶段 22 的 `ramag-tool-kafka` Windows GNU 测试 22 项通过，覆盖 Broker 健康摘要、快照来源/时间状态和 360/900/1440 宽度布局；当前 MSVC 复测受 Windows SDK 缺少 `msvcrt.lib` 阻塞 |
| native metrics 编译和基础测试 | 已通过 | Windows MSVC 下 `ramag-infra-kafka --features cmake-build` 使用 CMake/NMake 构建 `rdkafka-sys`，`cargo check` 通过，native 基础测试 10 项通过；未连接真实 Broker |
| Cargo feature 关系 | 已核对 | `ramag-infra-kafka` 默认 feature 为空，`ramag-bin` 显式启用 `cmake-build`；`cargo tree --locked -p ramag-bin -e features` 可看到 `rdkafka-sys` 和 CMake 路径 |
| plain KRaft Docker 集成 | 已通过 | 2026-09-09 在 WSL Docker 中运行 `scripts/kafka-test/kafka-test.ps1 test`：先创建并校验 5000 条消息和 61 个主题，再运行 `crates/ramag-infra-kafka/tests/docker_kafka.rs`，元数据、消息读取/搜索、消费者组/Offset 和 Topic 管理 4 项测试全部通过；测试脚本只使用专用 KRaft 容器和测试卷 |
| TLS/SASL Broker 集成 | 未完成 | 当前 fixture 没有 TLS listener、证书链、SASL 用户或 Authorizer 配置 |
| 纯 Rust 客户端阶段性读取候选 | 已通过 | 2026-09-11 Windows GNU 下 `ramag-infra-kafka --no-default-features --features pure-rust` 的 28 项单元测试通过，`cargo clippy --all-targets -- -D warnings` 和格式检查通过；本机 Docker `ramag-kafka-test`（`127.0.0.1:19092`）的 1 项真实集成测试通过，覆盖连接、Topic 列表、三 Partition Offset/时间读取、Value 搜索和未支持 Metadata 返回。Consumer Group、Admin、TLS、真实 114 Broker 和真实 Windows 窗口仍未完成 |
| Linux/macOS 默认构建 | 未在本次 Windows 工作区执行 | 需要对应工具链和独立构建日志；不能用 Windows GNU 结果替代 |

## KAFKA-001 结论和下一步

本矩阵确认了三个直接结论：

1. 当前 native 路径已经覆盖明文 Kafka 的主要读取和管理能力，但它仍依赖 `librdkafka` 构建链；`ramag-bin` 的显式 `cmake-build` 使“默认桌面构建完全不依赖 CMake”尚未成立。
2. TLS、SASL 和 ACL 目前只有代码级支持和 feature gate 证据，缺少对应 Broker fixture；不能把它们写成真实服务验收通过。
3. 纯 Rust 路径已完成阶段性读取候选验证，当前可覆盖 Topic、ListOffsets、有限消息读取和客户端搜索；它仍不能直接替换 native 客户端，不能从本次明文 Docker 结果推断 Consumer Group、Admin、TLS/SASL 服务兼容性。

阶段 18 的文档交付完成；其服务验收仍有明确未完成项。阶段 19 已增加 `KafkaTransport` 适配边界：领域层提供稳定的能力快照接口，`KafkaService` 只暴露能力结果，现有 `RdkafkaTransport` 位于基础设施层并保留 `RdkafkaDriver` 兼容别名；不在该项中伪造纯 Rust 实现或改变现有 Kafka 用户流程。

阶段 21 已增加 `KafkaMonitoringDriver` 和协议指标快照链路：快照包含来源、时间、状态、错误原因、集群/Topic/Partition/Consumer Group 指标和按连续 `high watermark` 样本计算的速率；应用层和 UI 均不依赖 `rdkafka` 类型。

阶段 22 已在 Kafka 概览页整合 Broker 健康摘要、Topic/Partition 健康和消费者组 Lag。Broker 健康只根据 Metadata API 返回结果和协议指标快照中的 Broker 数判断，并明确标出外部 Broker 运行指标尚未接入；明文 KRaft Docker Broker 已在 2026-09-09 完成基础集成复核，真实 Windows 截图仍未完成。

阶段 23 已增加独立的 `KafkaBrokerMetricsDriver` 和 `PrometheusBrokerMetricsDriver`：配置页保存可选指标端点，适配器只读取四个固定指标名，应用层和 UI 分别保留外部运行指标的来源、采样时间、状态和错误。该适配器不直接连接 JMX，也不把配置 ID 作为 Kafka 集群 ID；本机 Docker 已用真实 Kafka JVM、JMX/RMI 和 JMX Exporter 完成请求复核，生产安全配置仍需部署侧单独验收。

2026-09-11 的 `KAFKA-001` 阶段性切片增加 `PureRustTransport`：显式启用 `pure-rust` 后由 `rskafka` 承担 Topic 列表、Partition Offset/时间范围读取和客户端文本/正则搜索；请求在独立 Tokio 运行时中执行，并保留超时、取消、记录数、字节数和扫描时长限制。当前能力快照明确拒绝完整 Metadata、Consumer Group、Admin、Broker 运行指标和 TLS；该切片只证明候选读取路径可在本机 Docker KRaft 上工作，不改变默认 native 路径。

相关代码和测试：

- [`KafkaDriver`](../crates/ramag-domain/src/traits/kafka_driver.rs)
- [`KafkaAdminDriver`](../crates/ramag-domain/src/traits/kafka_driver.rs)
- [`KafkaMonitoringDriver`](../crates/ramag-domain/src/traits/kafka_driver.rs)
- [`RdkafkaDriver`](../crates/ramag-infra-kafka/src/lib.rs)
- [`PureRustTransport`](../crates/ramag-infra-kafka/src/pure_rust.rs)
- [`KafkaMetricsSnapshot`](../crates/ramag-domain/src/entities/kafka_metrics.rs)
- [`ramag-infra-kafka` 默认测试](../crates/ramag-infra-kafka/src/tests.rs)
- [`Docker Kafka 集成测试`](../crates/ramag-infra-kafka/tests/docker_kafka.rs)
- [`纯 Rust Docker Kafka 集成测试`](../crates/ramag-infra-kafka/tests/docker_kafka_pure_rust.rs)
- [`Kafka 测试脚本`](../scripts/kafka-test/kafka-test.ps1)
