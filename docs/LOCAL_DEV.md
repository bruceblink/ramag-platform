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
