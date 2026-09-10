# Kafka 传输能力矩阵

> 文档状态：`KAFKA-001` 阶段 22 增补稿；当前仅记录现有实现和可复核证据，不表示纯 Rust 客户端已经完成
> 更新日期：2026-09-08
> 适用范围：`ramag-domain`、`ramag-app`、`ramag-infra-kafka`、`ramag-tool-kafka` 和 `ramag-bin`
> 代码基线：`dev` 阶段 18-22 代码；当前实施分支：`dev`（只保留并同步 `main`、`dev`）

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
- 当前工作区没有纯 Rust Kafka 客户端依赖、协议实现或 `KafkaTransport` 适配器；矩阵中的纯 Rust 列统一记录为“未实现”。

## 能力矩阵

| 能力 | 领域/应用入口 | 当前 native 实现 | 构建和运行条件 | 纯 Rust 路径 | 当前证据和缺口 |
|---|---|---|---|---|---|
| Metadata | `KafkaDriver::cluster_metadata` | `RdkafkaDriver::cluster_metadata_blocking` 使用 `fetch_metadata` 和 `fetch_cluster_id`；Broker 地址和副本信息映射到领域实体 | `cmake-build`；明文 Docker KRaft 已有历史集成记录 | 未实现 | 代码和默认 feature 测试通过；历史 Docker 集成覆盖元数据读取。`controller_id` 和 `kafka_version` 当前没有可靠来源，保持空值，不能写成已提供 |
| Fetch | `KafkaDriver::read_messages`、`search_messages` | `messages.rs` 创建独立 `BaseConsumer`，手动分配 Partition 后调用 `poll`，不提交 Offset | `cmake-build`；查询必须通过 Topic、Partition、Offset/时间和记录/字节/时间预算校验 | 未实现 | 代码、领域边界测试和历史 Docker 消息读取测试已有；纯 Rust 多 Partition、取消和背压尚未实现 |
| ListOffsets | 读取路径内部的 `fetch_watermarks`、`offsets_for_times` | 使用 librdkafka 查询 Partition 首尾 Offset，并把时间转换为起始/结束 Offset | `cmake-build`；需要可访问目标 Topic/Partition | 未实现 | 代码和历史 Docker Offset/时间读取测试已有；没有独立的纯 Rust 请求实现 |
| Consumer Group | `KafkaDriver::list_consumer_groups` | `fetch_group_list`、`committed_offsets` 和有界 `ConsumerProtocolAssignment` 解码 | `cmake-build`；需要 Broker 对 Group 查询和 Offset 查询授权 | 未实现 | 领域、解码器和历史 Docker 组成员/Offset/Lag 测试已有；纯 Rust Group/Offset 请求未实现 |
| Metrics Snapshot | `KafkaMonitoringDriver::metrics_snapshot`、`KafkaService::metrics_snapshot` | `RdkafkaTransport::metrics_snapshot_blocking` 复用只读 Metadata、Topic/Partition 末尾 Offset 和 Consumer Group/Offset 查询；领域层按连续 `high watermark` 样本计算 Topic/集群速率 | `cmake-build`；需要可访问元数据、Topic 末尾 Offset、Consumer Group 和 Offset；不读取消息正文、不提交业务 Offset | 未实现 | Domain/App/UI 测试和 native feature 编译通过；当前没有本次 Docker Broker 复核 |
| Broker Runtime Metrics | `KafkaBrokerMetricsDriver::broker_metrics_snapshot`、`KafkaService::broker_metrics_snapshot` | `PrometheusBrokerMetricsDriver` 通过 HTTP/HTTPS 读取固定 Prometheus/OpenMetrics 指标，解析 `broker_id`、数值和样本时间；不与协议快照合并 | 可选 exporter 文本端点；5 秒超时、4 MiB 响应上限；JMX 需要部署侧 exporter | 本机 HTTP fixture 已验证；真实 Kafka exporter 未完成 | Domain/App/Infra/UI 定向测试、本机 `nginx:1.27-alpine` fixture 和 `docker_kafka_reads_broker_metrics_fixture` 通过；真实 exporter、端点权限/断开请求和真实 Windows 截图仍未完成 |
| Topic | `KafkaAdminDriver::create_topic`、`delete_topic`、`increase_topic_partitions`；读取侧复用 Metadata | `AdminClient`、`NewTopic`、`NewPartitions` 和 Kafka Admin API | `cmake-build`；变更还需要 `ReadWrite` 配置和 Broker 权限 | 未实现 | 代码、输入校验和历史 Docker 管理测试已有；纯 Rust Topic Admin 请求未实现 |
| Config | `KafkaAdminDriver::describe_configs`、`update_config` | 读取使用 `describe_configs`；修改通过显式 `IncrementalAlterConfigs` FFI，静态/只读项在发送前拒绝 | `cmake-build`；修改需要 `ReadWrite` 配置、动态配置支持和 Broker 权限 | 未实现 | 代码和配置项校验测试已有；当前 Docker fixture 未覆盖静态配置拒绝、动态配置回读和纯 Rust 实现 |
| ACL | `KafkaAdminDriver::list_acls`、`create_acl`、`delete_acl` | 通过 librdkafka Admin FFI 映射 ACL 查询、创建和精确删除；错误转换不保留凭据或消息正文 | `cmake-build`；需要 Broker Authorizer 和对应 ACL 权限 | 未实现 | 代码和领域输入校验已有；当前 Docker fixture 未启用 Authorizer，不能把 ACL 真实服务验收写成通过 |
| TLS | `KafkaSecurityProtocol::Ssl/SaslSsl`、`KafkaTlsConfig` | `ssl.ca.location`、客户端证书/密钥路径和 `TlsVerify` 转换为 librdkafka 属性 | `kafka-tls`；需要证书链、Broker TLS listener 和正确主机名 | 未实现 | feature gate 和属性映射测试通过；没有 TLS Broker fixture、证书轮换或错误证书服务证据 |
| SASL | `KafkaSecurityProtocol::SaslPlaintext/SaslSsl`、`KafkaSaslMechanism` | 支持 `PLAIN`、SCRAM、GSSAPI、OAUTHBEARER 枚举映射；用户名/密码由配置校验后传给 native 客户端 | `kafka-sasl`；需要匹配的 Broker 认证配置。GSSAPI/OAUTHBEARER 还需要当前模型未覆盖的外部认证参数 | 未实现 | feature gate、机制映射和敏感字段脱敏测试通过；没有 SASL Broker fixture、认证成功或拒绝服务证据 |

### 跨能力约束

| 约束 | 当前实现 | 评审结论 |
|---|---|---|
| 超时 | `RdkafkaDriver::with_request_timeout` 校验 1 毫秒到 60 秒预算；native 客户端使用同一请求预算 | 代码级可用；需要在不同 Broker 版本和慢请求场景补服务测试 |
| 取消 | 应用层以 `smol::unblock` 承载阻塞客户端；读取查询有时间和数量上限 | 读取范围有界，但 native 请求的主动取消和连接回收还没有独立能力接口 |
| 错误 | native 错误统一映射为 `KafkaErrorCategory`，重试属性只对网络和超时开放 | 已有代码和单元测试；纯 Rust 错误分类尚无实现 |
| 敏感数据 | 客户端配置使用 allowlist；密码、证书路径和密钥字段不进入普通 Debug 输出或安全错误正文 | 当前边界应保留到后续适配器，不允许开放任意属性 Map |
| 业务消费进度 | 浏览客户端关闭自动提交和自动 Offset 存储，并使用临时 group ID | native 读取路径已有代码和服务测试；纯 Rust 客户端必须保持相同语义 |

## 证据状态

| 证据 | 结果 | 说明 |
|---|---|---|
| 默认 feature 静态测试 | 已通过 | 本次 Windows MSVC 目标下 `ramag-infra-kafka` 默认 feature 7 项通过；此前 Windows GNU 基线为 5 项；覆盖默认不启用 native、TLS/SASL feature gate、输入校验和错误分类 |
| 阶段 21-22 Domain/App/UI 测试 | 部分通过 | 阶段 21 的 Windows MSVC 证据为 `ramag-domain` 159 项、`ramag-app` 191 项、`ramag-infra-kafka` 默认 feature 7 项、`ramag-tool-kafka` 20 项；阶段 22 的 `ramag-tool-kafka` Windows GNU 测试 22 项通过，覆盖 Broker 健康摘要、快照来源/时间状态和 360/900/1440 宽度布局；当前 MSVC 复测受 Windows SDK 缺少 `msvcrt.lib` 阻塞 |
| native metrics 编译和基础测试 | 已通过 | Windows MSVC 下 `ramag-infra-kafka --features cmake-build` 使用 CMake/NMake 构建 `rdkafka-sys`，`cargo check` 通过，native 基础测试 10 项通过；未连接真实 Broker |
| Cargo feature 关系 | 已核对 | `ramag-infra-kafka` 默认 feature 为空，`ramag-bin` 显式启用 `cmake-build`；`cargo tree --locked -p ramag-bin -e features` 可看到 `rdkafka-sys` 和 CMake 路径 |
| plain KRaft Docker 集成 | 已通过 | 2026-09-09 在 WSL Docker 中运行 `scripts/kafka-test/kafka-test.ps1 test`：先创建并校验 5000 条消息和 61 个主题，再运行 `crates/ramag-infra-kafka/tests/docker_kafka.rs`，元数据、消息读取/搜索、消费者组/Offset 和 Topic 管理 4 项测试全部通过；测试脚本只使用专用 KRaft 容器和测试卷 |
| TLS/SASL Broker 集成 | 未完成 | 当前 fixture 没有 TLS listener、证书链、SASL 用户或 Authorizer 配置 |
| 纯 Rust 客户端 | 未开始 | 工作区没有候选依赖、协议实现或等价服务测试，不能据 native 结果推断纯 Rust 兼容 |
| Linux/macOS 默认构建 | 未在本次 Windows 工作区执行 | 需要对应工具链和独立构建日志；不能用 Windows GNU 结果替代 |

## KAFKA-001 结论和下一步

本矩阵确认了三个直接结论：

1. 当前 native 路径已经覆盖明文 Kafka 的主要读取和管理能力，但它仍依赖 `librdkafka` 构建链；`ramag-bin` 的显式 `cmake-build` 使“默认桌面构建完全不依赖 CMake”尚未成立。
2. TLS、SASL 和 ACL 目前只有代码级支持和 feature gate 证据，缺少对应 Broker fixture；不能把它们写成真实服务验收通过。
3. 纯 Rust 路径尚未开始，不能直接替换 native 客户端，也不能从现有 native 测试推断协议兼容。

阶段 18 的文档交付完成；其服务验收仍有明确未完成项。阶段 19 已增加 `KafkaTransport` 适配边界：领域层提供稳定的能力快照接口，`KafkaService` 只暴露能力结果，现有 `RdkafkaTransport` 位于基础设施层并保留 `RdkafkaDriver` 兼容别名；不在该项中伪造纯 Rust 实现或改变现有 Kafka 用户流程。

阶段 21 已增加 `KafkaMonitoringDriver` 和协议指标快照链路：快照包含来源、时间、状态、错误原因、集群/Topic/Partition/Consumer Group 指标和按连续 `high watermark` 样本计算的速率；应用层和 UI 均不依赖 `rdkafka` 类型。

阶段 22 已在 Kafka 概览页整合 Broker 健康摘要、Topic/Partition 健康和消费者组 Lag。Broker 健康只根据 Metadata API 返回结果和协议指标快照中的 Broker 数判断，并明确标出外部 Broker 运行指标尚未接入；明文 KRaft Docker Broker 已在 2026-09-09 完成基础集成复核，真实 Windows 截图仍未完成。

阶段 23 已增加独立的 `KafkaBrokerMetricsDriver` 和 `PrometheusBrokerMetricsDriver`：配置页保存可选指标端点，适配器只读取四个固定指标名，应用层和 UI 分别保留外部运行指标的来源、采样时间、状态和错误。该适配器不直接连接 JMX，也不把配置 ID 作为 Kafka 集群 ID；真实 exporter 和 Broker 运行指标请求仍需单独复核。

相关代码和测试：

- [`KafkaDriver`](../crates/ramag-domain/src/traits/kafka_driver.rs)
- [`KafkaAdminDriver`](../crates/ramag-domain/src/traits/kafka_driver.rs)
- [`KafkaMonitoringDriver`](../crates/ramag-domain/src/traits/kafka_driver.rs)
- [`RdkafkaDriver`](../crates/ramag-infra-kafka/src/lib.rs)
- [`KafkaMetricsSnapshot`](../crates/ramag-domain/src/entities/kafka_metrics.rs)
- [`ramag-infra-kafka` 默认测试](../crates/ramag-infra-kafka/src/tests.rs)
- [`Docker Kafka 集成测试`](../crates/ramag-infra-kafka/tests/docker_kafka.rs)
- [`Kafka 测试脚本`](../scripts/kafka-test/kafka-test.ps1)
