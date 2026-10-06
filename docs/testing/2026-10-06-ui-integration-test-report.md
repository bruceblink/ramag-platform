# 2026-10-06 本机完整测试与 UI 功能测试

## 范围与验收

本次按用户要求验证整个 Rust workspace、现有本地 Docker 集成，以及真实 Windows 窗口的第一方工具流程。主线是 UI 对齐与功能修复；排序表头覆盖系统进程、Docker 四类资源、SQL 结果、MongoDB 结果、对象存储账号和挂载。设置卡片采用纵向布局。原 `02C` 后置键盘导航不再作为当前任务排期，历史归档保留。

验收分别记录自动测试、Docker 实际交互和 Computer Use 操作。调试程序不代表正式发布性能、跨平台或硬件精度验收。没有安全测试账号的流程保持未完成。

验证命令使用 `scripts/windows/invoke-cargo-msvc.ps1` 的原生 Windows MSVC 环境：`cargo test-all`、`cargo fmt --all -- --check`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、目标库测试、`cargo build --locked -p ramag-bin --target-dir target/ui-environmental-reason-verify`、`scripts/build-windows.ps1 -Release`、`scripts/windows/check-source-size.ps1` 与 `git diff --check`。回滚边界为本轮共享表头及调用处、Windows 网络默认路由适配、进程详情字段、Kafka 窄窗口消息结果区、API 草稿保存、调试数据目录开关、Docker/SSH 测试脚本和本文记录；不回滚用户数据或已有功能。

## 环境与数据隔离

- Windows x86_64，Rust 1.98.1，Docker Desktop Linux engine 29.7.2。
- 实际源码程序：`target/ui-environmental-reason-verify/x86_64-pc-windows-msvc/debug/ramag.exe`。
- 调试 UI 的连接/配置存储由 `RAMAG_UI_TEST_DATA_DIR=F:\project\ramag-platform\.ramag\ui-acceptance\data` 选择（实现要求绝对路径），系统偏好由 `RAMAG_SYSTEM_STATE_DIR=F:\project\ramag-platform\.ramag\ui-acceptance\system` 选择。它们仅设置于测试程序进程，没有复制用户连接或凭据。Release 不启用连接存储目录开关。
- VCS 使用 `.ramag/ui-acceptance/repo` 专用仓库和测试文本。密码只从已有忽略文件进入进程和遮蔽输入框，不写入报告或日志。
- 原生截图窗口为 1202×812；此前本轮还操作过用户存储下的监控和设置窗口，但未保存连接或修改配置。下表新建连接、保存和查询均使用隔离存储。

## 自动测试

| 检查 | 结果与范围 |
|---|---|
| workspace `test-all` | 最终源码在四个本地数据库与 `RAMAG_TEST_DATASET=full` 下全量运行，退出 0；所有输出的测试摘要均通过；未汇总精确总数，不据此虚构覆盖率 |
| Redis 大数据扫描 | 重测前检查发现 DB0=45,014、TTL 测试键已过期、DB15=0；从专用测试数据重新导入后 DB0=46,014、TTL 测试键=1,000、DB15=0，完整 `test-all` 通过 |
| 最终包库回归 | 10 个受影响 Rust 库：1,065 通过、0 失败、1 忽略 |
| Windows 默认路由 | 最终源码 4 通过、0 失败；覆盖有效度量、IPv4 优先/IPv6 回退、未知度量保留、诊断去重/上限和当前接口映射 |
| 对象存储库 | 最终源码 35 通过、0 失败，包含挂载排序与选中身份保持 |
| 格式、严格 Clippy | `cargo fmt --all -- --check` 与 workspace Clippy `-D warnings` 通过 |
| 源码尺寸、差异 | `scripts/windows/check-source-size.ps1` 与 `git diff --check` 通过 |
| 最终 Windows 调试构建 | `ramag-bin` 构建成功；产物位于 `target/ui-environmental-reason-verify/x86_64-pc-windows-msvc/debug/ramag.exe`，218,829,312 bytes |
| 正式 Windows Release 构建 | 正在运行；结果待本次构建结束后补记 |

DBClient 十万行 SQL 表通过真实表头点击完成升序、降序和清除排序。自动测试调用的交互覆盖范围及 MongoDB 原生窗口排序仍须分别记录，不由这次 SQL 交互替代。尝试增加的 SQL/MongoDB 确定性点击测试因后台工作线程与 GPUI 测试调度器不兼容而移除；没有把失败尝试计为通过，现有排序规则和渲染测试继续保留。

## Docker 集成

| 服务 | 镜像与本机端口 | 结果及清理 |
|---|---|---|
| MySQL | `mysql:8.4`，13306；另有原专用可视测试容器 13318 | running/healthy；十万行大表与类型/大值/空间数据已导入，复用测试卷并保留运行 |
| PostgreSQL | `postgres:17-alpine`，15432 | running/healthy；十万行大表与类型/大值/时间序列数据已导入，保留运行 |
| Redis | `redis:7-alpine`，16379 | running/healthy；专用 DB0/DB15 扫描通过，保留运行 |
| MongoDB | `mongo:8.2`，27018 | running/healthy；125,102 个测试文档，保留运行 |
| HTTP/TLS/代理 | 本地 `python:3.12.11-alpine3.22` 测试镜像，18089/18091/18093 | 真实 HTTP 集成 1 通过；测试容器保留运行 |
| gRPC/TLS/代理 | 本地 `rust:1.91.0-bookworm` 测试镜像，18090/18092/18094 | 真实 gRPC 集成 1 通过；测试容器保留运行 |
| MQTT | `eclipse-mosquitto:2.0.20`，18883 | 真实集成 2 通过；running/healthy，保留运行 |
| Kafka/Connect | `apache/kafka:4.0.0`，19092/18083 | 12 个 rdkafka 与 1 个纯 Rust 集成通过；5,000 条测试消息，Broker 目录含 68 个 Topic，保留运行 |
| Schema Registry/ksqlDB | `confluentinc/cp-schema-registry:8.3.1` / `cp-ksqldb-server:8.3.1`，18081/18088 | 随 Kafka 测试通过，running/healthy，保留运行 |
| Kafka 指标 | `nginx:1.27-alpine` / 本地 JMX exporter 1.6.0，19100/19101 | 随 Kafka 测试通过，保留运行 |
| RustFS | `rustfs/rustfs:latest`，19000/19001 | 本地健康接口可用；当前 COS/OSS 官方 Endpoint 校验无法接入，不能计为对象存储应用集成通过；保留专用测试卷 |
| SSH/SFTP | 本地 `debian:12-slim` / OpenSSH 9.2p1、Python 3.11.2 测试镜像，12222 | Git OpenSSH 隔离自检和 `true` 通过；最终脚本 `--package ramag-infra-ssh` 只运行目标集成；terminal/port-forward 与 SFTP 两项通过，容器、网络、密钥和临时 trust store 清理 |

仅访问本机 Docker；没有使用远程、开发或生产服务。导入用的容器临时 SQL/JSON 文件已删除，数据库测试卷没有删除。

## 真实窗口功能记录

| 工具/页面 | 操作与实际观察 | 结果边界 |
|---|---|---|
| Shell/首页 | 12 个注册工具入口、侧栏导航、页面标题 | 通过；完整拖拽重排和所有焦点场景未复验 |
| 全局设置 | Appearance、Sampling、Window behavior 纵向布局；浅色切深色并显示“已保存” | 通过；使用隔离配置 |
| 系统监控 | 本轮先前逐页操作 Summary/CPU/Memory/GPU/Disks/Network/Energy/Thermals/Processes；新构建 Summary 默认网络显示“以太网” | 实际设备显示通过，硬件采集精度/权限提升不据此通过 |
| Processes | PID 升序/降序，选中当前测试程序，七列表格及 CPU/内存/读写/Threads/User 详情 | 通过；Threads 显示后端明确不可用原因，没有结束真实用户进程 |
| DBClient/MySQL | 新建本地连接、测试成功、保存、打开工作区，`bulk_records` 十万行表分页，ID 升序 1 起、降序 100000 起、第三次清除排序 | 通过；只读查询，未写用户数据 |
| API | 最终调试构建加载隔离工作区（1 个集合、1 个请求）；向本机 HTTP Docker fixture 请求 `/json` 得到 HTTP 200，保存后仍保留集合/请求并显示“请求、环境和断言已保存” | 通过真实窗口验证请求、保存回写与计数 |
| Kafka | 最终调试构建连接本机 Broker；在 1202×812 下读取 `ramag.integration.messages` 返回 200 条，向下滚动可见消息行与分页栏 | 通过真实窗口验证窄窗口结果区尺寸、页面滚动和读取 |
| MQTT | 新建测试配置、连接和保存 `127.0.0.1:18883`；开始通配符订阅，从本机测试容器发布一条确定性消息，收到后停止 | 通过，时间线显示测试 topic/payload/连接状态 |
| VCS | 打开隔离 `.ramag/ui-acceptance/repo`；选中 `sample.txt` 查看实际 diff，暂存后取消暂存 | 通过；无提交、推送或用户仓库改动 |
| SSH UI | 打开连接表单并取消 | 仅验证表单和取消；未配置新 trust store 或保存真实 SSH 连接 |
| JSON Path | 在本机示例 JSON 上提取 `$.users[*].name` 得到 Alice/Bob；非法 `$[` 显示语法失败 | 通过正常和错误流程 |
| 本机协作 | 输入隔离草稿、保存并确认本机草稿条目出现 | 本地保存通过；未上传 Relay 或发送数据 |
| 对象存储 | 页面可访问；未保存 Cloud COS/OSS 测试账号 | 对象存储自动排序测试通过；云账号、Bucket、对象和传输不适用本机无凭据环境 |
| Container | 最终调试构建连接本机 named pipe，显示 Docker Engine 29.7.2、17 个运行容器和 29 个镜像；镜像大小升序从 12.2 MiB 开始、降序从 2.0 GiB 开始，活动排序方向可见 | 通过真实窗口验证共享排序表头与双向排序；资源读取只读 |
| 设置 | 最终调试构建显示 Appearance、Sampling、Window behavior、Presets 纵向排列 | 通过真实窗口验证纵向卡片和滚动可达 |
| Processes | 最终调试构建按 PID 升序显示；筛选并选中当前 Ramag 测试进程，CPU、内存、读写、Threads 与 User 详情均可见 | 通过真实窗口验证表格排序和详情；Threads 明确显示当前后端不提供线程集，没有结束真实用户进程 |
| 剪贴板 | 未在窗口中读取真实历史，也未调整或复制用户剪贴板内容 | 自动化覆盖不替代本机剪贴板权限和用户历史数据验收 |

## SSH 清理事故与恢复状态

旧脚本的 `finally` 在 `up` 命令未创建备份时仍调用恢复逻辑，误删了用户 `~/.ssh/known_hosts`（原 828 字节）。这是本轮测试操作造成的真实数据损失，不能用新的测试通过代替恢复。没有将文件内容或文件指纹写入本报告。

本机未找到匹配副本；`known_hosts.old` 仅 92 字节、内容不同，保持原样。已通知用户并请求备份路径，当前原文件仍未恢复。后续需要匹配的备份或经用户确认重新核验主机指纹；不能静默信任或拼接旧副本。

脚本现完全移除对用户 known_hosts 的写入和恢复逻辑，明确选择 Git for Windows OpenSSH，在唯一临时 HOME 写测试主机公钥。Cargo 前用 `ssh -G` 验证路径，失败立即停止；退出恢复原进程环境，递归删除前检查绝对路径位于专用 Temp 子目录。`status` 回归及最终失败/成功测试清理核对用户文件状态没有再次变化。

## 尚未完成或不适用

- 对象存储账号、Bucket、对象与传输真实流程：缺少专用 COS/OSS 测试账号和可写测试前缀；RustFS 不满足当前服务商校验。
- macOS/Linux 原生窗口、真实 Kubernetes 集群：本机环境不适用。
- 发布构建 300 活动帧/p95、跨 DPI/多显示器、系统认证和正式签名包：本次调试功能验收不覆盖。
- 用户 SSH 原始信任文件恢复：等待匹配备份。
