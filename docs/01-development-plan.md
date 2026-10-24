# Ramag Platform 最新开发计划

> 状态：现行计划
> 更新日期：2026-10-24
> 当前基线：main 与 origin/main 已同步
> 产品定位：原生核心能力运行时与 GPUI 客户端
> 配套产品：[it-tools-template](F:/project/it-tools-template/) 负责 Web/PWA、Electron 产品壳和通用工具工作台

本计划只描述新的执行边界。旧的逐切片记录保存在
[01-development-plan-history.md](01-development-plan-history.md)、专项计划和
[历史归档](archive/2026-09-25-pre-datagrip-rebaseline/)。
历史完成记录不自动变成新的开发范围。

P0 的共同边界记录见
[双项目能力归属矩阵](24-cross-project-capability-matrix.md)。两个仓库的矩阵必须保持同一
能力 ID、所有者、平台范围和回滚条件。

## 1. 决策原则

Ramag 只投入在 it-tools-template 无法低成本、稳定或安全替代的能力。每个新功能必须至少满足一项：

1. 需要原生 PTY、系统级会话或受控本机权限。
2. 需要 Rust 运行时的资源控制、取消、流式处理或跨平台协议实现。
3. 是 Ramag 自身需要的本地安全能力，且需要 GPUI 工作区保持统一保护边界。
4. 能形成稳定的 Ramag 内部原生接口，而不是另一套重复的产品界面。

Electron 和 GPUI 属于两个独立产品。它们只在需要比较的应用场景上记录相同的用户结果；参数模型、权限、错误、取消、分页、进度和资源限制分别由各自项目维护。

## 2. 双项目职责边界

| 能力 | it-tools-template | ramag-platform |
|---|---|---|
| Web/PWA 和 Electron 应用壳 | 负责 | 不负责 |
| 117 个通用工具、纯计算、格式转换和 WASM | 负责 | 只保留必要的纯 Rust 核心或测试样例 |
| 数据库管理器（PostgreSQL、MySQL、SQLite） | 负责 Electron 工作台、连接、分页、编辑和历史 | 移除 GPUI 产品入口；仅在后续证明需要 Rust 引擎时保留可复用服务 |
| Redis、MongoDB、API、Kafka、MQTT | 负责 Electron 工作台和平台适配 | 移除对应 GPUI 工具和专用产品路线 |
| Git、对象存储、容器 | 负责 Electron 工作台和平台适配 | 移除对应 GPUI 工具和专用产品路线 |
| 剪贴板历史、普通系统监控、更新和设置 | 负责 Electron 主进程与产品界面 | 移除重复 UI；只保留必要的原生采集或安全服务 |
| SSH、SFTP、PTY 终端、端口转发 | 当前不纳入 it-tools-template 的统一功能范围 | 作为 Ramag 的独立原生能力保留 |
| 加密凭据库、系统密钥库和本地安全存储 | 由 it-tools-template 独立管理应用配置和凭据 | 由 Ramag 独立管理自己的连接和会话秘密 |
| 高精度硬件采集和低层系统诊断 | 显示结果，按能力声明选择性启用 | 只有在 Electron/Node 不能满足精度和资源边界时保留 |
| GPUI Shell、原生窗口和键盘工作区 | 不负责 | 只承载保留下来的原生核心能力 |

两个仓库完全独立。它们不共享进程、运行时、代码包、接口协议或发布物。
双方只在功能边界和应用场景上互相参照。it-tools-template 可以用自己的
TypeScript、Node 或 Electron 能力实现产品功能；Ramag 使用自己的 Rust 和 GPUI
实现原生工作区。一个项目的实现完成情况不构成另一个项目的运行时依赖。

## 3. 当前项目的拆分结果

### 3.1 保留在 Ramag 的核心

- ramag-domain：只保留原生能力实体、错误、预算、取消和内部接口模型。
- ramag-app：只保留无界面的用例编排、能力目录和生命周期管理。
- ramag-infra-ssh、ramag-terminal、ramag-infra-tunnel：保留 SSH/SFTP、PTY、端口转发和会话安全边界。
- ramag-infra-storage：保留加密本地存储、主密钥和系统凭据库；不得向 UI 暴露任意文件或密钥。
- ramag-ui、ramag-bin：收缩为 GPUI 原生核心客户端，不再装配通用工具目录。
- ramag-infra-system：只在低层硬件或高精度采集有明确需求并通过基准后保留；普通 CPU、内存、磁盘、网络和进程页面迁移到 Electron。

### 3.2 交给 it-tools-template 并从 Ramag 主产品移除

以下功能的产品页面、状态管理、连接配置、驱动适配和发布验收全部转移到
it-tools-template：

- ramag-tool-dbclient、ramag-tool-redis、ramag-tool-mongodb 以及
  ramag-infra-mysql、ramag-infra-postgres、ramag-infra-sqlite、
  ramag-infra-sql-shared、ramag-infra-redis、ramag-infra-mongodb。
- ramag-tool-api、ramag-infra-api。
- ramag-tool-kafka、ramag-infra-kafka。
- ramag-tool-mqtt、ramag-infra-mqtt。
- ramag-tool-vcs、ramag-infra-git。
- ramag-tool-object-storage、ramag-infra-object-storage。
- ramag-tool-container、ramag-infra-container-docker、
  ramag-infra-container-registry。
- ramag-tool-clipboard、ramag-infra-clipboard 的产品页面和采集编排。
- ramag-tool-system 的普通监控页面；ramag-infra-system 仅按 3.1 的条件保留。
- ramag-infra-update 的桌面更新流程；更新由 Electron 分发和安装流程统一负责。
- ramag-tool-json-path、ramag-tool-json-path-wasm 的工具入口和 Web 适配。
  纯计算逻辑可以迁移到 it-tools-template，但不再维护两套入口。
- ramag-tool-collaboration、ramag-infra-collaboration 和 Relay 方向。
  协作不是当前核心能力，暂缓到两个产品的本地数据边界稳定后再评估。

移除是指从 Ramag 的默认构建、工具目录、GPUI 导航和当前路线中移除。删除源码前
必须完成 it-tools-template 的功能矩阵、数据迁移说明和回滚版本。

## 4. 独立产品边界

it-tools-template 和 Ramag 各自维护自己的应用状态、凭据、存储、错误处理、
更新流程和测试。两边可以记录相同的用户场景，例如“连接远程主机”或“查看系统状态”，
但不要求共享参数模型、接口协议或内部实现。

Ramag 的 Web 兼容性不属于当前产品目标。Ramag 的 GPUI 工作区只验证本机原生窗口、
远程会话、终端输入、文件传输和本地安全边界。it-tools-template 继续独立验证
Web/PWA 和 Electron 的平台差异。

## 5. 当前执行队列

| 顺序 | ID | 内容 | 完成条件 |
|---|---|---|---|
| P0 | BOUNDARY-001 | 已建立能力归属矩阵，继续核对两边入口和回滚信息 | 两个仓库的矩阵、所有者、旧入口和回滚版本一致 |
| P1 | TRANSFER-001 | 在 it-tools-template 收口数据库、API、Git、消息、容器、对象存储、剪贴板和普通系统监控 | 每项有 Electron 页面、错误/取消/权限验收和 Web 平台声明 |
| P2 | RAMAG-SCOPE-REDUCE-001 | 从 Ramag 默认构建和导航移除已由 it-tools-template 独立承担的功能 | workspace 依赖收缩、文档链接更新、旧配置可读或有迁移提示 |
| P3 | RAMAG-CORE-001 | 收口 SSH/SFTP/PTY、端口转发、本地安全和必要的低层采集 | GPUI 原生工作区的连接、取消、资源和权限边界明确 |
| P4 | RAMAG-UI-001 | 完成保留核心的 GPUI 工作区验收 | 真实窗口、headless、错误恢复和跨平台状态分别记录 |
| P5 | CORE-RELEASE-001 | 发布收缩后的 Ramag 原生产品版本 | Windows、Linux、macOS 的独立构建、资源和权限证据齐全 |

在 P0 完成前，Ramag 不新增数据库、Git、API、Kafka、MQTT、容器、对象存储、
剪贴板和普通系统监控功能。任何例外必须说明它为什么不能由 it-tools-template
承担，并附基准或安全证据。

## 6. 验收门槛

- Rust：受影响 crate 执行 cargo fmt --all -- --check、
  cargo clippy --workspace --all-targets -- -D warnings、目标测试和
  cargo build --locked -p ramag-bin。
- 原生能力：记录真实系统、连接、取消、错误恢复、关闭和权限拒绝路径。
- 场景对照：两项目只对照用户流程和功能边界，不把一个项目的测试结果写成另一个项目的证据。
- UI：Ramag 的 GPUI 真实窗口和 headless 证据分开记录；不能用静态截图替代交互证据。
- 数据库或消息集成只使用本机 Docker。PostgreSQL 使用 17+，MySQL 使用 8.4+；
  纯协议或 UI 切片明确写“不适用”。
- 文档：每个迁移切片记录所有者、旧入口、替代入口、兼容期、回滚方法和未完成证据。

## 7. 回滚边界

迁移期间先停用旧入口，再删除实现。发现 it-tools-template 的替代能力不满足
数据安全、取消、性能或错误恢复条件时，恢复对应 Ramag 工具注册，不恢复两套长期开发路线。
已发布版本、用户连接、加密存储和凭据不自动删除；删除工具前提供配置读取或迁移提示。
