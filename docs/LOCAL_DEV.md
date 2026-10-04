# 本地开发验收

## Kafka Broker 运行指标

本地端到端验收只使用仓库提供的 Docker Compose fixture，不连接远程集群或真实业务账号。

```powershell
pwsh -File scripts/kafka-test/kafka-test.ps1 -Command up
pwsh -File scripts/kafka-test/kafka-test.ps1 -Command test -MessageCount 5000
pwsh -File scripts/kafka-test/kafka-test.ps1 -Command status
```

测试会启动以下服务：

| 服务 | 镜像 | 本机端口 | 用途 |
|---|---|---|---|
| `ramag-kafka-test` | `apache/kafka:4.0.0` | `127.0.0.1:19092` | KRaft Broker 和 JMX |
| `ramag-kafka-jmx-exporter-test` | 本地构建的 JMX Exporter `1.6.0` | `127.0.0.1:19101/metrics` | 受 Basic Auth 保护的 Broker 指标 |
| `ramag-kafka-metrics-test` | `nginx:1.27-alpine` | `127.0.0.1:19100/metrics` | 无认证 OpenMetrics 静态 fixture |
| `ramag-kafka-connect-test` | `apache/kafka:4.0.0` | `127.0.0.1:18083` | Kafka Connect 共享 fixture |

`docker_kafka_reads_real_broker_jmx_exporter` 验证无认证请求返回 `401`，输入 Basic Auth 后返回 `200`，并解析 Kafka CPU、内存、磁盘和请求延迟指标。测试 exporter 的用户名和密码只用于本机 fixture；生产配置应通过 Secret 注入并使用 HTTPS，不能复用测试凭据。

测试结束后保留服务供后续本地检查；停止服务使用：

```powershell
pwsh -File scripts/kafka-test/kafka-test.ps1 -Command down
```

连同专用 Docker 数据卷清理使用：

```powershell
pwsh -File scripts/kafka-test/kafka-test.ps1 -Command clean
```

`clean` 会删除专用容器、网络和数据卷，不要把它用于其他 Compose 项目。

## Docker 测试服务故障恢复

本机 Docker 集成测试入口在启动前会检查自己管理的测试容器。发现容器处于
`exited`、`dead`、`created`、`restarting`、`paused` 或 `unhealthy` 状态时，
脚本会先停止并移除该测试实例，再由 Compose 创建新实例。启动或健康检查失败
时也会执行一次同样的清理并重试，避免复用已经不可用的容器。

该行为已接入数据库、HTTP、gRPC、MQTT、SSH、Kafka 和协作中继测试入口。故障
恢复只操作脚本声明的专用容器，不删除数据卷；数据库凭据和 Kafka 数据仍按各自
脚本的 `down`/`clean` 策略处理。
