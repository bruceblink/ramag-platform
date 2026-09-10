# Kafka 工作台功能矩阵

> 文档状态：`KAFKA-023` 与阶段 27 的当前功能矩阵和验收顺序
> 更新日期：2026-09-11
> 适用基线：`dev`

本文只描述 Ramag Kafka 工作台与 AKHQ、Offset Explorer 的能力对照和本仓库的交付证据。产品参考用于说明用户场景，不表示复制对方的权限模型、部署方式或全部实现细节。

## 术语表与命名约定

| 规范名称 | English / Acronym | 本文职责边界 | 不代表什么 |
|---|---|---|---|
| Kafka 集群 | Kafka Cluster | 用户保存的一组 Broker 接入配置及其运行上下文 | 不代表某一个 Broker 或本地配置文件本身 |
| Topic | Topic | Kafka 消息的逻辑分类及其 Partition 集合 | 不代表数据库表或消费者组 |
| Partition | Partition | Topic 内有序、独立的消息日志；消息定位和读取的最小服务端范围 | 不代表跨 Partition 的全局顺序 |
| 消息浏览上下文 | Message Browse Context | UI 当前选定的 Topic、Partition、Offset/时间范围和查询预算 | 不代表已向 Broker 发起读取或已消费业务消息 |
| 消费者组 | Consumer Group | Kafka 维护的成员、分配和已提交 Offset 集合 | 不代表 AKHQ/Ramag 的 UI 用户角色组 |
| 功能矩阵 | Feature Matrix | 记录用户能力、代码状态、证据和下一项切片的对照表 | 不代表一次提交必须覆盖整张表 |
| 真实窗口证据 | Native Window Evidence | 在 Windows 原生窗口中完成可复核操作和截图 | 不代表 headless GPUI 测试或 Docker 服务结果 |

## 状态说明

| 状态 | 含义 |
|---|---|
| 已完成 | 代码、风险匹配测试和必要的真实服务证据已达到当前切片要求；未完成的外部或原生窗口证据会单独列出 |
| 进行中 | 已确定责任边界，正在实现一个独立可验收切片 |
| 待排期 | 用户场景已确认，但当前切片尚未开始 |
| 不纳入当前范围 | 需要独立部署、安全或资源模型，不随核心 Kafka 工作台顺带实现 |

## 能力矩阵

| 能力域 | 用户场景 | 当前状态 | 当前代码/证据 | 下一项或限制 |
|---|---|---|---|---|
| 集群与 Broker | 保存多个集群、测试连接、查看 Broker/Controller/版本 | 已完成 | `KafkaClusterConfig`、Metadata 查询、连接测试和概览 headless 测试 | 真实 Windows Kafka 窗口证据仍待补充 |
| Topic 与 Partition | 搜索 Topic、查看 Partition/Leader/ISR/首尾 Offset、创建/删除/扩容 Topic | 已完成 | Topic 管理驱动、Topic 页响应式测试、本机 Docker KRaft 管理集成 | 完整 AKHQ/Offset Explorer 功能对照继续维护 |
| Topic 到消息定位 | 从 Topic 详情的指定 Partition 进入消息页 | 已完成 | `kafka_topic_partition_browse_preserves_message_context`；提交 `d9915ec` | 真实窗口鼠标操作仍待补充 |
| 消费者组到消息定位 | 从消费者组已提交 Offset 进入对应 Topic/Partition/Offset | 已完成 | `kafka_consumer_group_offset_browse_preserves_message_context`；只更新查询上下文 | 不自动读取；用户仍需点击“读取” |
| 消息浏览 | 按 Offset 或时间读取有限范围消息，查看 Key/Value/Headers/Metadata | 已完成 | `KafkaMessageQuery`、有界读取、消息详情和分页 headless 测试 | 不提交业务 Consumer Group Offset |
| 消息搜索与导出 | 在 Key/Value/Headers 中搜索，取消扫描，解码并导出有限消息 | 已完成 | 搜索字段选择、取消、预算校验、JSON/Base64 导出路径 | 正则搜索和批量导入另行排期 |
| 实时消息流 | 选择 Topic/Partition 后 Tail，暂停、停止、过滤和有限窗口 | 已完成 | `KafkaMessageTailRequest`、取消/背压/窗口边界和 UI 状态 | 不替代业务消费者；真实窗口证据仍待补充 |
| 单条消息生产 | 管理模式下确认后生产一条 UTF-8 消息并展示 Broker 定位 | 已完成 | 阶段 25 Domain/App/Infra/UI、Docker 生产回读和 31 项 Kafka UI 测试 | 批量生产、重放和事务编排不纳入当前范围 |
| 消费者组管理 | 查看成员、分配、提交 Offset、Lag；管理模式下重置到最早/末尾 | 已完成 | 消费者组快照、Offset reset Docker 回读和 headless 测试 | 组内 Offset 到消息定位由当前切片补齐 |
| Topic/Broker 配置 | 读取配置、修改支持动态变更的配置并拒绝静态项 | 已完成 | `KafkaConfigResource`、动态配置读改写和只读保护测试 | 配置批量导入另行排期 |
| Kafka ACL | 按 Principal/Host/Resource/Operation 查询，精确创建和删除 | 已完成 | ACL 过滤、二次确认、权限错误映射和 UI 测试 | 不实现 AKHQ UI Groups/Roles |
| 协议指标 | 查看 Broker 元数据、Topic/Partition 健康、Lag 和 high watermark 速率 | 已完成 | `KafkaMonitoringDriver`、`KafkaMetricsSnapshot` 和概览指标测试 | 运行指标必须与外部来源分开 |
| Broker 运行指标 | 展示 CPU、内存、磁盘、JVM 和请求延迟 | 进行中 | `PrometheusBrokerMetricsDriver`、有界解析和本机 Docker HTTP fixture 已验证 | 真实 Kafka exporter、真实 Broker 运行指标端点和真实窗口证据待补充 |
| Schema Registry | 浏览 Subject、版本和 Schema 内容 | 已完成 | Domain/App/Infra 版本列表与详情边界、Subject 选择和版本滚动 headless 测试、本机 Docker Registry 真实 REST 回读 | 真实 Windows 窗口证据仍待补充 |
| Kafka Connect | 浏览连接器和 Task 状态 | 已完成 | 只读 Connect HTTP 浏览、端点和数量边界测试 | 写操作不纳入当前范围 |
| ksqlDB | 对 Kafka 流执行有界只读查询 | 已完成 | `KafkaKsqlDbDriver`、ksqlDB 配置与查询 UI、HTTP 流式 JSON 解析、GPUI headless 测试和本机 Docker ksqlDB fixture；覆盖成功、HTTP 404、行数上限和取消 | 真实 Windows 窗口证据仍待补充；不执行 DDL、写入或查询管理命令 |
| 纯 Rust Transport | 用跨平台纯 Rust 客户端替换 native backend | 待排期 | 能力矩阵已记录当前 native 路径和缺口 | 不在 UI 功能矩阵切片中顺带替换 |

## `KAFKA-023` 顺序

1. 已完成：Topic 详情的 Partition 到消息页定位，提交 `d9915ec`。
2. 已完成：消费者组已提交 Offset 到消息页定位；起始 Offset 保留，用户显式点击“读取”后才访问 Broker。
3. 已完成：概览页 Partition 健康到消息页定位；headless 测试在 360/900/1200 宽度下确认入口布局，只更新 Topic、Partition 和消息查询上下文，不自动读取或提交 Offset；真实窗口证据仍待补充。

## 阶段 23 下一切片设计：本机 OpenMetrics HTTP Fixture

本切片只验证 Ramag 到外部指标 HTTP 端点的真实本机请求链路，不宣称已经接入 Kafka 生产环境 exporter。Docker Compose 增加独立 `metrics` 服务：镜像固定为 `nginx:1.27-alpine`，容器内 `/metrics` 返回受版本控制的 OpenMetrics 文本，宿主绑定 `127.0.0.1:19100`。Rust 集成测试通过 `PrometheusBrokerMetricsDriver` 请求 `http://127.0.0.1:19100/metrics`，检查 `ExternalBrokerMetrics` 来源、Broker ID、四个已知指标和样本时间；Kafka Broker、Kafka Connect 与指标 fixture 分别保留独立服务状态。

静态 fixture 不代表 JMX、真实 Kafka exporter 或生产 Broker 运行指标。真实 exporter 容器和 Windows 原生窗口证据继续单独排期；本机 Docker 不可用时，HTTP 集成验收标记为未完成。

## 统一验收规则

- 消息上下文切换只能更新 `KafkaView` 的查询输入、页面和取消代次，不得自动推进业务 Consumer Group Offset。
- 读取、搜索、Tail、生产、消费者组管理和指标刷新必须保持独立的请求状态、错误展示和资源预算。
- 读写操作必须保留只读保护、精确确认和结构化错误；失败不能伪装成功。
- GPUI headless 测试、Docker Kafka/KRaft 真实服务测试和真实 Windows 窗口证据分别记录；一种证据不能替代另一种证据。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` 和 `git diff --check` 通过后，才能提交并推送当前切片。
