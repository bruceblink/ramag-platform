# Ramag Platform 主线开发计划

> 状态：阶段 0 文档与基线收敛完成；阶段 1 的插件平台 P0-A、P0-B、PLAT-003 已完成；TERM-001 代码和真实 SSH 端点验收已完成
> 更新日期：2026-09-08
> 适用范围：插件平台、数据库工作台、Kafka 工作台、SSH/终端工作台以及跨工具质量与构建流程
> 分支策略：`main` 是稳定基线，`dev` 是集成分支，每个独立任务使用一个短期 `feat/<task-id>-<name>` 分支
> 当前交付切片：`UI-001`，逐项核验全软件响应式布局；数据库和 Kafka 的功能/UI 对齐按下方队列推进

## 术语与命名规则

| 名称 | English / Acronym | 本计划中的职责 | 不表示什么 |
|---|---|---|---|
| 主线 | Mainline | 规定四条产品线和质量工作的共同顺序、依赖和交付规则 | 不替代 Kafka、数据库或插件专项文档中的协议细节 |
| 产品线 | Product Track | 一组共享产品目标、代码边界和验收条件的连续任务 | 不表示必须长期使用一个开发分支 |
| 交付切片 | Delivery Slice | 能独立修改、测试、提交、推送和回滚的最小功能 | 不表示一次覆盖多个不相关模块 |
| 内置插件 | Built-in Plugin | 编译进主程序、通过 Rust 接口注册的现有工具 | 不表示已经支持第三方动态插件 |
| 传输边界 | Transport Boundary | 隔离 Kafka 具体客户端实现的接口和适配层 | 不表示 Kafka UI 或领域模型可以直接依赖客户端类型 |
| UI 证据 | UI Evidence | headless 边界测试、真实窗口操作、截图或可复核日志 | 不表示只通过编译或单元测试 |
| 真实服务证据 | Live Service Evidence | 使用实际数据库、Kafka、OpenSSH 或容器服务得到的结果 | 不表示模拟对象或静态 fixture 已覆盖生产行为 |

专项文档负责领域细节，本文件负责跨工具排期和当前任务。插件平台见 [`plugin-platform-roadmap.md`](plugin-platform-roadmap.md)，Kafka 见 [`kafka-tool-roadmap.md`](kafka-tool-roadmap.md)，数据库见 [`database-client-datagrip-roadmap.md`](database-client-datagrip-roadmap.md)，架构边界见 [`architecture.md`](architecture.md)。专项文档不得重新定义本文件的跨工具优先级；发现状态冲突时，先修正实现状态和证据，再开始下一项任务。

## 1. 目标和当前基线

Ramag Platform 是一个 Rust 2024 Cargo workspace，把数据库、Kafka、Git、SSH/SFTP、对象存储、剪贴板和系统监控组织在同一个原生桌面应用中。现有工具已形成可运行的编译期内置能力，下一阶段的重点是让新增工具有稳定的平台入口，并让每条产品线都保持明确的协议、状态和安全边界。

当前基线：

- `ramag-domain` 保存实体、错误和跨层接口，不依赖 GPUI、数据库驱动或 Kafka 客户端类型。
- `ramag-app` 负责用例编排、任务取消、上下文隔离和服务组合，不负责 UI 布局。
- `ramag-infra-*` 负责数据库、Kafka、SSH、Git、存储和系统适配。
- `ramag-tool-*` 负责具体工作台的交互；`ramag-terminal` 负责通用 PTY、ANSI 状态和终端绘制。
- `ramag-bin` 当前仍直接装配内置工具；插件平台先包装这条路径，不复制业务状态。
- Kafka 当前仍使用 `rdkafka/librdkafka` 基础设施，纯 Rust Transport、实时 Tail 和 Metrics Snapshot 尚未实现。
- 真实 Windows 窗口证据仍与 headless 验证分开记录；未取得窗口证据的项目不能写成真实窗口已验收。

主线目标：

1. 新增工具通过稳定的内置插件适配器进入平台，不再把注册细节散落在窗口入口。
2. SSH/终端、Kafka 和数据库各自保持独立的连接、任务、错误和安全语义。
3. Windows、Linux 和 macOS 使用同一套 Cargo 日常命令；平台差异只留在工具链准备和发布包装层。
4. 每个交付切片都有与风险匹配的测试、UI 或真实服务证据，并留下可回滚的独立提交。

## 2. 产品线状态

| 产品线 | 当前状态 | 下一项工作 | 暂不扩大的范围 |
|---|---|---|---|
| 插件平台 | P0-A、P0-B、PLAT-003 已完成，现有工具通过静态插件宿主注册并按生命周期管理 | 保持 P0-C 设置与权限接口待实现；后续按队列推进 `TERM-001` | 动态 ABI、插件市场、第三方不受信任代码 |
| SSH/终端 | `alacritty_terminal + GPUI` PTY 核心、SSH/SFTP 工作区、会话状态、每标签重连和 `-L/-R/-D` 参数模型已有；Windows OpenSSH 客户端访问 WSL OpenSSH 端点的真实验证已完成 | 补真实 Windows 窗口证据和独立转发状态/停止面板；进入 `KAFKA-001` | 在终端核心内加入 SSH、RDP、VNC、Telnet 或 Serial 协议 |
| Kafka 工作台 | 集群、Topic、消息、ACL、配置和消费者组基础能力已有；阶段 18 已形成传输能力矩阵 | 完成 `KafkaTransport` 适配边界后，按 AKHQ/Offset Explorer 能力表推进对象树、消息浏览、Consumer Group、配置和 ACL 的功能/UI 对齐 | 在适配边界和 Transport 决策前实现外部生态大模块 |
| 数据库工作台 | SQL、Redis、MongoDB 查询、结果、事务和迁移基础能力已有 | 按 DBeaver/DataGrip 能力表推进结果查看、大字段恢复、对象导航、执行计划和迁移工作流的功能/UI 对齐 | 把 Redis/MongoDB 强行套用 SQL 语义 |
| 质量与工具链 | stable channel、统一 Cargo 命令、Windows GNU 路线已建立 | 保持 CI、WSL Linux 验证、源码尺寸和 LF 规则一致 | 为单个平台恢复独立的日常编译命令 |

跨产品的 UI 响应性问题不再单独生成一条长期大路线。出现新的可复现 P0/P1 问题时，按下面的交付切片规则插入当前队列，并在对应专项文档记录实现细节。

## 3. 当前交付队列

同一时间只允许一个任务处于“开发中”。表中顺序是当前主线顺序；“待开始”任务不能提前扩展实现。

| ID | 产品线 | 负责人范围 | 状态 | 依赖 | 验收重点 |
|---|---|---|---|---|---|
| `PLAT-001` | 插件平台 | `ramag-domain`、`ramag-app` | 已完成 | 无 | `PluginId`、API 版本、能力集合、设置模式和重复注册校验有单元测试；现有工具顺序和入口不变 |
| `PLAT-002` | 插件平台 | `ramag-app`、`ramag-bin` | 已完成 | `PLAT-001` | 静态插件注册、初始化、失败隔离、逆序关闭和迟到调用拒绝有测试 |
| `PLAT-003` | 插件平台 | `ramag-ui`、`ramag-app` | 已完成 | `PLAT-002` | 插件状态、注册错误和可用入口在 360/1024/1440 headless 窗口内可见 |
| `TERM-001` | SSH/终端 | `ramag-domain`、`ramag-infra-ssh`、`ramag-tool-ssh` | 已完成（真实端点已验证） | `PLAT-003` | 会话状态、重连和 `-L/-R/-D` 参数模型有 OpenSSH 参数测试；Windows OpenSSH 客户端访问 WSL OpenSSH 端点已覆盖 Shell、SFTP、三类转发、停止、重连和错误 Host Key |
| `KAFKA-001` | Kafka | `ramag-domain`、`ramag-app`、`ramag-infra-kafka`、构建维护 | 阶段 19 代码完成，Windows GNU Release 构建已验证 | `PLAT-003` | 阶段 18 能力矩阵已记录；`KafkaTransport` 适配边界、能力快照和 native 命名已落地，保持当前用户流程 |
| `DB-001` | 数据库 | `ramag-app`、`ramag-tool-dbclient` | 待开始 | `PLAT-003` | 结果查看模式、大字段限制、编辑失败恢复和连接上下文隔离有测试 |
| `UI-001` | 跨工具 UI | `ramag-ui`、各 `ramag-tool-*` | 开发中 | `PLAT-003` | 共享弹窗、工具栏、列表和详情区在 360/1024/1440 headless 窗口内换行、滚动且不越界；真实窗口证据单独记录 |
| `DB-002` | 数据库 | `ramag-tool-dbclient`、`ramag-ui` | 待开始 | `DB-001` | 建立 DBeaver/DataGrip 功能矩阵，逐项实现并验收结果、对象导航、执行计划和迁移 UI，不以静态截图宣称完成 |
| `KAFKA-023` | Kafka | `ramag-tool-kafka`、`ramag-ui` | 待开始 | `KAFKA-001` | 建立 AKHQ/Offset Explorer 功能矩阵，逐项实现并验收 Topic、消息、Consumer Group、配置和 ACL UI |
| `QUALITY-001` | 质量与工具链 | workspace 维护者 | 持续任务 | 每个交付切片 | stable toolchain、统一 Cargo 命令、LF、CI 过滤器和三平台发布证据保持一致 |

`UI-001` 验收记录（2026-09-07）：共享弹窗的实际打开测试发现导入表单在 360×240 窗口中仍宽 414px，左侧越界 27px，说明此前仅调整内容宽度不足以修复 Dialog 外框。当前修复统一约束快捷键、最近项目和导入弹窗的宽度、顶部偏移及内容高度；导入操作区保留在滚动区外。3 项直接打开弹窗的 headless 测试覆盖 360×240、360×640、1024×768、1440×900、打开后缩放、取消、最近项目滚动/搜索/打开和快捷键录制错误/退出。真实窗口验证未完成：本次 Computer Use 的 `list_windows()` 返回空列表，安装的 `@oai/sky` 也没有技能要求的 `documentation` 接口。其他工作台仍须逐项检查，不能由这三类弹窗的结果推断全软件已适配。

`UI-001` 验收记录（2026-09-08）：数据库 SQL 会话的对象树工具栏改用共享响应式工具栏；搜索区允许收缩，筛选按钮和系统库、刷新、编辑器操作保持固定尺寸，空间不足时换行，不再把对象树顶部控件挤出侧栏。`table_tree_toolbar_wraps_inside_sidebar_widths` 覆盖对象树最小侧栏宽度 180px，以及 280/360/1024/1440px 窗口，检查工具栏与每个操作控件的边界和相互重叠；`ramag-tool-dbclient` 共 286 项库测试通过。真实 Windows 窗口截图仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` 验收记录（2026-09-08）：SSH 连接管理列表的连接行允许固定徽标和编辑/删除操作按可用宽度换行，连接名称保留可收缩区域；`connection_manager_rows_stay_inside_supported_window_widths` 覆盖 360/1024/1440px 窗口，检查连接行、JumpServer 图标、环境/系统/认证/生产标记、远程桌面槽和操作组均未越出父行或窗口。`ramag-tool-ssh` 共 76 项库测试通过。真实 Windows 窗口截图和键盘操作仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` 验收记录（2026-09-09）：对象存储账号管理列表的账号行改为可换行布局，服务商、只读、Bucket 数量和编辑/删除区域保持固定尺寸，360px 窗口隐藏非必要的 Bucket 数量列，避免固定列把行推出内容区。`account_rows_stay_inside_supported_window_widths` 覆盖 360/1024/1440px 窗口，检查账号行及各固定区域边界；`ramag-tool-object-storage` 共 24 项库测试通过。真实 Windows 窗口截图和键盘操作仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` 验收记录（2026-09-09）：Kafka 概览的 Topic 预览行为长名称增加可收缩和省略处理，Partition 数量保持固定位置；区块副标题和集群摘要允许在窄内容区换行，避免概览内容错位。主题页按可用高度扩大列表区域，桌面窗口不再只保留约 360px 的列表高度；纵向滚动条使用完整 16px 交互区域，仅在滚动时显示，避免窄条裁切和常驻色块。`kafka_overview_keeps_sections_aligned_without_vertical_gap` 和 `kafka_topics_reflow_header_and_split_at_supported_widths` 覆盖 360/900/1024/1440px 等窗口，Kafka 工作台共 25 项库测试、Clippy 和格式检查通过。真实 Windows 窗口截图和鼠标拖动滚动条仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` 补充记录（2026-09-09）：Kafka 概览按主内容区宽度统一使用 900px 分栏断点，避免外层窗口宽度与侧栏扣除后的实际空间采用不同布局；集群摘要值允许在卡片内收缩。主题列表把 16px 滚动条改为独立右侧槽位，列表内容不再被滚动条覆盖；`kafka_overview_keeps_sections_aligned_without_vertical_gap` 增加 1200px 主内容边界，`kafka_topics_reflow_header_and_split_at_supported_widths` 检查列表内容区与滚动条的相邻关系。真实 Windows 窗口截图和鼠标拖动滚动条仍未完成。

`KAFKA-001` 构建记录（2026-09-09）：使用 `scripts/build-windows.ps1 -Release` 和 stable Windows GNU 工具链完成 `ramag-bin` Release 构建；`fxc.exe` 着色器编译、x64 PE/GUI 子系统检查和依赖检查通过，产物为 `target/x86_64-pc-windows-gnu/release/ramag.exe`，大小 `90036736` 字节。该记录只证明本机 Release 产物和 PE 检查，不替代 Docker Broker、真实 Kafka 服务或真实 Windows Kafka 界面验收；安装文件替换因旧进程仍在运行，待进程退出后复核。

`PLAT-003` 完成后，`TERM-001` 已完成代码和真实 OpenSSH 端点验收；真实 Windows 窗口和独立转发状态面板仍单独排期。`KAFKA-001` 已进入传输能力矩阵和适配边界阶段，在其完成前不实现 Metrics Snapshot 或 Live Message Tail；终端和数据库任务不得借机修改 Kafka 或插件协议。

## 4. 分阶段主线

### 阶段 0：基线与文档收敛（已完成）

完成内容：

- README、架构说明、工具清单和平台关系已对齐当前独立仓库身份。
- Kafka 路线已区分现有管理能力与尚未实现的 Transport、Metrics 和 Tail 能力。
- 插件、Kafka、终端和数据库的职责边界已经写入专项文档；历史执行日志不再作为当前排期依据。
- Windows GNU、Linux、macOS 的日常 Cargo 命令保持一致；平台差异只用于准备环境和发布包装。
- 全局 Git 与 Codex 规则统一使用 LF；仓库跟踪文本文件不接受 CRLF 或混合换行。

阶段 0 的文档变化只在完成检查后提交，不与下一项代码功能合并。

### 阶段 1：内置插件平台（当前）

先完成 `PLAT-001` 至 `PLAT-003`。平台只包装现有工具注册路径，不改变数据库、Kafka、SSH 或其他工具的业务行为。

最低接口边界：

- `PluginId`、插件 API 主次版本、显示元数据和稳定入口 ID。
- 已知能力集合，例如 `ui.entry`、`ui.notification`、`storage.plugin` 和 `task.scoped`。
- 有界设置模式和独立命名空间；敏感值只能通过平台秘密存储访问。
- 注册结果、失败阶段和有界诊断；诊断不得记录密码、Token、完整连接配置或消息正文。
- 内置插件的实例、业务状态和 GPUI 视图不重复创建。

阶段 1 不扫描外部目录、不执行外部代码、不引入动态 ABI。动态插件必须另行完成来源、签名、权限、进程隔离、升级回滚和残留清理评估。

### 阶段 2：SSH/终端工作区

`TERM-001` 已补齐会话状态、退出标签、每标签重连和端口转发模型，并在 Windows OpenSSH 客户端访问 WSL Ubuntu-26.04 OpenSSH 端点的临时环境中完成真实验证。终端继续使用 `alacritty_terminal + GPUI`，只负责 PTY、ANSI、输入输出、选择、剪贴板和资源回收；SSH 认证、Host Key、SFTP 和 JumpServer 留在 `ramag-infra-ssh` 与 `ramag-tool-ssh`。真实 Windows 窗口和独立转发状态/停止面板不属于本次代码交付。

SecureCRT 和 MobaXterm 只作为功能参考，不作为完整复制目标。会话日志、宏、多主机执行和其他协议必须独立建模，并先完成敏感数据、权限和资源上限设计。

### 阶段 3：Kafka Transport 与观测前置条件

先完成 `KAFKA-001` 的能力矩阵和默认构建评估。`ramag-domain`、`ramag-app` 和 `ramag-tool-kafka` 不得直接依赖 `rdkafka` 类型；具体客户端只能位于 Kafka Transport 适配层。

只有传输边界、TLS/SASL、取消、断线恢复和 Windows 构建依赖得到结论后，才决定是否实现 `KafkaMonitoringDriver`、Consumer Group Lag、Metrics Snapshot 和 Live Message Tail。Broker CPU、内存、磁盘、JVM 和请求延迟必须来自明确配置的 JMX、Prometheus 或 exporter 数据源，不能由 Admin API 伪造。

### 阶段 4：数据库连续工作流

按 `DB-001` 及数据库专项路线推进结果查看、大字段、对象导航、执行计划、迁移确认和大数据量边界。SQL、Redis 和 MongoDB 共享连接工作区，但不共享不适用的操作语义。查询代次、取消、重试和失败恢复必须保留连接与标签上下文。

### 阶段 5：可选生态与动态能力评估

阶段 1-4 稳定后才评估动态插件包、Schema Registry、Kafka Connect、ksqlDB、Serial、Telnet、RDP、VNC 和 X11。每项评估都必须有独立用户场景、部署条件、安全边界和停止条件；参考产品具备某能力不构成实现承诺。

## 5. 交付和验证规则

每个交付切片按以下顺序执行：

1. 从最新 `main` 检查 `git status --short --branch`、远端状态和已有变更；不覆盖无关工作区文件。
2. 在对应专项文档写清责任组件、输入、输出、失败行为和最小验收条件。
3. 创建 `feat/<task-id>-<name>` 短期分支，只修改该任务及必要测试、文档和配置。
4. 先运行目标测试，再运行匹配风险的 UI、真实服务或构建验证。
5. 从 workspace 根目录运行 `cargo fmt --all -- --check` 和 `cargo clippy --workspace --all-targets -- -D warnings`；涉及 Rust 源码时补充目标测试、源码尺寸和日志约定检查。
6. 运行 `git diff --check`，确认提交路径、换行、敏感信息和提交说明只对应当前任务。
7. 测试通过后只创建一个独立 Conventional Commit，并立即推送当前分支；未验证或失败的任务不提交。
8. 把提交、测试命令、UI/真实服务证据、环境限制和未完成项写回本计划或对应专项文档，再开始下一项。

## 6. 验收条件

| 风险 | 必须提供的证据 |
|---|---|
| 领域规则和注册校验 | 目标 crate 单元测试、格式检查、workspace Clippy |
| 异步任务、取消和上下文 | 代次、取消、关闭、失败和迟到结果测试 |
| Kafka、数据库、SSH 或外部命令 | 实际服务/端点或 Docker 集成测试，记录版本、条件和跳过原因 |
| GPUI 布局和交互 | 360/1024/1440 边界的 headless 测试；真实窗口截图和操作记录单独记录 |
| Windows/Linux/macOS 构建 | 同一 Cargo 命令、目标工具链、产物或失败原因；不把平台脚本成功等同于二进制成功 |
| 安全和高风险写操作 | 正常、拒绝、超时、取消、二次确认和敏感信息脱敏路径 |
| 文档和配置 | 术语、组件名、箭头方向、链接、LF 换行和 `git diff --check` |

Headless 结果不能描述为真实窗口结果；外部服务未启动时只能记录为未完成或环境限制。发现新的 P0/P1 可见回归时，先暂停当前 UI 切片并更新队列，不把猜测当作缺陷修复范围。

## 7. 文档维护边界

- 本文件只维护跨工具阶段、当前队列、依赖、交付规则和证据状态。
- `architecture.md` 只维护已实现 crate、依赖方向和技术决策，不追加长期功能愿望。
- 专项路线只维护该产品线的领域模型、协议选择、具体任务和专项验收，不复制本文件的全部历史日志。
- README 只描述当前可用能力、统一命令和明确限制，不把未来计划写成已实现功能。
- 已完成任务保留提交 ID 和验证证据即可，不在主线重复堆积逐次执行日志；旧的发布待办和历史公告不作为当前开发计划。

`PLAT-002` 已在提交 `9b98b2e` 完成。`ramag-app` 生命周期专项测试 8 项和 `ramag-bin` 集成测试 14 项通过；GNU/MSYS 环境下 workspace Clippy、格式检查、源码尺寸检查和 `git diff --check` 通过。stable/MSVC 直接构建仍受本机缺少 Windows SDK 库影响，不能把该环境限制写成代码失败。

`PLAT-003` 已完成：`ramag-app` 保留插件描述、生命周期状态和注册/初始化/关闭失败的有界诊断；`ramag-ui` 在设置页展示插件状态、失败阶段、入口 ID 和当前可用入口，Activity Bar 在有故障时为设置入口显示角标。`ramag-ui` 全部 86 项测试、`ramag-app` 生命周期专项 8 项和 `ramag-bin` 集成 14 项通过；其中插件诊断 headless 测试覆盖 360/1024/1440 窗口，验证设置滚动区和诊断区域的边界。GNU/MSYS 环境下 workspace Clippy、格式检查、源码尺寸检查和 `git diff --check` 通过。真实窗口截图和键盘操作仍未完成，不能把 headless 结果描述为真实窗口验收。

`TERM-001` 代码交付已完成：新增 `SshSessionState`、受限的 `SshPortForward` 模型、SSH 命令解析和 `-L/-R/-D` 参数构造；终端状态刷新、退出标签和每标签重连均有 `ramag-tool-ssh` 生命周期测试。`ramag-domain` 152 项、`ramag-infra-ssh` 62 项、`ramag-app` 189 项、`ramag-tool-ssh` 73 项测试通过；目标四个 crate 的 Clippy、格式检查、源码尺寸检查和 `git diff --check` 通过；Windows OpenSSH `ssh -G` 已解析 local、remote、dynamic 三类转发参数。

真实端点验证使用 Windows OpenSSH 9.5p2 客户端和 WSL Ubuntu-26.04 临时 OpenSSH 服务，覆盖 Shell 命令、SFTP `pwd`、`-L`/`-R`/`-D` 监听建立、停止本地转发后监听关闭、强制断开后的重新连接，以及错误 Host Key 被拒绝。临时密钥、授权文件、配置和服务进程均在脚本结束时清理，脚本未纳入仓库。

未完成项：真实 Windows 窗口截图和键盘操作、独立转发状态/停止面板仍未完成；workspace 全量库测试被 `rdkafka-sys` 的 Windows GNU 构建前置条件阻断，错误为缺少 MSYS/MinGW CMake generator，与 TERM-001 源码无关。P0-C 设置与权限接口继续另行排期，不把 headless 结果写成真实窗口验收。
