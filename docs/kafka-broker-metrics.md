# Kafka Broker 运行指标接入说明

> 文档状态：已实现 Prometheus/OpenMetrics 文本端点适配，并增加本机 Docker HTTP fixture 验收；JMX 和真实 Kafka exporter 仍需要部署侧暴露文本端点
> 更新日期：2026-09-11
> 适用分支：`dev`；长期同步分支只保留 `main`、`dev`

## 术语与命名规则

| 名称 | English / Acronym | 负责范围 | 不表示什么 |
|---|---|---|---|
| Kafka 协议指标 | Kafka Protocol Metrics | 由 Kafka Metadata、Offset 和 Consumer Group API 返回的集群、Topic、Partition 和 Lag 数据 | 不表示 Broker 的 CPU、内存、磁盘或请求延迟 |
| Broker 运行指标 | Broker Runtime Metrics | 由外部 Prometheus/OpenMetrics 文本端点提供的 Broker 进程运行数据 | 不表示 Kafka Admin API 已经返回这些数值 |
| 指标端点 | Metrics Endpoint | 每个 Kafka 配置使用的 HTTP/HTTPS GET 地址 | 不表示 Ramag 会自动发现、启动或修改 exporter |
| `broker_id` | `broker_id` | 将每条已知指标样本关联到 Kafka Broker 的非负整数标签 | 不表示端点返回的实例名、主机名或 Ramag 配置 ID |

## 接入范围

Kafka 配置页增加一个可选的 `Prometheus / exporter` 指标端点。保存后，概览页刷新任务同时读取两类来源：

- `Kafka Protocol API` 继续提供协议快照、Topic/Partition 健康和消费者组 Lag。
- `ExternalBrokerMetrics` 通过配置的 HTTP/HTTPS 端点提供 Broker CPU、内存、磁盘和请求延迟。
- 两类快照分别保存、校验、记录错误和渲染；一类来源失败不会覆盖另一类来源的成功结果。
- 端点返回的 `cluster_id` 不会被推断为 Kafka 集群 ID。当前适配器只把实际解析到的 Broker 运行指标返回给对应的配置页面。

当前不直接连接 JMX。需要 JMX 时，应由部署侧 exporter 把数据转换为下列 Prometheus/OpenMetrics 文本指标，再把文本端点填入 Kafka 配置。

## 文本接口约定

端点使用 HTTP GET，响应必须是 UTF-8 的 Prometheus/OpenMetrics exposition 文本。适配器只消费下列四个完整指标名，其他指标会忽略：

| 指标名 | 单位 | 必需标签 | 允许值 |
|---|---|---|---|
| `ramag_kafka_broker_cpu_usage_percent` | 百分比 | `broker_id` | `0` 到 `100` |
| `ramag_kafka_broker_memory_used_bytes` | bytes | `broker_id` | 非负有限数 |
| `ramag_kafka_broker_disk_used_bytes` | bytes | `broker_id` | 非负有限数 |
| `ramag_kafka_broker_request_latency_ms` | milliseconds | `broker_id` | 非负有限数 |

示例：

```text
# HELP ramag_kafka_broker_cpu_usage_percent Broker CPU usage
ramag_kafka_broker_cpu_usage_percent{broker_id="0"} 18.5 1725600000000
ramag_kafka_broker_memory_used_bytes{broker_id="0"} 1073741824
ramag_kafka_broker_disk_used_bytes{broker_id="0"} 4294967296
ramag_kafka_broker_request_latency_ms{broker_id="0"} 2.25
```

样本时间戳可以省略；存在时支持 Unix 秒或 Unix 毫秒，页面显示解析后的 UTC 时间。一个 Broker 缺少四类指标中的任意一类时，快照状态为 `Partial`，缺少的字段显示为未知，不转换为零。没有任何已知 Broker 样本时，状态为 `NoData`。

## 失败状态与资源边界

- 未填写端点：状态为 `SourceNotConfigured`，页面显示“未配置数据源”。
- HTTP `401` 或 `403`：状态为 `PermissionDenied`，页面保留权限错误。
- 连接失败、超时、非成功 HTTP 状态、响应过大或响应不是 UTF-8：状态为 `CollectionFailed`。
- 单次请求超时为 5 秒；不跟随重定向，不读取系统代理；响应正文最多读取 4 MiB。
- 响应中已知指标必须带 `broker_id`，Broker ID 必须非负且不能重复；CPU、内存、磁盘和延迟数值必须有限且非负。
- 应用层会再次校验来源、快照数量、Broker ID 和数值范围，基础设施适配器不能绕过这些限制。
- 日志只记录操作名、配置 ID、耗时、结果数量和安全错误分类，不记录端点正文、认证信息或消息内容。

## 验证范围

- `ramag-domain`：配置端点边界、敏感字段脱敏、外部快照来源和数值范围。
- `ramag-app`：外部快照注入、应用边界校验以及与 Kafka 协议快照的来源隔离。
- `ramag-infra-kafka`：已知指标解析、样本时间、部分数据、无关指标、缺少 `broker_id` 和响应边界。
- `ramag-tool-kafka`：概览页外部指标区域、状态选择器以及 360/900/1440 宽度布局。
- `scripts/kafka-test/compose.yaml` 的 `metrics` 服务使用 `nginx:1.27-alpine` 在 `127.0.0.1:19100/metrics` 提供固定 OpenMetrics fixture；`docker_kafka_reads_broker_metrics_fixture` 通过真实 HTTP 请求验证适配器。该服务只证明 HTTP 接入链路，不代表真实 Kafka exporter 或生产 Broker 运行指标。
- 真实 exporter 容器、真实 Broker 运行指标端点和 Windows 原生窗口证据仍未完成；需要部署侧端点后再执行真实请求复核。
