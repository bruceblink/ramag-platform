# Ramag Platform 最新开发计划

> 状态：现行计划
> 更新日期：2026-10-27
> 当前基线：`main`，本地 UI 维护切片已提交；远端同步按发布流程处理
> 产品定位：保留全部现有原生工作台能力的 Rust/GPUI 桌面平台

本计划以“好钢用在刀刃上”为原则。`ramag-platform` 与
[`it-tools-template`](F:/project/it-tools-template/) 是两个独立项目。它们不共享进程、
运行时、代码包、接口协议或发布物。两个项目只在功能和应用场景上互补。

历史的迁移方案和切片记录保存在
[01-development-plan-history.md](01-development-plan-history.md)、专项计划和
[历史归档](archive/2026-09-25-pre-datagrip-rebaseline/)。历史记录不自动变成新的开发范围。

## 1. 当前范围决定

截图标出的两个工具已经从 Ramag 移除：

- 本机协作（`collaboration`）。
- JSON Path 提取器（`json-path-extractor`）。

除此之外，当前 Ramag 工具和基础设施全部保留。保留范围包括数据库、Redis、MongoDB、
API、Kafka、MQTT、Git、SSH/SFTP/PTY、对象存储、容器、剪贴板、系统监控、更新、插件宿主、
加密本地存储和 GPUI Shell。API 工作台内部用于响应变量和断言的 JSON Path 逻辑属于 API
功能，继续保留；它不等于独立 JSON Path 提取器。

`it-tools-template` 可以独立提供 Web、PWA、Electron 和自己的通用工具。Ramag 不因
it-tools-template 的实现而删除其他现有工具，也不把 it-tools-template 作为运行时依赖。

## 2. 功能冻结规则

从本计划生效后，Ramag 只做现有功能的打磨和维护。维护包括：

- 缺陷修复、崩溃修复和错误恢复。
- 性能、资源上限、取消、并发和稳定性改进。
- 安全、凭据保护、权限边界和兼容性修复。
- 现有页面的可用性、可访问性、响应式布局和视觉一致性改进。
- 构建、测试、文档、发布和平台适配维护。

本冻结规则适用于当前主线。当前主线完成前，不启动插件接口重构、插件迁移或新的业务功能。
用户已明确指定当前主线完成后的方向：先设计平台插件接口，再开发试点插件和逐步迁移现有能力；
插件化路线不再受“只维护现有功能”的冻结限制。插件接口仍采用分阶段评审，不能把未批准的
动态加载、第三方代码或插件市场视为已决定方案。

## 3. 现有能力边界

| 能力 | Ramag 当前处理 | it-tools-template 当前处理 |
|---|---|---|
| 数据库、Redis、MongoDB | 保留原生工具和基础设施，持续维护 | 独立实现 Web/Electron 工作台 |
| API、Kafka、MQTT | 保留原生工具和协议适配，持续维护 | 独立实现 Web/Electron 工作台 |
| Git、对象存储、容器 | 保留原生工具和基础设施，持续维护 | 独立实现 Web/Electron 工作台 |
| SSH、SFTP、PTY、端口转发 | 保留原生会话、终端和安全边界 | 按自身产品范围独立决定 |
| 剪贴板、系统监控、更新和设置 | 保留现有桌面功能，持续维护 | 独立实现自己的桌面和 Web 版本 |
| 插件平台和 GPUI Shell | 当前保留静态插件宿主；当前主线完成后设计稳定接口、开发插件并逐步收敛现有工具 | 使用自己的插件和页面体系 |
| 本机协作 | 已移除 | 可在自身项目中独立设计 |
| JSON Path 提取器 | 已移除 | 可在自身项目中独立提供 Web/Electron 功能 |

两个项目可以覆盖相同场景，但各自维护参数、权限、错误、取消、分页、进度、资源限制、
配置、凭据、存储和测试。一个项目的测试结果不能作为另一个项目的验证证据。

## 4. 当前执行队列

| 顺序 | ID | 内容 | 完成条件 |
|---|---|---|---|
| P0 | RAMAG-REMOVE-001 | 清理本机协作和 JSON Path 提取器的注册、窗口、领域、存储、独立 crate、脚本和工具目录引用 | 工作区不再构建或显示两个工具；其余工具注册顺序保持稳定 |
| P1 | RAMAG-MAINT-001 | 对现有数据库、消息、Git、SSH、容器、对象存储、剪贴板和系统监控做缺陷、稳定性和性能维护 | 受影响 crate 的测试、fmt、Clippy 和构建通过 |
| P2 | RAMAG-UI-001 | 持续打磨现有 GPUI Shell、首页、工具页、设置、错误和响应式布局 | 真实窗口与 headless 证据分开记录；不引入新业务入口 |
| P3 | RAMAG-RELEASE-001 | 维护 Windows、Linux、macOS 构建、安装包、更新和发布说明 | 平台构建、资产、校验和发布记录一致 |

P0 的代码清理已完成。P1 至 P3 只允许维护现有能力。任何新增范围都必须先更新本计划，
并在用户明确说明后执行。

P3 的 `v0.7.0` 发布切片已完成，证据见 [Ramag 0.7.0 发布计划](25-ramag-0.7.0-release-plan.md)。
数据库 `A-UI-REAL` 已于 2026-10-26 在真实窗口验收 MySQL/PostgreSQL 触发器子树、连接恢复、事务行编辑读回和 DML 连接失败反馈；PostgreSQL 结果网格的列宽拖动、大结果集分页和长表头也已验收。
完整数据库操作矩阵仍未完成。`DB-UX-006A` 的 10,000 行读取和 `DB-UX-006B` 的 1 MiB 字段预览已完成真实窗口验收；结果、内存和限制见[数据库工作区路线图](03-database-client-datagrip-roadmap.md)。发布验收与数据库窗口验收分别记录。

用户继续要求打磨整体 UI。`RAMAG-UI-001C` 已按窗口宽度扩展首页工具卡片网格，并保持工具入口、排序和卡片操作不变。`RAMAG-UI-001D` 已完成系统监控页的中文导航、标题和状态文案。`DB-UX-006D` 已完成并行查询结果隔离、页面/统计查询取消、取消耗时和取消历史验收。`RAMAG-UI-001E` 已完成 API 工作台的中文界面词汇和侧栏信息层级。`RAMAG-UI-001F` 已完成版本管理空状态的说明和动作层级。`RAMAG-UI-001G` 已完成容器连接失败与 WSL Docker 引导层级。`RAMAG-UI-001H` 已完成设置页空配置卡的说明和下一步层级。`RAMAG-UI-001I` 已完成工具显示页的入口摘要层级。`RAMAG-UI-001J` 已完成关于页的产品身份和更新状态层级。`RAMAG-UI-001K` 已完成剪贴板设置页的状态摘要层级。当前执行 `RAMAG-UI-001L`，增加监控预设空状态的说明和保存引导层级。此前的 `RAMAG-UI-001A/B`、`DB-UX-006B` 和 `DB-UX-006C` 记录保持不变。

当前 UI 子切片已完成 API 工作台简约布局和两条分隔栏的真实窗口拖动检查。2026-10-25 已完成
[连接型工作区验收计划](22-connection-workspace-acceptance-plan.md)中的全局 UI 回归：真实窗口确认
首页、API、Settings、数据库、Kafka、MQTT 和 SSH 页面入口可用，主题切换后已恢复。
数据库专项的 `A-UI-REAL` 仍在进行：MySQL 8.4 和 PostgreSQL 17 的连接、对象树、触发器子树、表数据页、
手动事务编辑、提交和回滚读回，以及单元格 UPDATE 连接失败反馈均有真实窗口证据；MySQL 连接失败后的恢复也已通过。
PostgreSQL 的 100,000 行分页、10,000 行只读查询、1 MiB 以上字段预览、1,000 张表的对象树、页内末行、列宽拖动、末列横向滚动和长表头已通过真实窗口检查。`DB-UX-006D` 的真实窗口检查确认短查询能在长查询运行时完成；停止长查询时，页面请求和分页统计请求均停止，取消失败记录进入历史，服务器活动查询在约 `629 ms` 后归零。详见[数据库工作区路线图](03-database-client-datagrip-roadmap.md)。

### RAMAG-UI-001A：主壳、首页与设置页布局

**范围：**统一主导航的选中状态、标题和页面切换反馈；打磨首页工具标题、工具卡片层级和留白；统一设置页标题、分组和行间距，并使用一致的中文界面文案。现有工具注册、卡片点击与拖动排序、主题、设置字段及保存行为保持不变。不增加业务入口。

**验收条件：**

- 设置、首页和工具入口切换后，侧栏选中项、窗口标题、面包屑和正文属于同一页面。
- 首页所有已注册工具仍可打开；管理入口、拖动排序和空状态可用。
- 设置页字段、当前值、按钮动作和持久化键不变；浅色和深色主题均可读。
- Headless GPUI 覆盖 `360x640`、`1024x768`、`1440x900`，检查主壳、首页和设置页的裁切、重叠、滚动和操作命中。
- Computer Use 在真实窗口检查首页、设置页、主题切换、工具入口和至少一次窗口尺寸调整；记录实际屏幕支持的尺寸。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚本切片涉及的 Shell、首页、设置视图和对应验证记录；不回滚前序数据库验收、WSL Docker 环境或其他用户数据。

**验收记录（2026-10-26）：**`cargo test --locked -p ramag-ui` 通过，147 项测试全部通过。首页与系统设置的 Headless GPUI 检查覆盖 `360x640`、`1024x768`、`1440x900`，并检查浅色和深色主题。Computer Use 在真实窗口检查了首页四列卡片、工具管理面板、API 测试入口、设置页标题与导航、浅色和深色主题；窗口从 `1200x775` 缩至 `1026x775` 后，首页仍显示完整的四列卡片。真实屏幕不支持 `1440x900` 和 `360x640` 窗口尺寸，这两种尺寸仅有 Headless GPUI 证据。卡片点击和拖动排序由 Headless GPUI 测试覆盖；真实窗口未单独验收拖动排序。

### RAMAG-UI-001B：数据库查询工作区信息层级

**范围：**压缩 SQL 查询区顶部重复的连接信息，保留工作区标题和 Schema 选择器；连接名称、类型、地址与状态继续由数据库连接标题区显示。查询标签、执行、取消、结果、连接和 Schema 行为不变。

**验收条件：**

- `360x640`、`1024x768`、`1440x900` 的 Headless GPUI 布局中，工作区标题和 Schema 选择器均完整位于查询面板内。
- 顶部上下文条高度不超过 `48px`，不再重复显示连接名称和地址。
- 真实窗口中，连接状态与 Schema 选择器仍可识别；编辑器和结果区域获得额外垂直空间。
- 执行、取消和结果状态不因布局变化而改变。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-tool-dbclient`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚数据库查询区的标题布局、相关布局断言和本节记录；不回滚连接标题、查询行为、数据库验收记录或 WSL Docker 环境。

**验收记录（2026-10-27，完成）：**真实窗口为 `1626x927`。新窗口显示一行 `42px` 查询区标题、SQL 标记和 Schema 下拉；连接名称、PostgreSQL 地址和连接状态仍显示在上方连接标题区。SQL 编辑器与结果区的分隔位置上移，编辑器获得更多垂直空间。应用启动日志显示隔离测试连接已恢复，并读取 PostgreSQL Schema 元数据；UI 布局检查时未在编辑器中运行 SQL，之后的只读查询见[数据库工作区路线图](03-database-client-datagrip-roadmap.md)中的 `DB-UX-006D` 记录。Headless GPUI 布局测试覆盖 `360x640`、`1024x768` 和 `1440x900`，`query_context_bar_keeps_schema_inside_supported_widths` 通过。`cargo test --locked -p ramag-tool-dbclient` 的 376 项测试全部通过；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。隔离调试窗口已关闭，临时测试数据目录保留。

### RAMAG-UI-001C：首页宽屏卡片布局

**范围：**首页工具卡片根据主内容区宽度从 1 列扩展到最多 8 列。标题栏与卡片网格使用相同宽度并保持居中。卡片宽度、间距、工具入口、拖动排序和管理入口保持不变。不新增业务入口。

**验收条件：**

- `360`、`1024`、`1440`、`1689` 和 `2560` 窗口宽度分别显示 `1`、`4`、`5`、`6` 和 `8` 列。
- 标题栏和卡片网格同宽、对齐；卡片不越过窗口或侧栏。
- 最大化窗口中，10 个现有工具按 8 列和 2 列两行排列；卡片点击和拖动排序的代码路径不变。
- Headless GPUI 覆盖窄、中、大和宽屏尺寸；真实窗口检查实际可用的最大尺寸。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚首页卡片列数、标题与网格宽度和本节记录；不改工具注册、持久化顺序、侧栏宽度或设置值。

**验收记录（2026-10-27，完成）：**真实窗口在 `2560x1392` 最大化尺寸显示 8 列和 2 列两行，标题、管理入口和卡片网格左右对齐。`1200x774` 窗口显示 4 列，卡片宽度与间距保持原值。调试应用使用隔离数据目录 `C:\Users\likanug\AppData\Local\Temp\codex-ramag-ui-polish-20261027-01a17345-02`，没有连接数据库或修改用户数据。Headless GPUI 在浅色和深色模式下覆盖 `360x640`、`1024x768`、`1440x900`、`1689x900` 和 `2560x1392`；列数计算用例覆盖 `360/600/900/1024/1200/1440/1689/2560` 宽度。`cargo test --locked -p ramag-ui` 的 147 项测试通过，`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

### RAMAG-UI-001D：系统监控页面中文导航与信息层级

**范围：**统一系统监控页的导航标签、主要标题、状态提示、空状态和进程表操作文案。为主要系统指标提供简洁中文显示名。保留 CPU、GPU、PID、单位、设备名称和驱动提供的未知指标名。监控采样、传感器标识、已保存的屏幕/设备选择、排序、筛选、键盘导航和进程操作保持不变。

**验收条件：**

- 系统监控的 9 个页面标签和主要页面标题使用中文；`CPU`、`GPU` 等常见缩写保留。
- 概览、处理器、内存、显卡、磁盘、网络、功耗、温度和进程页的主要标题、状态与空状态可读，不显示通用英文占位文案。
- 窄窗口的标签仍可横向滚动；宽窗口的图表、指标卡和进程表布局保持现有自适应行为。
- 真实窗口检查概览、显卡和进程页；确认导航可切换，采样状态持续更新，传感器和进程数据可见。
- 运行 `cargo fmt --all -- --check`、`cargo test --locked -p ramag-tool-system`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin` 和 `git diff --check`。

**回滚边界：**只回滚系统监控页面显示文案、对应单元测试和本节记录；不改传感器采集、系统权限、持久化选择、系统设置或进程操作逻辑。

**验收记录（2026-10-27，完成）：**Computer Use 在 `1626x927` 深色主题真实窗口检查了概览、GPU 和进程页。9 个导航标签、主要标题、图表范围与采样时长、常见内存/GPU 指标、进程表列名和操作提示均显示中文；`CPU`、`GPU`、`PID`、单位、设备名、进程名和未知设备指标名保留。概览、GPU 趋势图和进程表均在窗口内完整显示；进程页有实时行数据，系统状态每 `0.5` 秒更新。本轮没有选择或操作系统进程，也没有手动运行 SQL。使用隔离测试数据目录 `C:\Users\likanug\AppData\Local\Temp\codex-ramag-ui-visual-20261027-01a17345`。`cargo test --locked -p ramag-tool-system -p ramag-system-model` 的 124 项测试通过；`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。未知设备名与驱动诊断原文仍可能包含英文，这是保留的源信息。

### RAMAG-UI-001E：API 工作台中文界面与侧栏层级

**范围：**统一 API 工作台的请求标签、响应标签、认证选项、gRPC 元数据和环境编辑器标题。将默认工作区的显示名称从 `API Workspace` 映射为“API 工作台”，并使用强调色竖线和两行统计信息区分工作区标题与请求列表。保留 HTTP、gRPC、URL、Token、DescriptorSet 等协议和文件标识；不改变请求构造、发送、保存、导入、历史记录、分隔栏拖动或工作区数据。

**验收条件：**

- HTTP 与 gRPC 请求标签、响应标签和主要编辑器标题使用中文；协议名与固定 API 标识继续保留原文。
- 默认工作区在真实窗口显示“API 工作台”、集合数和请求数；标题、搜索框、请求列表和历史区不重叠。
- `1202x774` 真实窗口的请求编辑器、空响应区和认证面板保持在主体边界内；亮色主题可读。
- Headless GPUI 的 API 工作区布局、分隔栏拖动、请求切换和协议切换测试保持通过。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-tool-api`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚 API 工作台显示文案、侧栏标题层级和对应文档；不回滚请求执行、认证数据、工作区存储、数据库服务或其他工具页面。

**验收记录（2026-10-27，完成）：**Computer Use 在 `1202x774` 真实窗口检查 API 工作台。侧栏显示“API 工作台”、集合和请求统计，并使用强调色竖线与列表分隔；HTTP 请求标签显示“参数、请求头、认证、请求体、高级”，响应标签显示“正文、响应头、耗时、测试”；认证页显示中文标题和字段，空响应区显示“尚未发送”。请求编辑器、响应区、搜索框和分隔栏均在窗口内，没有执行请求或写入工作区。`cargo test --locked -p ramag-tool-api` 的 39 项测试通过，1 项需要 Docker 的集成测试按既有规则忽略；`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

### RAMAG-UI-001F：版本管理空状态层级

**范围：**为版本管理的空仓库页补充 Git 图标、空状态标题、下一步说明和“打开”动作层级。保留顶部搜索、打开、克隆、初始化动作及其回调、最近仓库读取、筛选和持久化行为。不改变 Git 协议、仓库扫描、凭据或文件操作。

**验收条件：**

- 空仓库页显示图标、标题、说明和打开动作；说明与按钮在面板内按顺序排列。
- `360`、`640`、`1024` 和 `1440` 宽度下，空状态各元素不越出仓库面板；亮色和暗色主题均可读。
- 顶部搜索、打开、克隆和初始化动作保持可见；点击回调不改变。
- 真实窗口只检查空状态，不打开仓库、不创建目录、不修改用户 Git 配置。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-tool-vcs`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚版本管理空状态的显示结构、对应布局测试和本节记录；不回滚仓库扫描、Git 操作、用户配置或其他工具页面。

**验收记录（2026-10-27，完成）：**Computer Use 在 `1202x774` 真实窗口检查版本管理空仓库页。亮色和暗色主题均显示 Git 分支图标、“还没有打开的仓库”、下一步说明和“打开”按钮；搜索、打开、克隆和初始化动作仍在顶部工具栏内。没有打开仓库、创建目录或修改 Git 配置。Headless GPUI 在 `360/640/1024/1440` 宽度检查空状态、图标、说明和动作边界；`cargo test --locked -p ramag-tool-vcs` 的 189 项测试通过，5 项手动性能观察按既有规则忽略；`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

### RAMAG-UI-001G：容器连接失败与 WSL Docker 引导

**范围：**在容器管理页保留现有 Docker 连接失败原文，并为 Windows named pipe 失败增加独立的 WSL Docker 连接建议卡。建议卡说明 SSH 端点格式 `ssh://<WSL用户名>@localhost`，与上方端点输入框和“连接”动作形成连续层级。不改变 Docker 连接协议、只读权限、刷新回调、错误分类、凭据或 WSL 服务配置。

**验收条件：**

- named pipe 连接失败时，错误状态、WSL 建议标题、操作说明和 SSH 端点示例按顺序显示；建议内容不再被压缩为逐字竖排。
- `360`、`800` 和 `1440` 宽度下，错误区和建议卡都位于容器内容区内；亮色和暗色主题均可读。
- Docker 地址输入框、连接按钮、资源导航和“连接 Docker Engine 后显示概览”空状态保持可见；不自动发起新的 Docker 操作。
- 其他端点类型的错误只显示原有错误提示，不显示 WSL 专用建议。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-tool-container`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚容器连接错误的显示结构、WSL 引导文案、对应布局测试和本节记录；不回滚 Docker 适配器、SSH 传输、连接配置、凭据或其他工具页面。

**验收记录（2026-10-27，完成）：**Computer Use 在 `1202x774` 真实窗口检查容器管理页。亮色和暗色主题均显示原始连接失败提示、独立的“WSL Docker 连接建议”卡、`ssh://<WSL用户名>@localhost` 示例和“连接 Docker Engine 后显示概览”空状态；地址输入框、连接按钮和资源导航保持可见。检查没有修改 Docker 地址、发起连接、创建资源或改动 WSL 服务。Headless GPUI 在 `360/800/1440` 宽度检查错误区、建议卡、标题和端点示例边界，并确认 SSH 端点只保留原有错误提示；`cargo test --locked -p ramag-tool-container` 的 38 项测试通过；`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

### RAMAG-UI-001H：设置页空配置卡层级

**范围：**为版本管理和云存储设置页共用的空配置卡增加信息图标、状态标题、说明和下一步提示。保留页面标题、模块名称、设置导航和工具页配置入口。不新增 Git、对象存储或凭据设置字段，不改变设置保存与工具行为。

**验收条件：**

- 空配置卡显示模块标题、信息图标、“暂无通用设置”、说明和下一步提示，内容按顺序排列。
- `360`、`1024` 和 `1440` 宽度下，卡片和全部层级元素位于设置内容区内；亮色和暗色主题均可读。
- 版本管理与云存储页面使用同一套层级；设置导航和页面滚动行为不改变。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚设置空配置卡的显示结构、对应 headless 布局测试和本节记录；不回滚设置存储、工具注册、Git 操作、对象存储凭据或其他设置页。

**验收记录（2026-10-27，完成）：**Computer Use 在 `1202x774` 真实窗口检查版本管理和云存储设置页。亮色和暗色主题均显示模块卡标题、信息图标、“暂无通用设置”、说明和“请在对应工具页面完成具体操作。”提示；设置导航、页面标题和页面滚动区域保持可用。检查没有修改设置、凭据或工具配置。Headless GPUI 覆盖亮色、暗色及 `360/1024/1440` 宽度，检查卡片、标题行、说明和提示的边界与顺序；`cargo test --locked -p ramag-ui` 的 148 项测试通过；`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

### RAMAG-UI-001I：工具显示入口摘要层级

**范围：**在工具显示设置页增加工具总数、首页入口数和侧边栏入口数的只读摘要卡。保留每个工具的首页/侧边栏独立开关、持久化、首页和活动栏同步行为。不新增工具入口，不改变工具注册顺序或可见性规则。

**验收条件：**

- 摘要显示工具总数、首页入口数和侧边栏入口数，并位于说明文字与工具卡列表之间。
- `360`、`1024` 和 `1440` 宽度下，摘要卡和三个摘要指标位于内容区内；亮色和暗色主题均可读。
- 每个工具的两个开关及其独立更新行为保持不变，摘要只反映当前注册表状态。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚工具显示摘要结构、对应布局断言和本节记录；不回滚工具注册、可见性存储、首页卡片、活动栏入口或其他设置页。

**验收记录（2026-10-27，完成）：**Computer Use 在 `1202x774` 真实窗口检查工具显示页。亮色和暗色主题均显示“工具总数”“首页入口”“侧边栏入口”三个摘要卡，当前均为 `10`；工具卡和两个入口开关仍按原顺序显示。检查没有切换开关或修改工具可见性。Headless GPUI 在 `360/1024/1440` 宽度检查摘要容器、三个指标和工具卡边界；`cargo test --locked -p ramag-ui` 的 148 项测试通过；`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

### RAMAG-UI-001J：关于页产品身份与更新状态层级

**范围：**为关于页增加 Ramag 产品身份区和基于现有 `UpdateUiState` 的更新状态徽标。保留当前版本显示、更新链接、交流群和问题反馈动作；不改变更新检查、外链地址或发布逻辑，不新增业务入口。

**验收条件：**

- 关于页显示产品名称、原生工作台说明和更新状态；状态与 `Idle`、`UpToDate`、`Available`、`UnsupportedPlatform` 四种现有状态一致。
- 产品身份区、状态徽标、当前版本和外链按钮在 `360`、`1024` 和 `1440` 宽度下位于版本信息卡内；长版本号可以换行。
- 亮色和暗色主题下，状态颜色、文字和按钮保持可读；现有外链动作和更新状态来源不改变。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚关于页身份区、状态徽标、对应布局测试和本节记录；不回滚更新服务、发布资产、外链地址、其他设置页或工具入口。

**验收记录（2026-10-27，完成）：**Computer Use 在 `1202x774` 真实窗口检查关于页。亮色和深色主题均显示 Ramag 产品身份、原生工作台说明、`尚未检查` 状态徽标、当前版本和两个外链按钮；没有打开外链或修改更新配置。Headless GPUI 在 `360/1024/1440` 宽度检查身份区和状态徽标边界，另以四种 `UpdateUiState` 检查状态文字映射；`cargo test --locked -p ramag-ui` 的 150 项测试通过；`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

### RAMAG-UI-001K：剪贴板设置状态摘要层级

**范围：**在剪贴板设置页的功能开关前增加功能状态、抽屉热键和图片采集的只读摘要卡。保留启用开关、备用热键、图片采集、自动粘贴、历史清理、保存状态和错误反馈行为；不读取或显示剪贴板内容，不改变快捷键和存储逻辑。

**验收条件：**

- 摘要显示当前功能状态、实际抽屉热键和图片采集状态，并位于错误提示与具体开关卡之间。
- `360`、`1024` 和 `1440` 宽度下，摘要卡、指标区和三个指标均位于内容区内；指标可以在窄窗口换行。
- 亮色和暗色主题均可读；开关、持久化、历史清理和失败提示的交互路径不改变。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚剪贴板摘要结构、对应布局测试和本节记录；不回滚剪贴板服务、快捷键注册、历史数据、设置存储或其他设置页。

**验收记录（2026-10-27，完成）：**Computer Use 在 `1202x774` 真实窗口检查剪贴板设置页。亮色和深色主题均显示“功能状态”“抽屉热键”“图片采集”三个摘要指标，功能开关和页面说明仍可见；检查没有切换剪贴板开关、清理历史或修改快捷键。Headless GPUI 在 `360/1024/1440` 宽度检查摘要卡、指标区和三个指标边界；`cargo test --locked -p ramag-ui` 的 151 项测试通过；`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

### RAMAG-UI-001L：监控预设空状态层级

**范围：**为系统设置中的命名监控预设空列表增加信息图标、状态标题和保存引导。保留内置预设、名称输入、保存、应用、覆盖、重命名、删除、确认和错误提示行为；不改变监控采样、展示偏好或预设存储格式。

**验收条件：**

- 没有命名预设时显示图标、标题和说明，说明指向已有的名称输入和保存动作。
- 空状态卡在 `360`、`1024` 和 `1440` 宽度下位于预设卡内，标题和说明不越界、不重叠。
- 亮色和暗色主题均可读；内置预设按钮、输入框和保存按钮保持可用。

**验证命令：**`cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、`git diff --check`。

**回滚边界：**只回滚监控预设空状态结构、对应布局测试和本节记录；不回滚监控采样、展示偏好、预设存储或其他系统设置。

**验收记录（2026-10-27，完成）：**Computer Use 在 `1202x774` 真实窗口滚动到系统设置底部，亮色和深色主题均显示预设空状态卡、信息图标、“暂无命名预设”和保存引导；内置预设按钮、名称输入和“保存当前监控状态”按钮保持可见。检查没有保存、应用、重命名或删除预设。Headless GPUI 在 `360/1024/1440` 宽度检查空状态卡、标题和引导边界；`cargo test --locked -p ramag-ui` 的 152 项测试通过；`cargo fmt --all -- --check`、严格 workspace Clippy、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

### 当前主线完成后的插件平台化路线

“当前主线完成”指当前执行队列和已启动的专项计划均满足各自验收条件。后续新出现的维护问题
继续单独排序，不无限期阻塞插件化阶段。连接型工作区全局 UI 回归已完成；其余已排期的维护和发布
任务和数据库 `A-UI-REAL` 仍按各自验收条件处理，再进入下列阶段。

| 顺序 | ID | 内容 | 完成条件 |
|---|---|---|---|
| 后主线 1 | `RAMAG-PLUGIN-DESIGN-001` | 盘点现有静态插件宿主，设计稳定的插件接口和平台/插件边界；评估静态内置、WASM、运行时动态加载和进程外插件等承载方式 | 形成经评审的接口规范和决策记录；定义身份、版本、贡献点、生命周期、能力授权、设置与秘密、任务取消、资源上限、诊断、兼容和分发边界；明确哪些承载方式进入实现 |
| 后主线 2 | `RAMAG-PLUGIN-PILOT-001` | 按接口规范选择一个现有低风险能力作为试点，交付开发样例和完整插件工作流 | 试点能完成注册、入口显示、生命周期、设置、权限、失败隔离和卸载；现有行为、用户数据和工具顺序保持兼容 |
| 后主线 3 | `RAMAG-PLUGIN-MIGRATE-001` | 按功能批次把保留的现有工具收敛为插件模块，减少业务工具对主程序装配细节的依赖 | 每个批次独立验收和回滚；现有协议、数据、凭据和用户流程不回退；平台核心只保留通用宿主能力 |

插件接口冻结后，所有新增业务功能都必须以插件模块交付。平台核心只接受跨插件共用的注册、生命周期、
安全授权、设置、任务和 UI 宿主能力。插件化不改变 `it-tools-template` 与 Ramag 的独立关系，两个项目
不共享进程、代码包、接口协议或发布物。动态插件、第三方不受信任代码、远程市场和自动更新只有在
接口设计阶段形成安全与兼容决策后，才可进入后续实现范围。

## 5. 验收要求

- Rust 变更执行 `cargo fmt --all -- --check`、严格 Clippy、受影响 crate 测试和
  `cargo build --locked -p ramag-bin`。
- UI 变更分别记录真实窗口、headless 和静态检查结果；启动成功不等于交互验收完成。
- 数据库和消息集成只使用本机 Docker；不适用时明确记录“不适用”。
- 保留功能的旧配置、用户数据和凭据不因移除两个工具而自动删除。
- 文档必须区分当前路线和历史记录。历史记录保留在历史计划和归档中，不作为新的开发任务。

## 6. 回滚边界

本次移除的回滚边界是 `RAMAG-REMOVE-001` 的代码、工作区清单、工具注册、存储表引用、
脚本和文档。若发布版本仍需要读取旧数据库，保留数据库文件和其他工具的数据；不自动恢复
本机协作或 JSON Path 提取器，也不删除用户数据。恢复这两个工具必须得到用户的明确说明。
