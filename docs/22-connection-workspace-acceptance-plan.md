# 连接型工作区完整程序验收计划

> 切片：`A-POST-0.5-CONNECTION-ACCEPTANCE-001`
> 日期：2026-10-24
> 最近更新：2026-10-25
> 状态：API 工作区读取竞态、SSH 清理、Kafka 和 MQTT 本机 Docker 回归已完成；API 简约双栏布局、两条分隔栏真实窗口操作和连接型工作区全局 UI 回归均已完成
> 范围：完整程序中的 SSH/SFTP、API、Kafka 和 MQTT 编辑/连接工作区

## 问题证据

`docs/testing/2026-10-06-ui-integration-test-report.md` 只记录 SSH 新建连接表单和取消操作，没有记录当前完整程序中的验证、保存、失败恢复和结果上下文。2026-10-02 已有 WSL SSH 窗口成功连接证据，2026-10-06 已有本机 Docker OpenSSH/SFTP 协议集成证据；这些历史证据不替代本轮的表单与错误流程检查。

## 代码审查结果（2026-10-21）

### P0：API 工作区首次读取期间仍可发送请求

`ApiView::with_service` 在 [`crates/ramag-tool-api/src/view.rs`](../crates/ramag-tool-api/src/view.rs) 第 213 行启动异步工作区读取，读取期间状态为 `Loading`。保存和导入会调用 `block_workspace_write_until_loaded`，但 [`operations.rs`](../crates/ramag-tool-api/src/operations.rs) 第 199 行的 `send` 和第 293 行的 `run_collection` 没有相同的状态门禁；工具栏的发送和 Collection 按钮也只检查执行、保存、导入和 gRPC 发现状态。

如果用户在读取完成前发送请求，历史可能使用初始临时工作区 ID 写入 Storage。迟到的读取回调随后会替换视图中的工作区和历史，导致本次结果从当前工作区消失。该路径目前没有延迟读取与发送并发的回归测试。

修复要求：

1. 发送和运行 Collection 在 `Loading`、`Failed` 状态下拒绝启动，并保留当前编辑内容；`HistoryFailed` 允许执行，因为工作区本体已经可信加载。
2. 状态检查必须位于操作方法内部，按钮禁用只作为交互提示，不能作为唯一保护。
3. 增加“读取延迟 → 点击发送/运行 Collection → 完成读取”的测试，确认不会写入临时工作区、不会丢失响应和历史；增加失败读取、重试后发送的测试。

### P1：SSH 测试凭据清理未闭环

上轮真实窗口验收在 `%TEMP%` 保留了本轮生成的私钥、公钥、SFTP 批处理文件和隔离 `known_hosts`。这些文件不在仓库内，但私钥仍属于敏感测试材料。后续 SSH 验收进入前，必须确认专用目录已删除；删除失败时记录阻塞原因，不继续创建新的测试凭据。脚本必须继续使用唯一临时 HOME 和路径范围检查，不能访问用户的 `known_hosts`、Agent 或配置。

### P2：API 双分隔栏只有 headless 拖动证据

`api_sidebar_divider_resizes_and_clamps_to_available_width` 与 `api_request_response_divider_resizes_and_preserves_minimum_widths` 已通过，API 工作区本机 Docker HTTP/gRPC 请求也已通过。Computer Use 在上轮发生输入竞争，真实窗口拖动尚未验证；headless 结果不能替代该证据。

## 后续实施顺序

同一时间只推进一个子切片。每个子切片完成目标测试、匹配的服务或窗口证据、格式、严格 Clippy、源码尺寸和差异检查后独立提交。

API 状态修复、SSH 清理、Kafka 和 MQTT 验收已完成。对应证据保留在本计划的记录章节中，不再重复进入当前实施顺序。

| 顺序 | 子切片 | 进入条件 | 交付条件 |
|---|---|---|---|
| 1 | `A-POST-0.5-REGRESSION-001` 连接型工作区回归收口 | API、Kafka、MQTT 和 SSH 证据均已记录；API 双分隔栏的真实窗口操作已确认；目标测试和 `ramag-bin` 构建已通过 | 复核首页往返、主题、Settings、工具入口和本轮受影响工作区；更新本计划、主计划和测试报告；未完成的原生窗口证据继续单独列出 |

## 实施范围

- 按 SSH/SFTP、API、Kafka、MQTT 的顺序逐项验收。每个工作区单独完成、记录和交付。
- 当前只验收 SSH/SFTP 表单、连接检查和工作区状态，不修改 SSH/SFTP 协议、认证策略或用户配置。
- 调试程序使用 `%TEMP%` 下的独立 Ramag 数据和系统状态目录。SSH 失败路径使用未监听的本机回环端口；成功路径使用用户指定的本机 WSL Ubuntu 24.04。测试只连接本机 WSL，不连接远程主机。
- SSH 检查将 `HOME` 和 Agent 环境限制在测试进程。成功路径使用临时 ED25519 密钥、临时主机密钥和隔离的主机信任文件；不读取或写入用户 SSH 密钥、Agent、配置、`authorized_keys` 或 `known_hosts`。
- 临时 SSH 服务只绑定本机 WSL 地址的 22222 端口。现有 WSL 22 端口服务不变。验收后已停止临时服务、删除 WSL `/tmp` 服务目录，并将本轮启动的 Ubuntu 发行版恢复为停止状态。
- 本轮没有修改 SSH/SFTP 协议。按用户指定，使用本机 WSL 完成 SSH/SFTP 成功路径，不启动或修改 Docker 资源。只有后续切片改变协议行为时，才使用本机 Docker，并记录镜像、端口、健康状态、启动方式和清理结果。
- 本轮不重复运行用户已确认通过的 System 工具测试。

## 验收条件

- 从当前 `main` 构建完整 `ramag.exe`，在隔离配置中启动，并从真实 Windows 窗口进入 SSH/SFTP 工作区。
- 检查页面标题、连接列表、空状态、表单滚动、字段可读性、取消、输入校验和连接失败反馈。
- 使用关闭的本机回环端口触发连接失败；错误应清楚显示，表单输入应保留，重试控件应恢复可用。随后使用 WSL 临时 SSH 服务验证密钥认证、Terminal 和 SFTP 成功状态。
- 只把测试连接写入隔离数据目录。连接使用本轮生成的临时密钥，不使用密码或用户密钥。保存后确认测试连接出现在隔离列表中。
- SSH/SFTP 目标测试、完整程序构建、格式、严格 workspace Clippy、源码尺寸和差异检查通过。
- 结果记录必须区分当前真实窗口、headless 和既有 Docker 协议证据；未运行的步骤标记为不适用或待后续切片。

## SSH/SFTP 子切片验收记录

- 真实窗口：完整调试程序在 1202×812 Windows 窗口运行，连接列表空状态、创建表单、必填校验、取消和保存结果均已检查。保存后，隔离列表显示 1 个 WSL 测试连接。
- 失败恢复：先连接未监听的 `127.0.0.1:1`。窗口显示诊断失败，已输入字段保留，测试按钮和保存操作恢复可用。
- WSL 成功路径：Ubuntu 24.04 中的 OpenSSH `9.6p1` 使用临时配置在 WSL 本机地址的 22222 端口运行。Windows 到该端口的 TCP 检查通过；Git OpenSSH 的临时密钥认证和只读 SFTP `pwd` 批处理通过。
- Ramag 窗口连接测试显示 OpenSSH、认证、命令执行、Terminal、标准 SFTP 和诊断均可用；远端平台为 Linux，Shell 和路径格式均识别为 POSIX。
- 隔离边界：私钥和应用数据位于 `%TEMP%`。WSL 临时主机密钥、授权公钥、配置和日志位于本轮专用 `/tmp` 目录。用户的 SSH 文件、WSL 22 端口服务和 Docker 资源未修改。
- 清理限制：Windows 自动安全检查拒绝删除本轮 `%TEMP%` 下的临时密钥目录。该目录仍包含本轮生成的 SSH 私钥、公钥和 SFTP 测试批处理文件；它不在仓库内，也没有被写入日志或用户 SSH 目录。隔离 HOME 中的临时主机信任文件也仍存在，但配置已恢复为不读取该文件。
- 后续清理：2026-10-23 复核专用目录 `ramag-ssh-wsl-acceptance-4f286cf5bee64d5685a16b6aca9c32b6` 为空，确认路径位于 `%TEMP%` 且目录名匹配专用前缀后删除；未访问用户 SSH 目录。Docker 恢复后运行 `ssh-test.ps1 -Command status` 成功，并运行 `ssh-test.ps1 -Command test`；Debian 12 OpenSSH/SFTP 容器健康，2 项集成测试通过，测试容器和网络已删除，`ramag-ssh-test-*` 临时目录数量为 0。
- System 工具测试：用户确认已全部通过；本轮未重复运行。
- Docker：不适用。本轮没有改变协议行为，并按用户要求使用 WSL 完成真实连接。
- 目标测试：`ramag-tool-ssh --lib` 90 项通过。完整构建和 workspace 检查在本计划交付前执行。

## 当前 SSH/SFTP 子切片验证命令

```powershell
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-ssh','--lib','--','--test-threads=1')
cargo fmt --all -- --check
& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked','--workspace','--all-targets')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand build -CargoOptions @('--locked','-p','ramag-bin')
& .\scripts\windows\check-source-size.ps1
git diff --check
```

## API 工作区双分隔栏子切片验收记录

- 2026-10-25 真实窗口尝试：从当前工作区重新构建 Debug 程序，并使用 `RAMAG_UI_TEST_DATA_DIR` 与 `RAMAG_SYSTEM_STATE_DIR` 启动隔离实例。API 页面已在 1200×774 窗口中显示，两条分隔栏均可见。
- 2026-10-25 简约风格窗口验收：API 编辑器改为参考 HTTPie 的工作台布局。请求配置使用 Params、Headers、Auth、Body、Advanced 标签；请求编辑区和响应区使用双栏布局；空工作区不再重复显示说明，空响应只保留图标和“尚未发送”；滚动条改为滚动时显示。使用隔离数据启动 Debug 程序后，Computer Use 已确认深色真实窗口中的页面布局。
- 代码验证：`cargo fmt --all -- --check`、`git diff --check`、`cargo test --locked -p ramag-tool-api --lib -- --test-threads=1`、Windows MSVC workspace Clippy、`ramag-bin` 构建和源码尺寸检查均通过。API 测试 39 项通过、1 项因需要本机 Docker 服务而按设计忽略。
- 拖动结果：Computer Use 在真实窗口中先将侧栏分隔栏向右拖动约 60px，侧栏边界随之移动；再将请求/响应分隔栏向右拖动约 60px，请求区和响应区边界随之移动。两次拖动后的窗口截图都显示了实际布局变化。随后重启隔离实例，默认布局恢复。原生拖动验收通过。
- Headless 回归：`api_sidebar_divider_resizes_and_clamps_to_available_width` 和 `api_request_response_divider_resizes_and_preserves_minimum_widths` 均在 Windows MSVC 目标下通过，各 1 项。Headless 结果不替代真实窗口拖动证据。
- Docker UI 集成：HTTP 测试镜像 `ramag-api-http-test:python-3.12.11-alpine-3.22` 在 `127.0.0.1:18089` 健康；预先运行的 gRPC 镜像 `ramag-api-grpc-test:rust-1.91.0-bookworm` 在 `127.0.0.1:18090` 健康。`api_workbench_sends_http_and_grpc_requests_to_local_docker_services` 实际发送 HTTP 与 gRPC 请求，1 项通过。HTTP proxy `127.0.0.1:18093` 也随 HTTP fixture 启动并健康。
- Docker 清理：运行 `scripts/api-test/api-test.ps1 -Command down`，移除了本轮 HTTP fixture、proxy 容器和 Compose 网络。测试前已运行的 gRPC 服务与 proxy（`127.0.0.1:18090`、`127.0.0.1:18094`）未停止或重建，继续保持运行。
- 测试报告修正：完整 `ramag-tool-api --lib` 曾显示 40 项通过，但该 Docker UI 测试在端口缺失时会直接返回，不能据此证明网络路径。此测试现标为默认忽略；显式运行时端口缺失会失败。普通测试结果不再把未执行的 Docker 路径计为通过。

## Kafka 工作区 Docker 验收记录

- 测试环境：本机 Docker Compose 项目 `ramag-kafka-test`；`apache/kafka:4.0.0`（`127.0.0.1:19092`）、Kafka Connect（`127.0.0.1:18083`）、Schema Registry（`127.0.0.1:18081`）、ksqlDB（`127.0.0.1:18088`）、OpenMetrics fixture（`127.0.0.1:19100`）和 JMX exporter（`127.0.0.1:19101`）。
- 测试数据：`ramag.integration.messages` 写入 5,000 条确定性消息；另建 60 个 UI Topic，准备 ksqlDB 流和两个 Schema Registry 版本。
- 验收结果：`scripts/kafka-test/kafka-test.ps1 -Command test` 通过；`docker_kafka.rs` 12 项、`docker_kafka_pure_rust.rs` 1 项全部通过。
- 覆盖范围：Broker 元数据、Topic 管理、消息读取/搜索、生产回读、Consumer Group/Offset 重置、Kafka Connect、Schema Registry、ksqlDB、OpenMetrics 和 JMX 指标。
- 清理结果：执行 `scripts/kafka-test/kafka-test.ps1 -Command clean`；专用容器、网络和 `ramag-kafka-test-data` 卷均已删除，`status` 无残留服务。

## MQTT 工作区 Docker 验收记录

- 测试环境：本机 Docker `eclipse-mosquitto:2.0.20`，地址 `127.0.0.1:18883`。
- 验收结果：`scripts/mqtt-test/mqtt-test.ps1 -Command test` 通过；`docker_mqtt` 2 项测试全部通过。
- 覆盖范围：两种 MQTT 协议消息投递、背压重试和空闲订阅停止。
- 清理结果：执行 `scripts/mqtt-test/mqtt-test.ps1 -Command clean`；专用容器和网络已删除，无命名数据卷残留。

## 连接型工作区回归记录

- 目标测试：`ramag-tool-api --lib` 40 项、`ramag-tool-ssh --lib` 90 项、`ramag-tool-kafka --lib` 38 项、`ramag-infra-mqtt --lib` 2 项全部通过；`ramag-ui --lib` 全局 UI 测试 145 项通过。
- 构建：2026-10-24 执行 `cargo build --locked -p ramag-bin` 通过。
- 质量门：提交前置检查完成 workspace 格式和严格 Clippy；源码尺寸和差异检查通过。
- 当前状态：2026-10-24 已通过 Computer Use 进入完整程序的 API 页面，确认请求列表、编辑区、响应区和两条分隔栏均可见；同日完成首页真实窗口、深色/浅色主题切换恢复、全局 Settings 系统页和数据库客户端页导航。2026-10-25 在隔离 Debug 窗口中完成两条 API 分隔栏的真实拖动并确认边界变化；Headless 拖动测试也通过。
- 2026-10-25 全局 UI 回归初次续查：从 API 页进入首页成功，首页真实窗口显示 10 个工具并包含 API、数据库、Kafka、MQTT、SSH 等入口。该次进入 API 时检测到输入冲突，重置后的窗口截图与无障碍标题不一致，因此当时未确认 API、Settings 和数据库入口；后续用新隔离窗口复核，结果见下一条。
- 2026-10-25 全局 UI 回归收口：使用 `target/debug/ramag.exe` 和隔离数据目录启动 Debug 程序。真实窗口显示首页 10 个工具，可在首页与 API、数据库、Kafka、MQTT、SSH 页面之间导航；全局 Settings 系统页可打开；主题从浅色切换到深色后恢复为浅色。数据库页显示 0 个数据源，Kafka 页显示 0 个本地配置，MQTT 页显示未保存的空配置表单，SSH 页显示 0 个连接。未创建连接、保存配置、发送请求或使用凭据。各页面导航均由真实窗口截图确认；页面刚切换时可能短暂显示上一帧，后续截图与窗口标题及无障碍树一致。
- 隔离边界：Ramag 数据库、系统监控状态和应用设置指向 `%TEMP%` 下的独立目录。启动日志仍写入默认 Ramag 日志位置。没有读取或修改已有连接、凭据或业务数据。

## API 工作区读取状态修复记录

- 实现：`send` 和 `run_collection` 在操作方法内部调用工作区状态门禁；工具栏同步禁用按钮，避免用户误触。
- 状态：`Loading` 和 `Failed` 显示可操作提示并保持编辑内容；`HistoryFailed` 允许执行，因为工作区身份已经加载。
- 目标测试：`request_execution_waits_for_delayed_workspace_load_and_history_failure_is_recoverable`；完整 `ramag-tool-api --lib` 测试共 40 项通过。
- 质量检查：目标 Clippy、`cargo fmt --all -- --check`、`check-source-size.ps1` 和 `git diff --check` 通过。

- 改动范围：API 请求列表与编辑区之间的分隔栏，以及请求编辑区与响应区之间的分隔栏均可左右拖动。请求列表宽度为 180–420px，并在窄桌面窗口中动态收窄，以保留编辑区最小 520px 宽度。请求编辑区最小宽度为 240px；响应区最小宽度为 220px。小于桌面布局断点时仍使用纵向堆叠布局。
- Headless 交互：`api_sidebar_divider_resizes_and_clamps_to_available_width` 和 `api_request_response_divider_resizes_and_preserves_minimum_widths` 使用 GPUI 模拟鼠标输入检查拖动、动态边界和最小宽度。第二条测试的鼠标点位必须落在窗口可见区域内。
- 本机 Docker 集成：`view::docker_tests::api_workbench_sends_http_and_grpc_requests_to_local_docker_services` 已在本机 Docker 服务监听时通过。HTTP 测试镜像为 `ramag-api-http-test:python-3.12.11-alpine-3.22`，端口为 `127.0.0.1:18089->8080`、`127.0.0.1:18091->8443`。gRPC 测试镜像为 `ramag-api-grpc-test:rust-1.91.0-bookworm`，端口为 `127.0.0.1:18090->50051`、`127.0.0.1:18092->50052`。代理服务端口为 `127.0.0.1:18094->8081`。
- Docker 清理：为测试启动的 HTTP 测试容器已停止并恢复为初始退出状态。gRPC 测试容器和代理在测试前已运行，测试后仍保持健康，未更改其状态。
- 真实窗口：早期拖动尝试因窗口输入竞争而中断。2026-10-25 后续在隔离 Debug 窗口中分别拖动两条分隔栏，并通过拖动前后的窗口截图确认两处布局边界均有变化。Headless 结果与原生交互证据分别记录。
- System 工具测试：用户已确认全部通过，本轮未重复运行。

## API 工作区布局与 Docker UI 集成验证命令

```powershell
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-api','--lib','api_sidebar_divider_resizes_and_clamps_to_available_width','--','--test-threads=1')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-api','--lib','api_request_response_divider_resizes_and_preserves_minimum_widths','--','--test-threads=1')

# Docker UI 集成需要本机 HTTP 和 gRPC fixture 都已启动。
& .\scripts\api-test\api-test.ps1 -Command up
& .\scripts\api-test\grpc-test.ps1 -Command up
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-api','--lib','api_workbench_sends_http_and_grpc_requests_to_local_docker_services','--','--ignored','--test-threads=1')
```

## 回滚边界

只回滚本计划中对应子切片的 SSH/SFTP UI 修复、API 双分隔栏实现、目标测试和验收记录。不更改用户 SSH 配置、`known_hosts`、凭据、远端主机、已有 Docker 容器或其它连接工作区。
