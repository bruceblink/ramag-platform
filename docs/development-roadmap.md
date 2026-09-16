# Ramag Platform 主线开发计划

> 状态：阶段 0 文档与基线收敛完成；阶段 1 的插件平台 P0-A、P0-B、PLAT-003 已完成；TERM-001 代码和真实 SSH 端点验收已完成；Kafka 阶段 25 单条消息生产代码、headless UI、本机 Docker KRaft 验收和纯 Rust 读取候选阶段性验证已完成
> 更新日期：2026-09-11
> 适用范围：插件平台、数据库工作台、Kafka 工作台、SSH/终端工作台以及跨工具质量与构建流程
> 分支策略：`main` 是稳定基线，`dev` 是集成分支，每个独立任务使用一个短期 `feat/<task-id>-<name>` 分支
> 当前交付切片：`UI-001`，先完成跨工具响应式布局、真实窗口截图和交互验证；Kafka/ksqlDB 新功能暂缓，待 UI 队列稳定后再恢复

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
- Kafka 默认用户流程仍使用 `rdkafka/librdkafka` 基础设施；`KafkaTransport` 适配边界、实时 Tail、Metrics Snapshot、单条消息生产、本机静态 OpenMetrics fixture、受 Basic Auth 保护的真实 Kafka JMX Exporter HTTP 链路和显式 `pure-rust` 读取候选已实现。纯 Rust 全能力替换和真实 Windows 证据仍未完成。
- 真实 Windows 窗口证据仍与 headless 验证分开记录；未取得窗口证据的项目不能写成真实窗口已验收。
- 本次联调边界固定为：MySQL、Redis、PostgreSQL 和 MongoDB 只使用本机 Docker；Kafka、MQTT 和 SSH 终端可以使用 `10.17.17.114`；`ramag-platform` 测试不启动 relay。

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
| Kafka 工作台 | 集群、Topic、消息读取/搜索/生产、ACL、配置、消费者组、实时 Tail、Metrics Snapshot、Schema Registry 版本浏览、受保护的真实 Kafka JMX Exporter 本机链路和纯 Rust 读取候选已有 | `KAFKA-023` 三个消息定位切片和阶段 27 已完成，继续维护功能矩阵，再补真实 Windows 证据 | 纯 Rust 全能力替换、外部生态大模块和批量消息生产 |
| 数据库工作台 | SQL、Redis、MongoDB 查询、结果、事务和迁移基础能力已有 | 按 DBeaver/DataGrip 能力表推进结果查看、大字段恢复、对象导航、执行计划和迁移工作流的功能/UI 对齐 | 把 Redis/MongoDB 强行套用 SQL 语义 |
| 容器管理工具 | CMT-001 已完成工具入口、Docker/Kubernetes 连接配置模型和空工作台 | 进入 CMT-002，接入本机 Docker Engine 只读查询和连接测试 | 远程明文 Docker TCP、动态插件、Secret 明文和任意 Shell |
| 质量与工具链 | stable channel、统一 Cargo 命令、Windows MSVC 路线已建立 | 保持 CI、WSL Linux 验证、源码尺寸和 LF 规则一致 | 为单个平台恢复独立的日常编译命令 |

跨产品的 UI 响应性问题不再单独生成一条长期大路线。出现新的可复现 P0/P1 问题时，按下面的交付切片规则插入当前队列，并在对应专项文档记录实现细节。

## 3. 当前交付队列

同一时间只允许一个任务处于“开发中”。表中顺序是当前主线顺序；“待开始”任务不能提前扩展实现。

| ID | 产品线 | 负责人范围 | 状态 | 依赖 | 验收重点 |
|---|---|---|---|---|---|
| `PLAT-001` | 插件平台 | `ramag-domain`、`ramag-app` | 已完成 | 无 | `PluginId`、API 版本、能力集合、设置模式和重复注册校验有单元测试；现有工具顺序和入口不变 |
| `PLAT-002` | 插件平台 | `ramag-app`、`ramag-bin` | 已完成 | `PLAT-001` | 静态插件注册、初始化、失败隔离、逆序关闭和迟到调用拒绝有测试 |
| `PLAT-003` | 插件平台 | `ramag-ui`、`ramag-app` | 已完成 | `PLAT-002` | 插件状态、注册错误和可用入口在 360/1024/1440 headless 窗口内可见 |
| `TERM-001` | SSH/终端 | `ramag-domain`、`ramag-infra-ssh`、`ramag-tool-ssh` | 已完成（真实端点已验证） | `PLAT-003` | 会话状态、重连和 `-L/-R/-D` 参数模型有 OpenSSH 参数测试；Windows OpenSSH 客户端访问 WSL OpenSSH 端点已覆盖 Shell、SFTP、三类转发、停止、重连和错误 Host Key |
| `KAFKA-001` | Kafka | `ramag-domain`、`ramag-app`、`ramag-infra-kafka`、构建维护 | 阶段 19 代码完成，纯 Rust 读取候选已在 Windows GNU 和本机 Docker KRaft 验证 | `PLAT-003` | 阶段 18 能力矩阵已记录；`KafkaTransport` 适配边界、能力快照、native 命名和显式 `pure-rust` Fetch/ListOffsets 路径已落地，保持当前用户流程 |
| `KAFKA-025` | Kafka | `ramag-domain`、`ramag-app`、`ramag-infra-kafka`、`ramag-tool-kafka` | 已完成（Docker/headless；真实窗口待补） | `KAFKA-001` | 管理模式单条消息生产、二次确认、只读拒绝、失败保留输入、Broker Partition/Offset/Timestamp 和 Docker 生产回读 |
| `DB-001` | 数据库 | `ramag-app`、`ramag-tool-dbclient` | 待开始 | `PLAT-003` | 结果查看模式、大字段限制、编辑失败恢复和连接上下文隔离有测试 |
| `CMT-001` | 容器管理 | `ramag-domain`、`ramag-tool-container`、`ramag-bin`、`ramag-ui` | 已完成 | `PLAT-003` | 工具入口、平台区分、连接配置校验、空工作台和 360/800/1024/1440 headless 布局测试 |
| `CMT-002` | 容器管理 | `ramag-domain`、`ramag-app`、`ramag-infra-container`、`ramag-tool-container` | 待开始 | `CMT-001` | 本机 Docker Engine 连接测试、版本、容器/镜像/网络/数据卷只读查询和权限错误 |
| `UI-001` | 跨工具 UI | `ramag-ui`、各 `ramag-tool-*` | 进行中（数据库会话紧凑窗口切片已验收，其他工作台证据待补） | `PLAT-003` | 共享弹窗、工具栏、列表和详情区在 360/1024/1440 headless 窗口内换行、滚动且不越界；真实窗口证据单独记录 |
| `DB-002` | 数据库 | `ramag-tool-dbclient`、`ramag-ui` | 待开始 | `DB-001` | 建立 DBeaver/DataGrip 功能矩阵，逐项实现并验收结果、对象导航、执行计划和迁移 UI，不以静态截图宣称完成 |
| `KAFKA-023` | Kafka | `ramag-tool-kafka`、`ramag-ui`、`ramag-infra-kafka` | 开发中（三个定位切片和受保护的真实 JMX Exporter 本机链路已完成） | `KAFKA-025` | 功能矩阵已建立；继续补真实窗口证据和下一项 AKHQ/Offset Explorer 能力 |
| `QUALITY-001` | 质量与工具链 | workspace 维护者 | 持续任务 | 每个交付切片 | stable toolchain、统一 Cargo 命令、LF、CI 过滤器和三平台发布证据保持一致 |

`UI-001` 验收记录（2026-09-07）：共享弹窗的实际打开测试发现导入表单在 360×240 窗口中仍宽 414px，左侧越界 27px，说明此前仅调整内容宽度不足以修复 Dialog 外框。当前修复统一约束快捷键、最近项目和导入弹窗的宽度、顶部偏移及内容高度；导入操作区保留在滚动区外。3 项直接打开弹窗的 headless 测试覆盖 360×240、360×640、1024×768、1440×900、打开后缩放、取消、最近项目滚动/搜索/打开和快捷键录制错误/退出。真实窗口验证未完成：本次 Computer Use 的 `list_windows()` 返回空列表，安装的 `@oai/sky` 也没有技能要求的 `documentation` 接口。其他工作台仍须逐项检查，不能由这三类弹窗的结果推断全软件已适配。

`UI-001` 验收记录（2026-09-08）：数据库 SQL 会话的对象树工具栏改用共享响应式工具栏；搜索区允许收缩，筛选按钮和系统库、刷新、编辑器操作保持固定尺寸，空间不足时换行，不再把对象树顶部控件挤出侧栏。`table_tree_toolbar_wraps_inside_sidebar_widths` 覆盖对象树最小侧栏宽度 180px，以及 280/360/1024/1440px 窗口，检查工具栏与每个操作控件的边界和相互重叠；`ramag-tool-dbclient` 共 286 项库测试通过。真实 Windows 窗口截图仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` 验收记录（2026-09-08）：SSH 连接管理列表的连接行允许固定徽标和编辑/删除操作按可用宽度换行，连接名称保留可收缩区域；`connection_manager_rows_stay_inside_supported_window_widths` 覆盖 360/1024/1440px 窗口，检查连接行、JumpServer 图标、环境/系统/认证/生产标记、远程桌面槽和操作组均未越出父行或窗口。`ramag-tool-ssh` 共 76 项库测试通过。真实 Windows 窗口截图和键盘操作仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` 验收记录（2026-09-09）：对象存储账号管理列表的账号行改为可换行布局，服务商、只读、Bucket 数量和编辑/删除区域保持固定尺寸，360px 窗口隐藏非必要的 Bucket 数量列，避免固定列把行推出内容区。`account_rows_stay_inside_supported_window_widths` 覆盖 360/1024/1440px 窗口，检查账号行及各固定区域边界；`ramag-tool-object-storage` 共 24 项库测试通过。真实 Windows 窗口截图和键盘操作仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` 验收记录（2026-09-09）：Kafka 概览的 Topic 预览行为长名称增加可收缩和省略处理，Partition 数量保持固定位置；区块副标题和集群摘要允许在窄内容区换行，避免概览内容错位。主题页按可用高度扩大列表区域，桌面窗口不再只保留约 360px 的列表高度；纵向滚动条使用完整 16px 交互区域，仅在滚动时显示，避免窄条裁切和常驻色块。`kafka_overview_keeps_sections_aligned_without_vertical_gap` 和 `kafka_topics_reflow_header_and_split_at_supported_widths` 覆盖 360/900/1024/1440px 等窗口，Kafka 工作台共 25 项库测试、Clippy 和格式检查通过。真实 Windows 窗口截图和鼠标拖动滚动条仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` 补充记录（2026-09-09）：Kafka 概览按主内容区宽度统一使用 900px 分栏断点，避免外层窗口宽度与侧栏扣除后的实际空间采用不同布局；集群摘要值允许在卡片内收缩。主题列表把 16px 滚动条改为独立右侧槽位，列表内容不再被滚动条覆盖；`kafka_overview_keeps_sections_aligned_without_vertical_gap` 增加 1200px 主内容边界，`kafka_topics_reflow_header_and_split_at_supported_widths` 检查列表内容区与滚动条的相邻关系。真实 Windows 窗口截图和鼠标拖动滚动条仍未完成。

`UI-001` 补充记录（2026-09-09）：Kafka 运行时、Topic、消息、消费者组、Schema Registry、ACL、远程配置和指标首次加载使用固定行高的骨架占位；加载期间保留正式列表的滚动区域，真实结果返回后再切换，并使用短淡入过渡避免空数据先参与布局。新增 `kafka_loading_tables_keep_stable_geometry`，概览和指标加载态覆盖 360/1200px，其他数据列表覆盖 1200px；Kafka 工具共 28 项测试通过。真实 Windows 界面截图和实际 Broker 加载过程仍未完成，不能由 headless 布局测试推断原生窗口已验收。

`UI-001` 首页切片（2026-09-09）：首页工具卡片根据扣除 Activity Bar 后的主内容区收缩，紧凑窗口使用较小边距和短字标；首页内容增加纵向滚动，拖拽网格的列数、卡片宽度和动画位置使用同一组布局参数。`home_view_scrolls_and_fits_compact_windows` 覆盖 360×260 低高度窗口，确认 Logo、工具网格和卡片不越界且存在滚动范围；`ramag-ui` 共 90 项库测试、workspace Clippy 和格式检查通过。真实 Windows 首页截图和拖拽操作仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` VCS 仓库列表切片（2026-09-09）：最近仓库行在 720px 以下改为两行响应式布局，路径移到仓库名下方并保持单行省略；桌面宽度继续显示 Git 标记、仓库名、路径和移除操作的横向信息。`repo_list_rows_reflow_inside_supported_window_widths` 覆盖 360/640/1024/1440px 窗口，检查列表头部、仓库名、路径和操作区域均未越出父容器；`ramag-tool-vcs` 共 129 项库测试通过，workspace Clippy、fmt、源码尺寸和 `git diff --check` 均通过。真实 Windows 仓库列表截图和鼠标操作仍未完成，不能由 headless 结果推断原生窗口已验收。

`UI-001` VCS 工作区空状态切片（2026-09-12）：真实 Windows Debug 构建在 1040px 桌面窗口和 360px 窄窗口中验证仓库进入、Diff 空状态和分支下拉交互；窄窗口提示文本保持在右侧主面板内，分支按钮始终保留 `feat/` 前缀并通过悬浮提示提供完整名称。新增 `vcs_empty_diff_status_stays_inside_main_panel_at_narrow_width`，`ramag-tool-vcs` 135 项库测试中 130 项通过、5 项按设计忽略；真实窗口证据使用系统 Win32 截图与鼠标输入，不记为 Computer Use。

`UI-001` MQTT 配置工作区切片（2026-09-12）：真实 Windows Debug 构建在 360×240 和 360×640 窄窗口验证配置表单；配置列表默认收起，主区提供显示/隐藏配置栏入口，显示后列表堆叠在表单上方；表单字段以 180px 最小宽度换行，桌面保持三列、窄主区按两列或单列排列，协议按钮不再互相覆盖。`ramag-tool-mqtt` 4 项库测试和 `mqtt_sidebar_collapses_and_can_be_reopened_in_narrow_window` 全部通过；真实窗口证据使用系统 Win32 截图与鼠标输入，不记为 Computer Use。

`UI-001` SSH 弹窗与连接管理切片（2026-09-12）：SSH 新建连接弹窗改用共享响应式宽度、顶部偏移和最大高度；表单在 360×240 中保留标题、首屏字段和底部测试/取消/保存操作，在 360×640 中按单列排列并保留滚动主体。JumpServer 导入连接和远程会话弹窗同步收缩宽度与高度，连接表单、资产树、资产列表和会话行在紧凑宽度下改为纵向或可换行布局。`ramag-tool-ssh` 77 项库测试、目标 Clippy 和格式检查通过；真实 Windows Debug 窗口使用系统 Win32 截图与鼠标输入验证 360×240/360×640 的新建连接、JumpServer 导入和远程会话入口，不记为 Computer Use。紧凑弹窗的深层内容仍依赖滚动查看，真实网络服务未在本切片中启动。

`UI-001` 对象存储账号表单切片（2026-09-12）：账号弹窗改用共享响应式宽度、顶部偏移和最大高度；360×240 优先显示标题、服务商选择和底部取消/保存操作，360×640 将 COS/OSS、账号、凭据和生产模式按单列布局，避免固定服务商卡片和生产开关把字段推出视口。路径弹窗同步使用共享视口约束。`ramag-tool-object-storage` 25 项库测试和紧凑表单边界测试通过；真实 Windows Debug 窗口使用系统 Win32 截图与鼠标输入验证新建账号及 COS/OSS 切换，不记为 Computer Use。账号的 Bucket 挂载字段在短窗口中继续通过表单主体滚动查看。

`UI-001` 共享确认弹窗切片（2026-09-12）：所有复用 `ramag_ui::open_confirm` 的二次确认弹窗改用共享响应式宽度、顶部偏移和最大高度，避免默认 448px 宽度在窄窗口中从左侧越界；`ramag-ui` 91 项库测试和 `confirm_dialog_stays_inside_compact_window` 通过。真实 Windows 复核使用对象存储账号取消流程，在 360×240 与 360×640 中确认标题、说明、取消和危险操作按钮均可见；系统 Win32 截图与鼠标输入不记为 Computer Use。

`UI-001` 原生窗口紧凑尺寸切片验收记录（2026-09-11）：真实 Windows 验收把主窗口最小尺寸从 `800×500` 调整为 `360×240`，`ramag-bin` 尺寸测试 15 项、`ramag-tool-dbclient` 测试 288 项通过；workspace `cargo fmt --all -- --check` 和 `cargo clippy --workspace --all-targets -- -D warnings` 通过。窄于 720px 时数据库会话默认收起横向对象树，查询区通过“切换对象树”按钮按需打开导航；宽窗口保持可拖拽左右分栏。`scripts/build-windows.ps1 -Release` 使用 Windows GNU 工具链构建，PE 依赖检查通过，产物 `91706880` 字节；产物和 `D:\Program Files\ramag.exe` SHA-256 均为 `99F5B21F70FDA7D9437A33EFB15295082502811E602C558FC3BCCCEF831E8DCD`。新安装版真实窗口截图覆盖请求尺寸 `360×240`（实际外框 `376×279`）、`800×600`、`1024×768`、`1440×900`；系统 Win32 输入验证了对象树切换和 SQL 编辑器打开，编辑器草稿在 `360×240` 保持可读且不再单字符换行。Computer Use 的 `@oai/sky` 在窗口状态调用中反复丢失 node context，因此本次按约定使用系统前台切换、Win32 鼠标输入和 `Graphics.CopyFromScreen`，未将其描述为 Computer Use 证据。源码尺寸脚本仍报告既有超 600 行文件 `crates/ramag-domain/src/entities/mqtt.rs`、`crates/ramag-infra-kafka/src/messages.rs`、`crates/ramag-infra-kafka/src/pure_rust.rs`、`crates/ramag-infra-mqtt/src/lib.rs`、`crates/ramag-tool-kafka/src/render_messages.rs`、`crates/ramag-tool-mqtt/src/lib.rs`，本切片未修改这些文件。

`KAFKA-001` 构建记录（2026-09-09）：使用 `scripts/build-windows.ps1 -Release` 和 stable Windows GNU 工具链完成 `ramag-bin` Release 构建；`fxc.exe` 着色器编译、x64 PE/GUI 子系统检查和依赖检查通过，产物为 `target/x86_64-pc-windows-gnu/release/ramag.exe`，大小 `90148864` 字节。该记录只证明本机 Release 产物和 PE 检查，不替代 Docker Broker、真实 Kafka 服务或真实 Windows Kafka 界面验收；旧进程正常退出后，已将产物复制到 `D:\Program Files\ramag.exe`，两个文件 SHA-256 均为 `788DE15744BF047A5706B0B9778E9F250BA00C2EB9458D972A2EE0E56A101BB0`。

`KAFKA-025` 验收记录（2026-09-10）：生产 UI 的 GPUI headless 测试覆盖只读拒绝、确认前不写入、取消、成功提示和失败保留输入；本机 Docker 使用 `apache/kafka:4.0.0` KRaft 服务 `ramag-kafka-test`（`127.0.0.1:19092`）和 Connect 服务 `ramag-kafka-connect-test`（`127.0.0.1:18083`），创建并核对 5000 条 fixture 消息和 61 个主题，`docker_kafka` 集成测试 7 项全部通过。生产回读覆盖显式 Partition、Key、Header 和 Broker 返回 Offset；真实 Windows 窗口截图、鼠标/键盘操作、Docker exporter 和真实 Broker 运行指标端点仍未完成。

`KAFKA-023` 首个切片验收记录（2026-09-11）：Topic 详情的每个 Partition 提供“浏览此 Partition”入口；点击后切换消息页并保留 Topic、Partition，清理旧消息页和详情选择，不自动发起 Kafka 读取。`cargo test --locked -p ramag-tool-kafka --lib` 31 项通过；真实 Windows 窗口截图和鼠标操作仍未完成。

`KAFKA-023` 第二个切片验收记录（2026-09-11）：消费者组详情的有效已提交 Offset 提供“浏览”入口；点击后切换消息页并保留 Topic、Partition、起始 Offset，清空结束 Offset并保持 Offset 模式，清理旧消息状态且不自动读取。`cargo test --locked -p ramag-tool-kafka --lib` 32 项通过；真实 Windows 窗口截图和鼠标操作仍未完成。

阶段 23 本机 HTTP fixture 验收记录（2026-09-11）：Docker Compose 启动 `ramag-kafka-metrics-test`（`nginx:1.27-alpine`，`127.0.0.1:19100/metrics`），与 `ramag-kafka-test`（`apache/kafka:4.0.0`，`127.0.0.1:19092`）和 `ramag-kafka-connect-test`（`apache/kafka:4.0.0`，`127.0.0.1:18083`）分别保持 healthy。Rust `docker_kafka` 集成测试 8 项全部通过，新增测试验证 `PrometheusBrokerMetricsDriver` 的 `ExternalBrokerMetrics` 来源、Broker ID、CPU/内存/磁盘/延迟和采样时间；静态 fixture 不代表真实 Kafka JVM 指标。

阶段 23 真实 JMX Exporter 验收记录（2026-09-11）：Docker Compose 为 `apache/kafka:4.0.0` KRaft Broker 开启容器内 JMX/RMI `9999`，使用固定 SHA-256 的 Prometheus JMX Exporter `1.6.0` 暴露受 Basic Auth 保护的 `127.0.0.1:19101/metrics`。`docker_kafka_reads_real_broker_jmx_exporter` 通过真实 HTTP 请求确认无认证返回 `401`、带认证返回 `200`，再回读 Kafka JVM 的 CPU、Heap、Topic/Partition 磁盘和请求延迟；解析器对重复磁盘样本求和、对重复延迟样本取最大值。生产环境仍需替换测试凭据、启用 HTTPS，并补充真实 Windows 窗口证据。

`UI-001` Kafka 加载稳定性切片（2026-09-13）：概览页 Partition 健康区改用有界虚拟列表，只渲染当前可见行，保留最多 100 条明细和完整数量提示；窄窗口行拆为两行并固定行高，避免加载较大 Partition 快照时一次性构造全部 UI 行导致桌面进程退出。Broker 健康状态文本增加可伸缩单行布局，`协议可达` 不再按字符竖排。`ramag-tool-kafka` 38 项库测试、workspace 全量测试、workspace Clippy、格式检查、`ramag-bin` Debug 构建均通过；本机 Docker Kafka 使用 `apache/kafka:4.0.0`、服务 `ramag-kafka-test`（`127.0.0.1:19092`）及配套 Connect、ksqlDB、Schema Registry、指标服务，12 项 Kafka 集成测试通过。真实 Windows Debug 窗口连接该 Broker 后显示 182 个 Partition，系统 Win32 截图和滚动操作确认页面持续响应；截图不记为 Computer Use，生成文件保留在本地测试目录，未纳入源代码提交。

`UI-001` 功能数据复核记录（2026-09-13）：本轮在本机 Docker 服务上重新验证功能数据链路，使用 MySQL 8.0（`127.0.0.1:13306`）、PostgreSQL 17-alpine（`127.0.0.1:15432`）、Redis 7-alpine（`127.0.0.1:16379`）、MongoDB 8.2（`127.0.0.1:27018`），以及 Kafka `apache/kafka:4.0.0`（`127.0.0.1:19092`）、Connect（`18083`）、ksqlDB（`18088`）、Schema Registry（`18081`）、指标端点（`19100`）和 JMX Exporter（`19101`）；服务均保持 healthy。`cargo test --locked -p ramag-tool-kafka --lib` 的 38 项通过，`ramag-infra-kafka` 的 `docker_kafka` 12 项真实服务测试通过；workspace 测试中的 App 207 项、数据同步 8 项、传输 9 项、MySQL/PostgreSQL/MongoDB 集成测试和 Kafka 集成测试均在 Redis 前通过。workspace 全量测试在 Redis 种子完整性检查处停止：DB 0 当前只有 45,014 个 Key，低于测试要求的 46,000 个，带 TTL 的 Key 已过期；本轮没有运行重置或重新生成测试数据，因此该项仍记为未完成。`cargo fmt --all -- --check` 和加载 VS 18 MSVC 环境后的 `cargo clippy --workspace --all-targets -- -D warnings` 通过。当前重新采集的 `artifacts/ui-data-functional/native-kafka-current.png` 显示已保存但未连接的页面，不能作为本轮真实数据窗口验收；仓库已有连接后历史截图仍单独保留，不与本轮 headless/服务数据证据混写。

`UI-001` MQTT 请求上下文隔离切片（2026-09-14）：切换 MQTT 配置或开始新的异步操作时递增配置上下文和请求代次；Broker 状态、发布/订阅、Mosquitto Dynamic Security、静态文件以及配置保存、删除和连接测试的迟到结果，只能更新发起时对应的配置。切换配置会同时清除旧配置的运行数据和进行中状态，避免旧请求把新页面留在错误的加载状态。新增 `mqtt_snapshot_result_does_not_cross_profile_context` 回归测试；加载 VS18 MSVC 环境后 `cargo test --locked -p ramag-tool-mqtt --lib` 的 7 项和 `cargo clippy --locked -p ramag-tool-mqtt --lib -- -D warnings` 均通过，`cargo fmt --all -- --check` 与 `git diff --check` 通过。本轮未新增真实窗口截图或远端 MQTT 服务证据。

`DB-001` 表格结果工作区切片（2026-09-13）：结果数据区收敛为默认表格视图，移除表格、树形、文本和转置转换按钮及对应状态；保留数据结果/执行计划页签、分页、双轴滚动、单元格查看、复制和编辑。服务端排序重新加载结果时恢复排序前的横向位置；结果过滤允许单个 `WHERE` 表达式包含多个 `AND` 条件，同时继续拒绝多语句输入。`cargo test --locked -p ramag-tool-dbclient --lib` 288 项通过；本机 Docker MySQL 8.0（`127.0.0.1:13306`）和 PostgreSQL 17-alpine（`127.0.0.1:15432`）的派生表查询实际执行多个 `AND` 条件，分别返回 99,900 和 9 行。真实 Windows Debug 窗口使用系统 Win32 截图和鼠标/键盘输入验证 `bulk_records` 的 100,000 行表格、99,900 行多条件筛选，以及横向滚动后点击 `created_at` 排序仍保持右侧列视口；截图保存在本地 `artifacts/ui-data-functional/`，不纳入源代码提交。

`DB-001` 对象树刷新切片（2026-09-13）：刷新 schema 时保留已有对象树、展开状态、表缓存和当前选择；刷新期间只在顶部显示状态，旧表行继续可操作，schema 请求失败时显示可重试提示；表请求失败时保留旧表行并以内联状态说明失败。删除的 schema 会清理对应展开状态、列缓存和选择。`cargo test --locked -p ramag-tool-dbclient --lib` 的表树模型和渲染回归测试通过；MSVC Debug 构建产物位于 `target\x86_64-pc-windows-msvc\debug\ramag.exe`。本机 Docker MySQL 8.0（`ramag-db-test-mysql`，`127.0.0.1:13306`，healthy）实际加载 `ramag_test.bulk_records`，系统截图验证 100,000 行结果在刷新开始、进行中和完成后均未被全屏加载态遮挡；截图保存在本地 `artifacts/ui-data-functional/`，不纳入源代码提交。

`UI-001` 数据库工作台切片（2026-09-08）：`ramag-ui` 提供统一的对话框宽度、顶部偏移和最大高度计算；数据库连接选择、连接表单、数据同步、查询历史、单元格查看、结果差异、Schema Diagram、表结构差异、表设计、元数据 SQL 和删除确认弹窗均改为按视口收缩。连接表单和连接选择器在 680px 以下改用纵向布局，正文或长内容继续在有界滚动区内显示；结果差异和 Schema Diagram 的内容高度随窗口预算变化。`cargo test --locked -p ramag-tool-dbclient --lib` 通过 285 项，现有 360/1024/1440 headless 检查继续通过。真实 Windows 窗口截图和操作记录仍未完成，Kafka 工作台的功能/UI 对齐按 `KAFKA-023` 单独排期。

`PLAT-003` 完成后，`TERM-001` 已完成代码和真实 OpenSSH 端点验收；真实 Windows 窗口和独立转发状态面板仍单独排期。`KAFKA-001` 的传输能力矩阵和适配边界已落地，Metrics Snapshot、Live Message Tail 和阶段 25 消息生产已在协议/headless/Docker 范围内完成；纯 Rust Transport、外部 Broker 运行指标端点和真实 Windows 证据仍单独排期，终端和数据库任务不得借机修改 Kafka 或插件协议。

## 4. 分阶段主线

### 阶段 0：基线与文档收敛（已完成）

完成内容：

- README、架构说明、工具清单和平台关系已对齐当前独立仓库身份。
- Kafka 路线已区分现有管理能力与尚未实现的 Transport、Metrics 和 Tail 能力。
- 插件、Kafka、终端和数据库的职责边界已经写入专项文档；历史执行日志不再作为当前排期依据。
- Windows MSVC、Linux、macOS 的日常 Cargo 命令保持一致；平台差异只用于准备环境和发布包装。
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

`KAFKA-001` 的能力矩阵和默认构建评估已完成。`ramag-domain`、`ramag-app` 和 `ramag-tool-kafka` 不得直接依赖 `rdkafka` 类型；具体客户端只能位于 Kafka Transport 适配层。

传输边界、取消、断线恢复和 Windows 构建依赖已有结论；`KafkaMonitoringDriver`、Consumer Group Lag、Metrics Snapshot 和 Live Message Tail 已实现，阶段 25 还增加了受管理模式和确认保护的单条消息生产。TLS/SASL、纯 Rust Transport、生产 exporter 安全配置仍需单独验收；本机静态 OpenMetrics fixture 和真实 Kafka JMX Exporter 已验证端点接入链路。Broker CPU、内存、磁盘、JVM 和请求延迟必须来自明确配置的 JMX、Prometheus 或 exporter 数据源，不能由 Admin API 伪造。

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

`UI-001` MQTT 权限预览切片（2026-09-14）：`ramag-tool-mqtt` 在窗口宽度小于 760px 时将 Mosquitto 用户权限预览从固定四列改为纵向信息块，宽窗口继续使用表格行，避免 Topic、Role 和权限信息越出内容区。`mqtt_client_permissions_reflow_inside_supported_window_widths` 覆盖 360/1024/1440 headless 窗口；`cargo test -p ramag-tool-mqtt --lib` 的 8 项测试、workspace MSVC Clippy、格式检查和 `git diff --check` 通过。`check-source-size.ps1` 仍报告既有的 `dynamic_security_operations.rs` 为 606 行，`HEAD` 基线同样为 606 行，本次未修改该文件。真实 Windows 窗口截图和键盘操作仍待补充。

`UI-001` MQTT Dynamic Security 编辑器切片（2026-09-14）：用户、Group 和 Role 编辑器的标题区、字段组和保存/删除操作在窄窗口下改为可换行布局，360px 时字段组纵向排列，宽窗口继续使用横向字段布局；新增 `mqtt_management_editors_reflow_inside_supported_window_widths`，覆盖 360/1024/1440 headless 窗口。workspace MSVC 测试、Clippy、格式检查和 `git diff --check` 通过；`check-source-size.ps1` 仍只报告既有的 `dynamic_security_operations.rs` 为 606 行，本切片未修改该文件。真实 Windows 窗口截图和键盘操作仍待补充。

`UI-001` MQTT 发布/订阅操作区切片（2026-09-14）：发布按钮和订阅启停操作在 760px 以下保留完整可用宽度，订阅消息的长 Topic 与 QoS/时间元数据允许换行，避免紧凑窗口把操作或消息头部推出主工作区；新增 `mqtt_message_operations_reflow_inside_supported_window_widths`，覆盖 360×240、640×480、1024×768 和 1440×900 headless 窗口，并使用长 Topic、512 字节 Payload 回归布局边界。真实 Windows 窗口截图、键盘操作和远端 MQTT 服务证据仍待补充。

`UI-001` 表属性紧凑弹窗切片（2026-09-15）：触发器元数据区域根据弹窗可用高度动态收缩，桌面窗口保持原有高度，`360×240` 窗口仍同时保留触发器列表和 DDL 预览；新增 `trigger_metadata_and_ddl_stay_inside_a_compact_modal` 和高度边界测试。`ramag-tool-dbclient` 库测试 301 项、workspace MSVC Clippy、格式检查和 `git diff --check` 通过；源码尺寸检查仍只报告基线已有的 `dynamic_security_operations.rs` 606 行。本切片未新增真实 Windows 窗口截图或数据库服务证据。

`UI-001` 结果表编辑操作区切片（2026-09-15）：待提交单元格修改以及新增行的取消/提交按钮统一收进独立的响应式操作区，分页和长状态摘要换行时不会把操作按钮挤出结果状态栏；新增 `pending_edit_actions_stay_inside_status_bar_at_supported_widths`，覆盖 280/360/1024px，并为四个变更按钮补充可定位的调试选择器。`ramag-tool-dbclient` 结果表渲染专项 7 项测试、目标 Clippy、格式检查和 `git diff --check` 通过。完整库测试的 302 项断言均打印通过，但 Windows 测试进程在既有表属性测试结束阶段以 `STATUS_STACK_BUFFER_OVERRUN` 退出；相关表属性测试单独运行通过，本切片没有把该基线进程异常记为完成证据。真实 Windows 窗口截图和实际数据库服务证据仍待补充。

`UI-001` SQL 事务工具栏切片（2026-09-15）：活动事务的提交、回滚、保存点和最近保存点状态，以及未开启事务时的开始入口，统一使用可收缩、可换行的控制区；窄窗口下各按钮继续位于结果工具栏和事务操作组边界内。`active_transaction_controls_wrap_inside_three_window_widths` 与 `inactive_transaction_control_wraps_inside_three_window_widths` 覆盖 360/1024/1440px，两个测试均通过；`cargo fmt --all --check`、`git diff --check` 通过。真实 Windows 窗口截图、实际数据库服务数据和完整库测试的进程退出稳定性仍待补充。

`UI-001` 结构对比标题栏切片（2026-09-15）：表结构对比弹窗的源表/目标表连接上下文增加收缩、单行省略和稳定的调试选择器，迁移预览、复制差异和刷新按钮继续由共享响应式工具栏换行承载；新增 `schema_diff_toolbar_keeps_context_and_actions_inside_supported_widths`，覆盖 360/1024/1440px。`ramag-tool-dbclient` 库测试 307 项、目标 Clippy、格式检查和 `git diff --check` 通过；真实 Windows 窗口截图和实际数据库服务证据仍待补充。

`structured-query-plan` 执行计划复制切片（2026-09-15）：结构化和原始 EXPLAIN 结果共用“复制原始执行计划”入口；PostgreSQL 单列计划保留原始行，MySQL 多列计划复制列名和制表符分隔字段，窄窗口下复制与原始视图按钮仍位于响应式工具栏内。新增 `plan_copy_preserves_single_column_raw_rows`、`plan_copy_includes_headers_for_multi_column_plans`，并扩展 `renders_structured_plan_tree_and_keeps_it_read_only` 覆盖 360/1024/1440px；`ramag-tool-dbclient` 库测试 309 项、目标 Clippy、格式检查和 `git diff --check` 通过。源码尺寸检查仍报告既有的 `result_table/render.rs`（603 行）和 `dynamic_security_operations.rs`（606 行）超过 600 行限制，本切片未修改这两个文件。真实 Windows 窗口和实际数据库服务证据仍待补充。

`schema-migration-review` 重命名候选切片（2026-09-15）：表结构差异只在源表和目标表存在唯一、完整定义匹配但字段名不同的情况下标注“重命名候选”；定义不同或匹配不唯一时继续按普通新增/删除显示，不自动生成改名 SQL。新增 `uniquely_matching_unmatched_columns_are_marked_as_rename_candidates` 和 `ambiguous_column_shapes_are_not_marked_as_rename_candidates`；`ramag-tool-dbclient` 库测试 311 项、目标 Clippy、格式检查和 `git diff --check` 通过。源码尺寸检查仍报告既有的 `result_table/render.rs`（603 行）和 `dynamic_security_operations.rs`（606 行）超过 600 行限制，本切片未修改这两个文件。真实 Windows 窗口和实际数据库服务证据仍待补充。

`schema-migration-review` 迁移阶段摘要切片（2026-09-15）：迁移生成器为每条语句保留阶段和破坏性标记，预览按“删除受影响的外键 -> 删除受影响的索引 -> 处理字段变化 -> 恢复索引 -> 恢复外键”展示执行顺序和各阶段计数，不改变实际 SQL、显式确认或生产连接只读规则。新增 `summarize_stages_preserves_dependency_order_and_destructive_counts`、`migration_stages_stay_inside_preview_at_supported_widths`，并补充真实生成脚本的阶段断言；`ramag-tool-dbclient` 库测试 313 项、目标 Clippy、格式检查和 `git diff --check` 通过。源码尺寸检查仍报告既有的 `result_table/render.rs`（603 行）和 `dynamic_security_operations.rs`（606 行）超过 600 行限制，本切片未修改这两个文件。真实 Windows 窗口和实际数据库服务证据仍待补充。

`DB-001` 触发器元数据 Docker 回读切片（2026-09-15）：MySQL SQL 后端对已拆分的 DDL/DML 增加文本协议执行钩子，解决 `CREATE TRIGGER` 被 prepared statement protocol 拒绝的问题；专用 `ramag-db-test` MySQL 服务开启 `log-bin-trust-function-creators=1`，仅用于本机测试账号创建临时触发器。本机 Docker Compose 从 `scripts/db-test/compose.yaml` 启动并保持 MySQL 8.0（`ramag-db-test-mysql`，`127.0.0.1:13306`）和 PostgreSQL 17-alpine（`ramag-db-test-postgres`，`127.0.0.1:15432`）healthy；集成测试分别创建临时表级触发器并通过 `list_triggers` 回读名称、时机、事件和定义，随后删除临时表、触发器和 PostgreSQL 函数，未清理供测试复用的 Docker named volumes。MySQL 15 个和 PostgreSQL 16 个集成测试、对应 11 个和 14 个单元测试，以及 SQLite 4 个单元测试均通过。数据库工作台 headless 测试覆盖表树分组折叠、表属性触发器与 DDL 紧凑弹窗、大字段查看器和未提交编辑操作区；6 个专项用例全部通过。全量 `ramag-tool-dbclient` 315 项断言均打印通过，但 Windows 测试进程仍在既有 GPUI 触发器窄弹窗测试结束阶段以 `STATUS_STACK_BUFFER_OVERRUN` 退出，因此本切片只记录专项 UI 证据，不把该进程异常写成全量通过；真实 Windows 窗口截图仍待补充。
