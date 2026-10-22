# 连接型工作区完整程序验收计划

> 切片：`A-POST-0.5-CONNECTION-ACCEPTANCE-001`
> 日期：2026-10-22
> 状态：API 工作区读取竞态已修复并验证；下一项清理 SSH 测试凭据，再继续 API、Kafka 和 MQTT 的逐工具验收
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

| 顺序 | 子切片 | 进入条件 | 交付条件 |
|---|---|---|---|
| 1 | `A-POST-0.5-CONNECTION-API-STATE-001` API 工作区读取竞态修复 | 已完成；40 项 API 测试、严格 Clippy、格式、源码尺寸和差异检查通过 | `send`、`run_collection` 在 `Loading`/`Failed` 内部拒绝执行；历史读取失败仍允许执行；延迟读取和失败读取回归通过；未改变 HTTP/gRPC 协议行为 |
| 2 | `A-POST-0.5-CONNECTION-SSH-CLEANUP-001` SSH 测试材料清理 | 找到上轮专用 `%TEMP%` 目录并确认不属于用户 SSH 路径 | 删除私钥、公钥、批处理文件和隔离信任文件；运行脚本状态回归；记录清理成功或明确阻塞；不重建用户 `known_hosts` |
| 3 | `A-POST-0.5-CONNECTION-API-ACCEPTANCE-002` API 页面和双分隔栏验收 | 子切片 1 完成；子切片 2 清理闭环 | 在隔离数据下验证 HTTP/gRPC 发送、保存、失败重试、取消、历史、响应上下文和两条分隔栏；有 Computer Use 时补真实拖动，否则保留“headless 已通过、原生拖动未验证” |
| 4 | `A-POST-0.5-CONNECTION-KAFKA-001` Kafka 工作区验收 | API 子切片收口；本机 Kafka/Connect/Schema Registry 容器健康 | 验证连接失败恢复、Topic/分区浏览、消息读取/分页、Consumer Group、Schema/指标状态和取消；只使用本机 Docker，记录镜像、端口、测试数据和清理 |
| 5 | `A-POST-0.5-CONNECTION-MQTT-001` MQTT 工作区验收 | Kafka 子切片收口；本机 Mosquitto 健康 | 验证配置保存、连接/断开、订阅、发布、停止、格式查看和有界消息时间线；覆盖失败恢复和大载荷边界；只使用本机 Docker |
| 6 | `A-POST-0.5-REGRESSION-001` 连接型工作区回归收口 | API、Kafka、MQTT 和 SSH 证据均已记录 | 完整程序构建；复核首页往返、主题、Settings、工具入口和本轮受影响工作区；更新本计划、主计划和测试报告；未完成的原生窗口证据继续单独列出 |

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

## API 工作区读取状态修复记录

- 实现：`send` 和 `run_collection` 在操作方法内部调用工作区状态门禁；工具栏同步禁用按钮，避免用户误触。
- 状态：`Loading` 和 `Failed` 显示可操作提示并保持编辑内容；`HistoryFailed` 允许执行，因为工作区身份已经加载。
- 目标测试：`request_execution_waits_for_delayed_workspace_load_and_history_failure_is_recoverable`；完整 `ramag-tool-api --lib` 测试共 40 项通过。
- 质量检查：目标 Clippy、`cargo fmt --all -- --check`、`check-source-size.ps1` 和 `git diff --check` 通过。

- 改动范围：API 请求列表与编辑区之间的分隔栏，以及请求编辑区与响应区之间的分隔栏均可左右拖动。请求列表宽度为 180–420px，并在窄桌面窗口中动态收窄，以保留编辑区最小 520px 宽度。请求编辑区最小宽度为 240px；响应区最小宽度为 220px。小于桌面布局断点时仍使用纵向堆叠布局。
- Headless 交互：`api_sidebar_divider_resizes_and_clamps_to_available_width` 和 `api_request_response_divider_resizes_and_preserves_minimum_widths` 使用 GPUI 模拟鼠标输入检查拖动、动态边界和最小宽度。第二条测试的鼠标点位必须落在窗口可见区域内。
- 本机 Docker 集成：`view::docker_tests::api_workbench_sends_http_and_grpc_requests_to_local_docker_services` 已在本机 Docker 服务监听时通过。HTTP 测试镜像为 `ramag-api-http-test:python-3.12.11-alpine-3.22`，端口为 `127.0.0.1:18089->8080`、`127.0.0.1:18091->8443`。gRPC 测试镜像为 `ramag-api-grpc-test:rust-1.91.0-bookworm`，端口为 `127.0.0.1:18090->50051`、`127.0.0.1:18092->50052`。代理服务端口为 `127.0.0.1:18094->8081`。
- Docker 清理：为测试启动的 HTTP 测试容器已停止并恢复为初始退出状态。gRPC 测试容器和代理在测试前已运行，测试后仍保持健康，未更改其状态。
- 真实窗口：Computer Use 检测到窗口有新的用户输入，且窗口尺寸发生变化。因此本轮没有完成拖动操作验证。Headless 通过不代表真实窗口拖动通过。
- System 工具测试：用户已确认全部通过，本轮未重复运行。

## API 工作区布局子切片验证命令

```powershell
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-api','--lib','api_sidebar_divider_resizes_and_clamps_to_available_width','--','--test-threads=1')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-api','--lib','api_request_response_divider_resizes_and_preserves_minimum_widths','--','--test-threads=1')
```

## 回滚边界

只回滚本计划中对应子切片的 SSH/SFTP UI 修复、API 双分隔栏实现、目标测试和验收记录。不更改用户 SSH 配置、`known_hosts`、凭据、远端主机、已有 Docker 容器或其它连接工作区。
