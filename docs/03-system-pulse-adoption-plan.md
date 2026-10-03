# System Pulse 采集替换与 Ramag UI 吸收计划

> 状态：2026-10-03 重新基线：System Pulse 完整迁入、监控设置收口和公共样式首轮推广已完成；当前进入逐工具真实窗口验收与差异修复。
> 设计确认：用户于 2026-10-02 回复“确认，按此方案执行”；2026-10-01 的原吸收方案保留为历史记录。
> 当前主线：以完整编译的 Ramag 程序逐工具核对真实运行界面和功能状态，把 `system-tool` 作为公共视觉基准；先修已知布局、字体和状态问题，再按工具分片推广，动效与性能专项后置。
> 来源：https://github.com/eas4ai/system-pulse/tree/f1be5d51d24c21fa8c740be79200bdda3df3a00c
> 优点吸收与剩余差距：[`04-system-pulse-gap-matrix.md`](04-system-pulse-gap-matrix.md)

## 2026-10-03 当前进度重排与执行队列

System Pulse 固定来源的完整监控实现和十页 UI 已迁入 `ramag-tool-system`，用户已完成该 UI 的迁入验收。System Monitor 的 Appearance、Sampling 和 Presets 已收口到 Ramag 全局 Settings，监控页签和重复的工具级设置入口已移除。主程序 Summary 的 CPU/内存非零读数、图表纵轴和 SSH 本机 WSL 连接状态也有代码及运行验收记录。

公共 Pulse 样式首轮已覆盖 Shell、首页、Settings、DBClient、SSH、VCS、容器、对象存储、API、Kafka、MQTT、剪贴板、JSON Path 和协作页面。现有提交证明这些切片已接入共享标题、面板、状态或列表组件；headless 测试和编译证明的范围仍限于相应布局、交互和构建，不能代替用户要求的逐工具完整程序验收，也不能据此宣称所有视图已经对齐。

四个具体 UI 切片已完成代码、测试和本机窗口核对，并已独立提交推送：容器资源表占满剩余窗口高度、长列表在表格视口内纵向及横向滚动，并统一字体、字号和背景；插件诊断与标准插件入口采用公共页面标题、面板和状态表达；VCS 文件模式按钮保持横向成组，分支选择器与搜索操作分层，未选文件时使用 Pulse 面板表达空状态；DBClient 连接列表填充剩余视口并保留空状态入口。容器提交为 `72ce4f18`，插件 UI 提交为 `1b17e891`，VCS 提交为 `f0c36752`，DBClient 列表提交为 `336e5228`。对象存储配置账号列表高度已通过合成账号的 headless 验收，真实窗口目前没有已保存账号，配置列表及 Bucket/对象/传输流程继续标为未验收。

| 顺序 | 阶段与边界 | 完成条件 |
|---|---|---|
| 0 | **已完成的 UI 切片**：容器表格视口高度、纵向/横向滚动及 System-tool 字体/字号/背景；插件目录、诊断状态和标准入口视图；VCS 文件模式行、分支选择器、搜索操作和 Diff 空状态；DBClient 连接列表视口高度和新建空状态入口。每个工具仍保留自己的领域语义与操作。 | `72ce4f18`、`1b17e891`、`f0c36752`、`336e5228` 已推送 `main`；容器 31 项、`ramag-ui` 132 项；VCS 188 项通过、0 失败、5 项性能观察用例 ignored；DBClient 369 项通过；workspace Clippy、格式、源码尺寸、完整程序构建和差异检查通过；原生窗口查看容器列表、插件目录、VCS 仓库/文件、DBClient 结果表及连接列表。 |
| 1 | **逐工具完整程序验收**：从仓库源码构建并运行 `target/debug/ramag.exe`，关闭旧 Ramag 实例，只保留一个当前构建；从首页逐个打开工具，并先检查列表/表格型工作区。当前源码构建和 Computer Use 已恢复，可直接从该基线继续。 | 记录实际窗口版本、尺寸、主题、入口和可操作流程；每个工具单独记录符合项、可见差异和未验收项。启动成功、headless 或静态截图不替代交互验收。 |
| 2 | **按差异修复主要工作区**：先完成表格/列表型工作区（容器、VCS、对象存储、DBClient、剪贴板、插件），再完成编辑/连接型工作区（SSH/SFTP、API、Kafka、MQTT），随后检查 JSON Path、协作和其余已注册工具。此顺序可根据完整程序发现的阻断问题调整，但每次只做一个可验收切片。 | 不只补标题：逐页检查字体、字号、背景、间距、标题层级、主要内容高度、滚动、空/加载/失败/已连接状态和主要操作；保留各领域的对象导航、编辑器、数据表、危险确认和异步行为。 |
| 3 | **核对 Settings 职责和重复项**：应用级主题、字体、字号、滚动条、窗口和系统监控采样由全局 Settings 管理；工具独有的连接/查询/协议选项留在其业务上下文。只移除重复入口，保留存储键和旧配置读取兼容。 | 有真实重复项时记录入口、状态来源和迁移兼容测试；若没有重复项则标记不适用，不为追求“全部集中”移动领域设置。 |
| 4 | **全局回归和交付**：逐工具修复后重新构建完整程序，复核首页往返、主题、全局设置、状态反馈和跨工具共享组件；协议/数据库行为改变时才启动本机 Docker 服务。 | 对受影响 crate 运行目标测试和 Clippy；公共组件或跨工具改动运行 workspace 测试、fmt、workspace Clippy、源码尺寸、LF 与 diff 检查。真实窗口证据与 headless/服务证据分开记录。 |
| 5 | **后置体验专项**：页面动效、系统减少动画偏好和发布性能采样排在功能、布局、加载/失败恢复与逐工具视觉核验之后，不作为这些切片的前置条件。 | 仅按 `PULSE-M01`、`PULSE-P01` 的独立证据宣布完成，不用静态截图、调试构建或空闲帧替代。 |

回滚以单个工具渲染模块和对应测试为界；修改公共 `ramag-ui` 组件时，同时验证所有实际消费者。System-tool 是字体、颜色、密度和反馈的参考，不要求数据库、终端或编辑器复制它的监控业务布局。列表在页面有剩余空间时填充视口，长内容在列表内部滚动；小于原生最小客户区的 headless 尺寸只作为响应式防御测试。

## 术语表与命名约定

| 规范中文名 | English / Acronym | 职责边界 | 不代表什么 |
|---|---|---|---|
| 系统采集器 | System Collector | 读取本机硬件、计数器和进程数据 | 不负责 GPUI 渲染 |
| 系统快照 | System Snapshot | 一次采样产生的独立设备、传感器和诊断记录 | 不代表持久化日志 |
| 传感器读数 | Sensor Reading | 带稳定 ID、物理单位、来源时间和状态的单项指标 | 不代表把缺失数据估算为零 |
| 稳定进程身份 | Stable Process Identity | PID 与平台精确启动时间组合，发送信号时重新核对 | 不代表进程名称或秒级启动时间 |
| 系统监控视图 | System Monitor View | Ramag 内的 GPUI 页面、交互和偏好 | 不代表另一个应用窗口 |
| 源码迁入 | Source Port | 将固定来源的完整监控实现与资源纳入 Ramag，只调整宿主和依赖接口 | 不代表重写相似页面或运行外部程序 |
| 宿主适配 | Host Adapter | 对接工具注册、配置目录、主题、窗口尺寸和生命周期 | 不负责重新设计监控页面 |
| 公共 UI 推广 | Shared UI Adoption | 监控验收后，将其经验证的视觉规则和组件用于其他工具 | 不代表其他工具采用监控的业务布局 |

## 2026-10-02 完整源码迁入方案

### 问题与源码依据

用户明确允许删除 Ramag 当前系统监控，不要求复用。此前以 Ramag 组件重建近似页面，保留了不同的标题、图表、量表和布局规则，造成源码功能存在但真实界面仍不一致。本次改为先完整迁入，再适配宿主，最后推广公共 UI。

本机来源为 `F:/project/system-pulse`。2026-10-02 读取 `git rev-parse HEAD` 确认为 `f1be5d51d24c21fa8c740be79200bdda3df3a00c`，`git status --short --branch` 显示来源工作区干净。来源只作读取和独立运行参考，迁入后 Ramag 不依赖该绝对路径。

`src/screens.rs` 的 `ApplicationView` 拥有 `WorkspaceView`，后者连接模型、采样器、历史、进程操作、预设和持久化；只迁移 `screen_summary.rs` 不能构成完整功能。`screen_style.rs` 提供独立配色和 Michroma 标题，`assets.rs` 内嵌五种字体。`tray.rs` 设置默认客户区 `1280x880`、最小客户区 `960x640`。这些都是来源实现的一部分，不再按旧方案排除。

来源使用 GPUI Kit `=0.6.1`，Ramag 使用 `0.6.6`。两者不能直接交换 GPUI 实体和控件类型；需把源代码纳入 Ramag，统一依赖图并解决必要 API 差异。来源的 `crates/base`、`crates/ui` 和原生补丁先按所用 API 核查，不能以框架升级为由改变页面布局或删掉行为。

### 迁入边界

1. 原样迁入十页视图，以及其依赖的模型、控制器、历史、图表、量表、进程表、设置、预设、存储和字体资源。保留来源目录关系、默认值、数据单位、状态规则、许可证和来源记录；避免在迁入阶段抽象或重新设计。
2. `ramag-tool-system` 继续以工具 ID `system` 接入 Ramag。新页面由 Ramag 组合根创建，运行在同一 GPUI 应用和窗口内；来源的 `main.rs`、独立应用启动和独立托盘不进入工具入口。
3. 删除现有 `SystemView`、其页面实现及仅供旧监控使用的状态和组件。先核对调用者，再移除公共库中没有其他消费者的旧监控代码；既有 Shell、首页和其他工具的修改不随之删除。
4. 将来源模型和采样接口与 `ramag-infra-system` 核对，所需源码差异随整体迁入处理。监控只启动一个采样服务；Ram​​ag 其他消费者仍从基础设施接口取得数据，不能为兼容旧页面保留另一套监控状态机。
5. 保存来源配置格式和恢复行为，配置根目录由 Ramag 注入，避免读取或覆盖独立 System Pulse 的用户配置。主题与字体注册对接 Ramag 宿主；来源主题选项映射到宿主设置，界面配色和字体结果与来源一致。
6. 窗口最小客户区以来源 `960x640` 内容空间为基准，加上 Ramag 活动栏和页头占用后计算宿主最小尺寸。响应式分支使用监控实际可用内容宽度，不能直接把整个窗口宽度当作监控宽度。保留来源的断点、重排、滚动和稳定控件尺寸。
7. 采样、历史和持久化任务随宿主生命周期启动、暂停或释放；进程操作仍校验完整身份并保留确认、取消、授权失败和超时反馈。主窗口不自行提升权限，不复制第二个托盘。

### 实施顺序与提交边界

| 阶段 | 可审查结果 | 验收后提交条件 |
|---|---|---|
| 1. 固定来源与完整迁入 | 来源清单、完整页面及其依赖、资源和必要宿主适配；新监控替换旧监控 | 十页可运行，目标测试通过，真实窗口逐页对比通过；不能把仅可编译的骨架或部分页面当作完成提交 |
| 2. 宿主行为核验 | 最小窗口、响应式内容空间、主题同步、配置恢复、采样释放和工具往返 | 对应原生操作与生命周期测试通过；阶段 1 必需的接入行为应在阶段 1 一并完成 |
| 3. 公共 UI 推广 | 从已验收监控提取实际共用的字体、颜色、标题、导航、图表和表格规则，逐工具替换 | 每个独立工具或组件单独验收、提交并立即推送；业务工作区按各工具职责保留 |

阶段 1 的功能是“完整参考监控在 Ramag 内可用”，不是十个重新实现的页面。开发期间可以逐模块编译和核查，但替换未完整、测试失败或真实窗口验收未通过时不提交、不推送。阶段 3 在阶段 1 和阶段 2 通过后开始。

### 验收方法

验收先关闭旧 System Pulse 实例，再启动固定来源的参考程序，始终只保持一个参考实例。参考与 Ramag 使用隔离配置，记录构建、缩放、客户区及实际监控内容尺寸、主题和采样周期。按相同内容尺寸比较，避免 Ramag 壳层占用导致错误的断点判断。

Computer Use 按 Summary、CPU、Memory、GPU、Disks、Network、Energy、Thermals、Processes、Settings 顺序操作参考程序，再在 Ramag 复现同一流程。每页核对标题、字体、配色、区域比例、量表、图表网格、单位、表格、导航、滚动、设备选择和不可用状态；实时数值不要求同一时刻相等，核对数据来源和含义。进程操作只使用本次测试拥有的进程；主线移植已有行为，不另加键盘增强作为迁移目标。

记录每页通过、差异或未完成，不以截图相似、标签数量或测试总数宣布完成。最小窗口、默认窗口、宽窗口及明暗主题分别检查；低于原生最小尺寸的 headless 情况属于防御检查，不代替原生布局验收。Computer Use 失败时按用户恢复流程排查，无法恢复才使用截图和 headless，并明确未覆盖的原生操作。

最终依次执行风险匹配测试、UI 验收、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` 和差异/LF 检查。纯监控和本地配置不使用外部协议服务，Docker 不适用；后续其他工具涉及服务时使用本机 Docker。全部适用检查通过后才提交并立即推送 `main`。

### 2026-10-02 设置迁移与公共样式推广切片

用户已完成 System Pulse 十页真实窗口验收，后续主线从监控页面迁移转为设置和公共 UI。第一步把来源 Settings 的三块职责迁入 Ramag 全局 `SettingsView`，保留 Ramag 已有的存储键和工具级设置范围：`Appearance` 对应系统设置，`Sampling` 对应系统监控设置，`Presets` 对应监控预设管理。迁移只改变页面组织、标题层级、字体、面板、分段控件和反馈状态，不复制独立应用窗口或另建存储。

本切片先完成 `Appearance` 页面：使用 System Pulse 的双栏响应式面板、Michroma 分区标题、紧凑边界、选中/悬停/焦点状态和排版预览；主题、界面字体、数值字体、字号、滚动条和托盘行为继续通过 Ramag 全局设置保存。验收覆盖明暗主题、`360x640`、`1024x768`、`1440x900`、最大字号、键盘焦点可见性和设置回读。随后独立完成 `Sampling + Presets` 的同样布局迁移，再提取设置面板、分段按钮、状态提示和页标题为公共样式，按数据库、对象存储、SSH、VCS、剪贴板、插件顺序推广。

不在本切片中改动其他工具的业务流程、连接状态、查询结果、危险操作或存储协议；公共样式推广必须逐工具保留原有领域颜色和状态语义。每个设置或工具切片单独测试、UI 验收、提交和推送，未通过真实窗口或替代证据范围不扩展为整体 UI 完成。

### 原迁入计划的历史边界

本节是已确认的替换设计，不是实施或验收完成记录。上一轮 Summary 和字体修改已经保留本地备份；迁入时逐项盘点有效来源资源，移除被整体替换的旧页面修改。

以下原吸收计划、历史提交和验收记录保留供追溯。本节优先于其中“保留旧监控视图”“独立实现近似 UI”“不复制字体资源”等冲突约定；历史通过结果不自动证明完整源码迁入已经验收。

### 2026-10-03 全应用公共样式推广

设计基准为当前已验收的 `ramag-tool-system/src/screen_style.rs`，不是旧的 Ramag 主题或新设计稿。先把其明暗配色、Michroma 标题、数值字体、8px 面板边界和紧凑间距收口到 `ramag-ui`，让系统监控与其他工具消费同一来源；按钮、输入、菜单、表格、弹窗和滚动条由宿主主题同步。连接状态和危险操作继续使用真实领域语义，不把监控的业务布局复制到编辑器或终端。

实施顺序为公共视觉规则、壳层/首页/全局设置、数据库/对象存储/SSH 已有改动复验，然后 VCS、容器、API、Kafka、MQTT、剪贴板、JSON Path、协作和插件入口。每个工具保留其对象树、编辑器、结果表、快捷键、确认和异步状态；页面、工具栏、列表、空/加载/失败状态和主要工作区都需核查，不能以增加一个标题宣布整个工具完成。

验收覆盖明暗主题、标准/最大字号和 `360x640`、`1024x768`、`1440x900` 的 headless 布局与控件回归。原生使用本工作区新构建的 `target/debug/ramag.exe`，从首页逐工具进入、往返、滚动和操作；不用安装版或 `ui-preview`。只读样式改动不新增外部服务，Docker 不适用；SSH 本机 WSL 连接单独标为本机运行证据，不算 Docker 集成。

最终检查运行 `cargo test --locked --workspace -- --test-threads=1`、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin`、源码尺寸和差异/LF 检查。发现 lint 或布局失败应修复源码/测试，不压低 lint。回滚边界按各工具渲染文件、公共主题和对应测试划分，保留此前迁入、配置兼容和无关改动；尚未逐工具验收的部分继续列为未完成。

## 实施顺序与接口

1. `A-CI-VCS-APP-CONTEXT`：独立修复 macOS 视觉测试的 `AppContext` 导入，目标测试和最终 fmt/Clippy 通过后推送 `main`。提交 `e807ff36` 已完成。
2. `A-SYSTEM-PULSE-001`：将指定提交的完整采集源码和测试纳入 `ramag-infra-system`，保留 Linux proc/sysfs、Intel/NVIDIA、Windows GPU/EMI/温度和 Apple Silicon 后端。平台内部依赖归该 crate 所有；业务 crate 和根 workspace 移除原 `sysinfo` 声明与直接调用。
3. `A-SYSTEM-PULSE-UI-001`：系统监控采用固定十标签和公共 Pulse 组件，完成概览、分领域读数、真实时间图表、设备选择、进程查询/排序/终止确认、采样和传感器显隐设置。
4. `A-PULSE-SHELL-001` 及各工具切片：系统监控验证后，逐项推广标题层级、边界、状态、间距和表格规则至 Shell、首页、设置及现有工具。

提交边界先建立独立的公共 Pulse 组件与有界展示偏好，再提交采集器、应用内存适配和系统监控接入。后一个功能同时替换采集与视图，避免中间提交使用不兼容的旧快照字段。

系统采集器返回拥有所有权的系统快照。`SystemMonitor` 持有一个采集工作线程和有界缓存，GPUI 定时器只取最新快照，不读取操作系统或等待采集。手动刷新请求合并；慢后端跳过错过的采样周期，避免补采风暴。视图释放后工作线程停止，最终线程等待在清理线程执行。

图表按来源时间绘制，保留 60 秒且每条曲线最多 120 点、最多 4096 条曲线。当前、预热、不可用、失败和过期分别显示；非当前样本产生曲线缺口。旧刷新偏好键 `monitor_settings` 和 1/2/5 秒档位保持兼容；设备选择和隐藏传感器使用独立有界键 `monitor_presentation_settings`。显式选择的设备消失后显示不可用，不能替换为另一个设备。

应用内存统计也通过系统采集器读取当前进程，不采集进程参数或环境变量。进程确认保存完整稳定进程身份，刷新或重新排序不改变目标；Linux 使用 pidfd，Windows 使用进程句柄与完整创建时间，macOS 使用平台身份校验。自进程和未验证身份拒绝操作。强制结束需要确认，并显示原始操作错误。Windows 温度读取仅在用户显式启用后调用受限 helper；普通窗口保持原权限，缺少 PawnIO 或硬件不支持时说明原因，不自动安装驱动。

用户要求移除原 `sysinfo = { workspace = true }` 声明。根 workspace、`ramag-app` 和 `ramag-tool-system` 已移除这些声明和直接调用；两者只消费本地系统采集器。指定来源提交的采集器自身依赖 `sysinfo = "=0.37.2"`，适配后仅在 `ramag-infra-system` 内使用 `=0.38.4`。该底层依赖保留，不能将本次替换描述为整个依赖树不再包含 sysinfo。

## 视觉规范

固定顺序为 Summary、CPU、Memory、GPU、Disks、Network、Energy、Thermals、Processes、Settings。导航使用原生可聚焦控件，窄窗口重排，页面始终可到达。Summary 展示 CPU、内存、进程和子系统趋势；分领域页面用设备或传感器名称、单位、来源范围和状态表达真实数据。对硬件缺失保留页面和原因。

`ramag-ui` 提供页面标题、标签栏、设备选择、指标卡、轻量面板、状态提示和带缺口的时间图表。颜色读取当前主题，保留 Ramag 明暗主题；圆角不超过 6px。页面区域保持自然布局，卡片只包裹独立指标。复杂工具继续使用必要的对象导航和编辑工作区，应用相同视觉层级和状态规则。

用户于 2026-10-01 补充整体界面需要达到 System Pulse 的鲜明、流畅观感。基准采用指定来源提交的 `assets/screens/summary-dark.jpg`、`summary-light.jpg` 及实际设置截图：紧凑数据组合、细密网格、带颜色的物理读数、明确选中标记和及时交互反馈。后续切片必须改变页面的信息组合与动态表现，不能只替换标题或减淡边框。

| 视觉职责 | 实施规则 | 验收重点 |
|---|---|---|
| 数据强调 | CPU/磁盘用绿色、内存用紫红、网络/GPU用蓝青、能耗用黄色、温度用橙色；背景保持中性，浅色主题使用足够深的读数颜色 | 各领域颜色可区分且文字可读，不把错误色当装饰 |
| 趋势表达 | 实际采样时间、细密但低对比的网格、轻透明面积和清晰折线；缺失样本保留断点 | 正常/预热/失败/过期仍准确，不能用插值伪造采样 |
| 页面组合 | 减少重复页头和空白分区，主要读数、图表与相关表格紧邻；保留工具的领域导航 | 三个窗口尺寸均能扫描主要对象与状态 |
| 过渡与反馈 | 页切换采用 120..180ms 的短淡入，位移不超过 6px；选中标记、焦点和悬停明确；刷新不重复触发整页动画 | 不阻挡点击，不移动命中范围，遵守系统减少动画偏好 |
| 资源使用 | 动画只随操作执行，结束后停止重绘；监控采样与 UI 绘制继续分离 | 静止界面不持续动画，响应式布局和存储结果不受影响 |

## 来源、许可证与发布

`ramag-infra-system` 保留 System Pulse 的 GPL-3.0-or-later 许可证、来源提交和版权说明；Apple、Intel UAPI 与 PawnIO 的来源声明随代码保留。Ramag 自有代码继续沿用 AGPL-3.0-only。可选 PawnIO IntelMSR 模块保持原签名字节，并携带对应源文件、源归档和 LGPL 文本。修改记录写明与上游的差异：本地模块分拆、现有 sysinfo 版本适配、精确进程身份和手动刷新接入。

发布前检查应确认对应许可证和来源记录包含在发行包或对应源代码中。此次开发不重新发布安装包，不宣称已经完成驱动安装或全部硬件精度验收。

## 验收条件与记录

整体 UI 必须逐页满足 [`01-development-plan.md`](01-development-plan.md) 的 `PULSE-V01` 至 `PULSE-G01`，系统采集替换采用其 `2.4` 的 `PULSE-C01` 至 `PULSE-C05`，当前动效、采样工具和设置切片采用其 `2.5` 的独立条件；视觉基准、图例和操作边界见 [`07-ui-acceptance-standard.md`](07-ui-acceptance-standard.md) 的 `2.0.1` 至 `2.0.4`。每个切片先更新验收条件再开发。已有采集接入和 UI 提交的验证仍按当时范围保留；新整体视觉、动态和资源条件未复验前不能宣称全部完成。当前三个切片处于未提交实现阶段，需按新条件继续验证并分别提交。

- 采集：移植测试覆盖 proc/sysfs 算术、计数器重置/预热、设备发现、来源时间、权限失败、进程身份与 PID 复用；实际硬件缺失不能记录为硬件准确性通过。
- 适配：测试来源状态、非有限数值、缺失读数、历史上限、重复快照、过期与身份锁定。
- UI：亮暗主题分别覆盖 360x640、1024x768、1440x900，确认区域追加 360x240；验证所有标签、设备变化、设置保存、查询/排序、取消和长文字边界。
- 原生流程：优先使用 Computer Use 运行隔离 UI 预览，完成打开、切换、滚动、查询和设置流程。失败时记录具体错误、恢复尝试和替代证据，不能用静态截图或 headless 结果代替原生验收结论。
- 质量检查：目标测试、workspace 测试、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、源码尺寸和差异/LF 检查；推送后复核 Linux/macOS/Windows CI。
- 外部服务：本切片没有数据库或协议变更，Docker 服务、镜像、端口、启动与清理均为不适用。
- 公共组件验收：`d2710bf6` 已推送 `main`。Windows `cargo test --locked -p ramag-ui --lib -- --test-threads=1` 的 114 项测试通过；Pulse 组件测试覆盖明暗主题、窄/宽窗口、命中区域、时间轴缺口和最多 120 点。展示偏好测试覆盖旧刷新配置兼容、无效 ID 和容量边界。

### 2026-10-01 系统监控接入验证

| 验证范围 | 命令或证据 | 结果与限制 |
|---|---|---|
| Windows 系统采集器 | `cargo test --locked -p ramag-infra-system --lib -- --test-threads=1` | 111 通过，1 个真实 GPU 测试默认忽略；进程身份、原生进程控制、helper 协议、超时与调用进程退出行为通过 |
| Windows GPU | `cargo test --locked -p ramag-infra-system --lib windows_gpu::native::tests::present_wddm_adapter_provides_current_scheduler_and_memory_readings -- --ignored --exact` | 本机 WDDM 调度器利用率和 GPU 内存测试通过；不代表已核对所有厂商、温度或功率的硬件精度 |
| Windows 系统监控 | `cargo test --locked -p ramag-tool-system --lib -- --test-threads=1` | 16 通过；headless 包含三个要求尺寸、明暗主题十标签、设备缺失/选择、显隐、刷新、360x240 确认与稳定进程身份 |
| Windows workspace | `cargo test --locked --workspace -- --test-threads=1` | 默认测试和文档测试通过；其他工具的环境变量控制或忽略的集成测试不计为 Docker 集成证据 |
| Linux 系统采集器 | WSL Ubuntu 24.04 中 `CARGO_TARGET_DIR=target/pulse-linux cargo test --locked -p ramag-infra-system --lib` 和该 crate 的 all-target Clippy | 161 项测试通过，Clippy 通过；没有在 Linux 原生桌面运行 GPUI |
| macOS 系统采集器 | 分别针对 `aarch64-apple-darwin`、`x86_64-apple-darwin` 执行该 crate 的 `cargo clippy --locked --all-targets -- -D warnings` | 条件编译路径和测试源码通过；未在 macOS 主机执行 IOReport、SMC、HID 或进程控制 |
| Rust 与源码检查 | 最终 `cargo fmt --all -- --check`、workspace all-target Clippy `-D warnings`、Windows 源码尺寸脚本、`git diff --check`、修改文本 LF 检查 | 全部通过；新增采集代码不含 Clippy 抑制 |
| 发布脚本 | WSL 中 `bash -n scripts/package-linux.sh scripts/build-dmg.sh`、Linux/macOS 打包逻辑测试 | 通过；许可证文件已接入三平台打包脚本，未重新构建或发布安装包 |

Computer Use 能获取预览窗口及截图，但新鲜窗口状态下点击仍返回 `foreground window did not report a process id`。已尝试重新列出窗口、重新取得窗口、激活和刷新状态，错误仍复现；完整真实窗口鼠标键盘流程未验收。替代证据为上述 GPUI headless 渲染/交互测试和系统截图 `target/ui-fallback/system-pulse-summary-dark-1024x768-20261001.png`。截图对应 `cargo build --locked -p ramag-bin --example ui-preview` 后的 Windows 实际窗口，客户区目标 1024x768、含系统边框 1040x807，显示真实 CPU/内存变化及进程；截图不能证明搜索、终止确认、UAC 或设置持久化的原生操作。预览不读取用户配置或连接，验收后已停止测试进程。删除 `%TEMP%/ramag-ui-preview-6088.redb`、`ramag-ui-preview-28552.redb`、`ramag-ui-preview-22888.redb` 被自动审批检查以 `blocked by policy` 拒绝，三个专属临时文件保留，清理未完成。

Windows 可选温度读取的协议、授权状态和失败路径通过测试；本机没有执行 PawnIO 实际驱动读取。NVIDIA、Linux Intel/AMD 和 Apple 原生硬件准确性仍待各自平台验收。推送后的 GitHub Actions 状态另行核查，不以本机跨平台编译代替远端 CI 结果。

### `A-PULSE-GAP-06-Energy` 设计与验收边界

Energy 页的主传感器选择使用采集器提供的稳定传感器 ID，展示范围限定为一个明确的功率来源；页面同时保留所有可用功率通道的独立卡片，禁止将 CPU 包功率、GPU 板卡功率或其他组件功率相加成系统总功率。无保存偏好时优先选择 `CPU package power`，保存的 ID 消失时保留不可用状态和原因，等待用户主动选择，不静默替换来源。

主区域必须同步展示传感器名称、当前值、`W` 单位、来源、作用范围、量表和有界历史图。当前值为零时保留有效 `0 W`；预热、失败、不可用、过期、快照停滞和非有限读数必须保留状态并抑制实时量表，历史曲线保留已有样本上下文。选择、重建视图和重新读取隔离偏好后都使用同一稳定 ID。

实现先通过 `ramag-tool-system` 的选择解析、菜单点击、保存回读、缺失 ID、快照过期和 `360x640`/`1024x768`/`1440x900` 布局测试，再在同尺寸真实 System Pulse 与 Ramag 窗口执行“打开 Energy -> 打开选择器 -> 选择 CPU package power -> 回读主值和 measured channel”的流程。数值只在同一应用快照内核对；两个实时进程的不同采样时刻不用于硬件精度结论。Docker 不适用，Thermals 单传感器和平台授权另行验收。

### `A-PULSE-GAP-06-Thermals` 设计与验收边界

Thermals 沿用 Energy 的稳定传感器选择持久化，但只接受摄氏温度描述。无保存 ID 时选择有限 `Current` 样本中的最高温度，并按传感器 ID 稳定处理并列；保存 ID 缺失时保留不可用身份，不能静默改选。最热摘要独立于手动选择，预热、失败、不可用、过期、停滞采集和非有限数值都不能参与当前最高值。

页面顺序与 System Pulse 对齐：温度选择器和 Windows CPU 温度授权入口位于标题区，随后显示最热摘要、选中温度的量表和动态物理范围历史图，最后显示全部温度传感器卡片。图表的上下界从有限历史样本计算，负温度和零值保持真实值；菜单有界并可滚动，授权请求执行期间不允许重复提交。CPU 温度授权仍由受限平台帮助程序执行，硬件不支持时显示采集原因。

Headless 必须覆盖稳定 ID、并列、零/负值、NaN/Infinity、所有非 Current 状态、缺失选择、菜单点击和 `360x640`/`1024x768`/`1440x900` 边界。Computer Use 先观察固定提交的 System Pulse Thermals，再在同尺寸 Ramag 窗口执行打开选择器、切换 CPU/GPU 温度、回读最热值与不可用原因、点击授权入口的流程。数值只在同一快照内核对；本机 GPU/CPU 温度精度、其他硬件平台和授权成功路径分别记录，Docker 不适用。

### `A-PULSE-GAP-09-Settings-Appearance` 设计与验收边界

Settings 外观切片只收口应用级显示职责：主题、界面字号、滚动条策略、界面字体和数值字体都由 `SystemSettings` 保存，并在选择后立即应用到 GPUI 全局主题。界面字体提供 `Inter Variable` 与 `IBM Plex Sans`，数值字体提供 `JetBrains Mono` 与 `IBM Plex Mono`；缺失字段继续使用参考程序一致的默认值，未知枚举值拒绝读取并回退为可操作默认配置。

页面保留现有托盘驻留和滚动条设置，在外观分区增加两组字体选择与排版预览。采样周期仍属于监控设置；固定监控预设由 `A-PULSE-GAP-05` 单独保存采样、设备和传感器展示偏好，不把连接、凭据、历史样本或全局外观写入预设。验收必须覆盖配置序列化兼容、明暗主题切换后字体保持、三个窗口尺寸的控件可达性，以及真实 System Pulse 与 Ramag 窗口中选择字体、回读预览和重启恢复的流程。

### `A-PULSE-GAP-10-Settings-Sampling` 设计与验收边界

采样切片把 System Pulse 的 `0.5/1/2/5s` 四档周期贯通到 Ramag 的监控偏好、Settings 选择器、监控采集器和启动恢复。新增 0.5 秒只扩展已有有界枚举，旧的 1/2/5 秒 JSON 继续解析，默认仍为 1 秒；周期变化只更新本机系统监控，不修改主题、字体、设备选择、传感器显隐或历史样本。

设置页和系统监控页必须显示同一当前档位，点击后先更新采集器周期，再保存偏好并显示统一保存状态；采集器拒绝更新时保留旧周期并显示失败原因。验证覆盖 0.5 秒序列化、两处控件一致、实际采样周期更新、启动恢复、三种窗口尺寸和明暗主题；真实窗口按“参考 Settings 选择 0.5s -> Ramag Settings 选择 0.5 秒 -> 回到监控页观察选中档位和更新”执行。固定监控预设已由 `A-PULSE-GAP-05` 单独验收，跨页面可停靠布局不属于 Ramag 的快照范围。

当前补充切片只调整系统监控 Settings 的采样信息组合：在四档按钮上方显示当前周期的大号数值和单位，旁边说明较短周期会增加更新频率及 CPU 使用，按钮下方保留“立即应用并自动保存”提示。读数必须与采集器当前周期同步，不能用独立的页面缓存替代真实状态；这项不改变采样枚举、保存键或采集生命周期。

### `A-PULSE-GAP-05` 固定监控预设设计与验收边界

Ramag 的系统监控由十个固定页面组成，不能把 System Pulse 的可停靠窗口布局直接写入本地状态。本切片把快照边界固定为 `MonitorSettings` 和 `MonitorPresentationSettings`：采样档位、设备选择、传感器选择和显隐集合可以保存；连接、凭据、历史样本、采集器运行态、全局主题/字体和页面布局不进入快照。库使用版本字段、100 项上限、64 字节名称上限和 256 KiB 序列化上限；未知版本、控制字符、内置名称冲突和超限内容拒绝写入。

界面提供 Default、Minimal、GPU Focus、Developer 四个受保护的内置入口，同时保留命名预设的创建、应用、重命名、覆盖和删除。覆盖与删除必须先显示确认区域，取消不改变候选状态，保存失败不发布部分库。内置预设只调整 Ramag 能表达的采样/展示偏好，并明确提示固定页面结构保持不变。

验收先操作固定提交的 System Pulse Settings，再在同尺寸 Ramag 设置页执行相同的内置入口、命名保存和确认取消流程。Headless 断言库边界、序列化和每个动作的实际状态；Computer Use 只把真实窗口中的控件、状态和操作结果记为通过，不把隔离预览或静态截图扩展成生产 redb 重启回读证据。

## 视觉推广设计与已完成切片记录

### 2026-10-02 Settings 统一设计

当前系统监控 Settings 同时提供 Appearance、Sampling 和工具内布局预设；Ramag 全局 Settings 已分别提供应用外观（主题、字号、滚动条、界面/数值字体、窗口行为）以及系统监控采样和传感器展示预设。Appearance 与 Sampling 是重复入口，且写入不同状态：监控工具内 Appearance 写入 `workspace.json`，全局外观写入 `system_settings`；同一刷新周期分别有 Ramag 全局值和工具工作区值。两种预设作用范围不同：全局预设保存采样与传感器展示偏好，工具布局预设保存面板布局，两者不合并或删除。

本切片统一以 Ramag 全局 Settings 管理应用外观与采样：系统监控工具内 Settings 移除 Appearance、Sampling 控件，保留布局预设并提供直接打开 Ramag 系统设置的按钮。全局 System Settings 拥有应用外观、采样及传感器展示预设；工具工作区继续保存面板布局。旧 `workspace.json` 中的 `appearance` 字段暂时保留用于读取兼容，但不再覆盖应用全局主题或字体；序列化时省略此字段，新偏好只写入 Ramag 设置存储。旧命名布局预设格式保持可读取，回放时不修改全局外观。

不在本切片重排其他工具 Settings，也不删除全局设置存储键、监控设置数据或工具布局预设。验收覆盖迁移入口存在且能请求打开 `system` 设置页、监控工具页不再渲染旧 Appearance/Sampling 控件、工具布局预设仍可用、全局设置仍能修改采样和外观，以及旧工作区恢复不会覆盖应用级外观。目标测试、`cargo fmt --all -- --check` 和目标 Clippy 通过后再交付；workspace all-target Clippy 当前被既有测试代码中的 unwrap/expect lint 阻挡；本切片不涉及外部服务，Docker 不适用。

### `A-PULSE-DB-001` 数据库客户端 UI 对齐设计与验收边界

数据库客户端保留现有连接管理、对象树、查询编辑器、结果表和领域回调；本切片只统一视觉边界。`DbClientView` 增加公共页面标题，连接列表使用统一的标题下工具栏与轻量面板，查询工作区复用公共工具栏和状态提示，结果表外框和底部状态栏使用同一主题边界。现有连接标签、Schema 选择、查询执行、事务、筛选、分页、导入导出和失败重试的控件 ID 与回调保持不变。

验收覆盖明暗主题与 `360x640`、`1024x768`、`1440x900` 三种窗口尺寸：页面标题、连接工具栏、对象树工具栏、查询上下文、结果工具栏、结果表和状态栏必须在客户区内，长连接名和长状态文案不得覆盖操作区；已有领域导航和结果表回归测试继续通过。本切片只涉及本地 GPUI 渲染，Docker 不适用；若不改变连接或数据库行为，不启动外部服务。回滚边界为恢复本节涉及的 DBClient 渲染文件和对应 headless 布局测试，不回滚此前系统监控、Settings 或公共 Pulse 组件。

### 2026-10-02 DBClient 切片验证

`A-PULSE-DB-001` 已完成 headless UI 对齐：连接列表增加公共页面标题、响应式工具栏和 Pulse 轻量面板；查询工作区增加公共页面标题；对象树连接状态复用 Pulse 状态标签；连接建立中的占位和配置过期面板复用 Pulse 状态提示。连接标签、Schema、查询、事务、结果筛选、分页、导入导出、重试和既有控件回调未改变。

`cargo test --locked -p ramag-tool-dbclient --lib -- --test-threads=1` 通过 368 项；`cargo test --locked -p ramag-ui --lib -- --test-threads=1` 通过 131 项；`cargo clippy --locked -p ramag-tool-dbclient --all-targets -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check` 通过。连接列表新增 `360x640`、`1024x768`、`1440x900` 布局断言，查询上下文和对象树测试同步检查公共标题/状态边界。Docker 不适用；本轮未完成 Computer Use 原生窗口点击，证据范围为 headless 渲染和交互测试。

### `A-PULSE-OBJECT-001` 对象存储账号管理设计与验收边界

对象存储账号管理主视图采用 `ramag-ui::pulse_ui` 的页面标题、紧凑工具栏和轻量面板：页面标题显示“对象存储”和账号管理职责，标题下方保留账号搜索与新建入口，账号列表使用统一边界、背景和 6px 圆角。账号服务商、只读状态、Bucket 数量、编辑和删除操作继续使用对象存储自己的领域表达与已有回调；本切片不改变连接凭据、账号存储、筛选、创建、编辑、删除和确认流程，也不改动 Settings。

验收覆盖明暗主题与 `360x640`、`1024x768`、`1440x900` 三种窗口尺寸：页面标题、搜索框、新建按钮和账号列表面板必须在客户区内，窄窗口允许工具栏换行，长账号名必须省略且不覆盖操作区；已有账号服务商图标、筛选和账号操作测试继续通过。该切片只涉及 GPUI 本地渲染，不使用 Docker；交付前运行对象存储目标测试、`cargo fmt --all -- --check` 和目标 Clippy。

### 2026-10-02 系统监控设置入口收口

系统监控页签移除 `Settings` 导航项，旧工作区状态中的 `active: "settings"` 在读取时迁移到 `Summary`；旧布局面板和预设数据仍保持读取兼容，不再把它作为监控页签展示。Ramag 壳层移除工具右上角齿轮入口，系统外观、采样和监控预设统一从左下角全局 Settings 进入；主题切换图标保留。

验证使用当前源码完整构建的 `target/debug/ramag.exe`，未将 `ui-preview` 作为最终运行验收程序。完整程序启动窗口标题为 `Ramag — 系统监控`，实际屏幕截图 `target/ui-fallback/ramag-full-system-settings-removal-20261002.png` 显示九个监控页签、无右上角齿轮和保留的左下角全局设置入口。Computer Use 重新初始化后仍返回 `apps: []`，无法绑定原生窗口；截图属于替代视觉证据，不扩展为 Computer Use 鼠标/键盘验收。

`cargo test --locked -p ramag-system-model --lib -- --test-threads=1`（9 项）、`cargo test --locked -p ramag-tool-system --lib -- --test-threads=1`（103 项）、`cargo test --locked -p ramag-ui --lib -- --test-threads=1`（131 项）、`cargo build --locked -p ramag-bin`、`cargo fmt --all -- --check` 和源文件尺寸检查通过。目标 all-target Clippy 仍被工作区既有测试代码中的 `unwrap_used`/`collapsible_if` 告警阻挡，未将其描述为通过；本切片不使用 Docker。

### 2026-10-02 Settings 与 Summary 读数验证

全局 `MonitorSettings` 现在是生产采样器和状态栏的唯一运行来源；系统监控工具内的旧布局预设仍可恢复面板状态，但不会覆盖采样周期。旧 `workspace.json` 和旧布局预设仍可读取 `interval_ms`、`appearance`，保存时不再写回这两个旧字段。Summary 的 CPU 与内存顶部指标直接读取最新历史样本，并由回归测试锁定非零样本不得退化为 `0 %` 或 `0 B`。

目标测试和目标 Clippy 已通过：`ramag-tool-system` 104 项、`ramag-ui` 131 项、`ramag-system-model` 8 项、`ramag-tool-object-storage` 27 项；`cargo fmt --all -- --check` 和 `git diff --check` 通过。workspace all-target Clippy 仍被既有测试代码中的 373 个 `unwrap_used`/`collapsible_if` 告警阻挡，未将该结果描述为通过。当时 Computer Use 无法发现 Ramag 原生窗口，本节仅记录 headless 证据；后续原生窗口验收见 SSH UI 对齐验证。

### 2026-10-02 SSH UI 对齐验证

`A-PULSE-SSH-001` 已完成首个公共 UI 对齐切片：SSH 管理页复用 `pulse_page_title` 和 `responsive_toolbar`，保留搜索、远程会话、JumpServer 导入和新建连接入口；工作区新增连接标题、Endpoint、统一 Pulse 状态标签和返回连接管理入口。`SshSessionState` 到 `PulseStatus` 的映射集中在呈现模型中，连接、终端和 SFTP 领域回调未改变。工作区页头改为响应式纵向布局，主体不会覆盖标题区域。

`cargo test --locked -p ramag-tool-ssh --lib -- --test-threads=1` 通过 85 项；`cargo clippy --locked -p ramag-tool-ssh --all-targets -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check` 通过。当前源码完整构建 `cargo build --locked -p ramag-bin` 通过，运行验证使用 `target/debug/ramag.exe`，未使用 `ui-preview`。

Computer Use 已绑定当前源码构建的唯一 `Ramag — 系统监控` 窗口：Summary 实际显示非零 CPU `7.7 %` 和内存 `21.3 / 47.9 GiB`，确认截图中的 `0 % / 0 B` 回归已修复；切换到 SSH 后，真实窗口显示 `Ramag > SSH 管理`、`SSH 管理` 页面标题、连接/终端/SFTP 副标题、搜索工具栏和空连接状态。当前没有测试连接，不在验收中创建或保存用户连接；工作区页头与六种状态映射由 headless 测试覆盖。

### 2026-10-02 WSL SSH 原生全链路验收

在本机 `Ubuntu-24.04` WSL 中安装并启动 `openssh-server`，保持 WSL 运行后由当前源码构建的 `target/debug/ramag.exe` 连接 `127.0.0.1:22`。应用严格主机指纹校验；通过受信任的 WSL `ssh-keyscan` 将本机指纹写入当前用户 `known_hosts` 后，SSH 表单测试实际返回：`OpenSSH 可用`、`认证 可用`、`执行 可用`、`Terminal 可用`、`SFTP 可用`、`通道 标准 SFTP`、`诊断 可用`、`远端 Linux`、`Shell POSIX`、`路径 POSIX`。

保存连接后，Computer Use 在同一个完整 Ramag 窗口中打开 `WSL 本机` 工作区，确认状态为“已连接”，SFTP 显示 `/home/likanug` 内容并可进入 `.config`，终端显示真实 Ubuntu 登录提示和 POSIX shell 就绪状态；点击加号后第二个终端标签成功创建并显示独立登录提示。没有通过 UI 自动输入远端 shell 命令，终端命令执行语义继续由 OpenSSH 集成测试和现有 headless 测试覆盖；本次原生证据覆盖连接、主机指纹、认证、SFTP 浏览、终端启动和多终端标签。

### 2026-10-02 SSH 标签连接状态修复设计

真实 WSL 工作区已连接，但顶部连接标签仍显示灰点和固定 `SSH`，与页头“已连接”不一致。灰点此前同时依赖文件加载、SFTP 错误和环境标记，并不表示连接状态。本切片将工作区标签与页头统一使用 `SshSessionState` 的六种运行状态和公共 Pulse 状态样式，连接名与关闭入口保留；生产环境警告独立显示，目录加载不覆盖连接状态。终端标签保留编号和退出码，并按该终端自身的运行/退出结果展示状态，不能把其他终端的连接状态当作自身状态。现有工作区状态来自终端启动及生命周期，不新增 SSH 握手确认；因此单个终端仍使用“运行中”等进程状态文案，SFTP 可用性与终端生命周期分别记录。

验收覆盖六种工作区状态、终端正常/异常退出、目录加载不影响已连接标签，以及明暗主题下 `360x640`、`1024x768`、`1440x900` 长连接名称与操作边界。运行 `cargo test --locked -p ramag-tool-ssh --lib -- --test-threads=1`、`cargo clippy --locked -p ramag-tool-ssh --all-targets -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check`；构建 `cargo build --locked -p ramag-bin` 后，通过 Computer Use 检查当前源码完整程序中的 WSL 连接与多终端标签，不使用安装版或 `ui-preview`。本切片不改变 SSH、SFTP 协议或凭据，Docker 不适用；回滚仅恢复本切片涉及的 SSH 标签呈现、状态展示辅助函数和对应测试，不回滚已有设置与公共 UI 改动。

2026-10-01 用户明确将功能完善与已知 UI/功能问题置于主线。当前顺序按差距矩阵执行，加载反馈、等待期间可操作和失败恢复先验收；动效、特效及发布性能优化后置，不阻挡独立功能修复。本轮不检查 GitHub CI。已有历史测试记录继续按其范围引用。

| 切片 | 组件职责与实施范围 | 验收证据 |
|---|---|---|
| `A-PULSE-SHELL-001` | 平台壳层复用公共紧凑页头：品牌与当前页面分级、主题语义边界、固定图标命中尺寸；工具缺失使用明确不可用状态 | 明暗主题三个尺寸、长工具名、首页/工具/设置切换、主题切换、不可用视图；Computer Use 或如实记录的替代证据 |
| `A-PULSE-HOME-001` | 首页采用公共标题和统一轻量工具项边界、密度与响应式间距 | 三个尺寸、入口点击、滚动和现有拖拽排序回归 |
| `A-PULSE-SETTINGS-001` | 设置采用公共标题与分区层级、统一导航状态和响应式间距 | 导航切换、长文字、表单命中、保存回归 |
| `A-PULSE-DB-001` | 数据库保留对象树和编辑工作区，统一标题、工具栏、状态与表格边界 | 布局和领域导航 headless、本机 Docker 在业务行为变化时执行 |
| `A-PULSE-SSH-001` | SSH 保留连接/终端/SFTP 流程，统一标题和连接状态表达 | 连接名称与状态可见、页面导航、终端区域和紧凑窗口 |
| `A-PULSE-VCS-001` | VCS 保留仓库、文件、历史与 Diff，统一标题和状态边界 | 页面导航、长仓库/文件名、滚动与状态回归 |
| `A-PULSE-CONTAINER-001` | 容器保留资源、日志和性能流程，统一标题、趋势与状态 | 页面导航、长资源名、空/失败状态和性能布局 |
| `A-PULSE-OBJECT-001` | 对象存储保留连接、对象列表和传输流程，统一标题与表格状态 | 页面导航、长对象名、列表滚动和传输状态 |

`A-PULSE-SHELL-001` 设计确认沿用用户已确认的全局 UI 吸收范围，已在 `165b8e9c` 推送。页头保持既有 32px 高度，主题及工具设置按钮保持 28px 固定命中区域，长标题省略且可查看全名。平台壳层不在运行界面暴露 crate 名或注册 API。每个切片通过最终 fmt、workspace Clippy、风险匹配测试与 UI 验收后独立提交并推送。

### 2026-10-01 Shell 切片验证

`A-PULSE-SHELL-001` 已完成代码和 headless 验收。`ramag-ui` 测试从 114 项增加到 115 项；新增 `pulse_shell_keeps_titles_actions_and_navigation_reachable`，覆盖明暗主题、`360x640`/`1024x768`/`1440x900`、长工具名、首页/工具/设置导航、主题切换、固定 28px 操作按钮、内容区边界和缺失工具不可用状态。`cargo test --locked -p ramag-ui --lib -- --test-threads=1` 全部通过，fmt、workspace Clippy、源码尺寸和差异检查通过。

Windows 实际窗口截图 `target/ui-fallback/pulse-shell-summary-dark-1024x768-20261001.png` 显示 `Ramag > 系统监控` 页头、紧凑边界、十标签和实时监控页面。Computer Use 重新发现窗口、激活并刷新截图成功，但一次观察后的点击仍返回 `foreground window did not report a process id`；完整原生点击流程继续标记未验收，截图只作为视觉证据。

### 首页切片设计

`A-PULSE-HOME-001` 沿用已确认的全局视觉范围：标题复用公共页面标题，显示可用工具数；入口使用 96px 固定高度、6px 圆角和轻量主题边界，图标与名称/描述形成紧凑层级。页面只显示工具状态和业务名称，移除操作教程文字。卡片尺寸同时用于拖拽落点与重排动画，保持当前工具注册、点击事件、顺序存储和侧栏同步。空注册表显示不可用状态，窄窗口继续收缩单列宽度。验收覆盖明暗主题三个尺寸、长名称/描述、空状态、滚动、点击和排序后的布局；不新增协议服务，Docker 不适用。

### 2026-10-01 首页切片验证

`A-PULSE-HOME-001` 完成代码和 UI 验收。`cargo test --locked -p ramag-ui --lib -- --test-threads=1` 的 117 项测试通过；首页新增 headless 入口命中与拖拽释放测试、明暗主题空注册表测试，并补充长标题/描述边界检查。现有网格、滚动和动画偏移测试继续通过。最终 fmt、workspace all-target Clippy、源码尺寸和差异/LF 检查通过。完整 workspace 默认测试和文档测试通过；忽略或环境变量控制的外部服务测试不计为集成证据。Docker 服务、镜像、端口、启动和清理均不适用。

Computer Use 本次恢复成功：在隔离预览的 `1024x768` 暗色窗口点击 API 测试入口并返回首页，将系统监控从第二行拖至第一位，首页与侧栏顺序同步；在 `360x640` 亮色窗口将 API 测试拖至末尾，卡片和侧栏再次同步；观察 `1440x900` 暗色窗口的三列布局。此次记录只覆盖首页导航和排序，不扩大为系统监控或其他工具的全部原生操作完成。系统截图保存于 `target/ui-fallback/pulse-home-dark-1024x768-20261001.png`、`pulse-home-light-360x640-20261001.png` 和 `pulse-home-dark-1440x900-20261001.png`，含系统边框尺寸分别为 1040x807、376x679、1456x939。测试进程已停止，未读取用户配置或连接。

### 设置切片设计

`A-PULSE-SETTINGS-001` 按已确认范围继续实施：页面标题复用公共 Pulse 标题，设置分区改为带顶部轻量分隔线的自然布局，去除整段设置的装饰外框；统一导航项尺寸、选中标记和悬停状态，补齐键盘焦点，保留桌面侧栏和窄窗口横向滚动导航。互斥选项使用分段控件，页面切换采用 140ms 短过渡并遵守减少动画偏好。控件继续显示真实配置，修改与异步保存仍使用现有服务和存储键；连接导入导出、转换程序和剪贴历史清理流程不变。系统监控外观、采样和预设并入系统设置页；其他工具设置保留各自页面。验收覆盖明暗主题三个尺寸、最大字号、所有可用设置页面的导航/滚动/长文字、监控刷新与系统偏好保存、SSH 兼容开关保存；真实窗口使用隔离预览，不读取用户配置或连接，不操作系统安全设置。不新增数据库或协议服务，Docker 不适用。

设置页过渡作为后置优化，在 `A-PULSE-MOTION-001` 独立验证后再验收。页面布局、导航、加载、保存回读和失败重试按独立功能验收；具体条件以开发计划 `2.5` 为准，未覆盖的整体视觉、过渡和发布性能继续列为未完成。

### 公共减少动画切片设计

`A-PULSE-MOTION-001` 先接入系统减少动画偏好，再验收各页面过渡。主程序和隔离预览采用同一初始化入口；读取在后台执行，每 2 秒至多一次且不重叠，仅当有效值发生变化才同步 GPUI `App::set_reduce_motion`。读取失败或平台不支持时禁用可选过渡，保留可诊断原因。无窗口时暂停读取，应用退出时释放任务，不产生无界线程、积压请求或静止页面重复重绘。

Windows 使用系统客户端区动画开关，macOS 使用系统辅助功能的减少动画设置，Linux 使用可用的桌面设置来源；各来源注明适用环境，无法可靠读取时采用上述失败行为。命令来源必须限制运行时间、输出量并回收子进程；不读取业务配置、凭据或连接，不修改操作系统偏好。

独立通过条件包括无窗口暂停读取、重复值不更新、读取失败禁用过渡、退出释放任务，以及 Linux 命令 300ms 超时和 128 字节上限；本机读取与其他平台编译分别记录。详细验收见开发计划 `2.5`，不能仅凭解析函数测试判定系统偏好同步完成。

验收先核对本机读取结果和系统来源，再以可控 GPUI 上下文覆盖允许/减少/读取失败和重复值：减少模式立即显示最终状态，重复值不请求帧，任务没有重叠且释放后停止。真实窗口只验证应用行为，保持本机系统设置不变；CI 分别检查 Windows、Linux、macOS 条件编译。此功能无数据库或协议服务，Docker 不适用；它不证明所有页面已满足 `PULSE-M01` 或 `PULSE-P01`。

### 原生性能采样设计

`A-PULSE-PROFILING-001` 在隔离预览增加可选的 `ui-profiling` feature，仅在显式选择时启用 GPUI 现有 profiler。预热 5 秒后采集有界的原始输入、draw 与平台 present 事件，记录单调时间、窗口 ID、阶段耗时、事件丢失和样本数，周期性写入进程日志；不为了采集主动请求绘制。发布构建按固定工作流连续操作至少 30 秒，随后静止 10 秒，再关闭窗口，记录 p95 和证据范围。

采样工具的正确性使用已知事件和有界容量验证；平台 present 提交、显示合成器呈现与实际可见反馈各自注明边界。原始日志缺少所需事件、样本不足、事件丢失或显示追踪不可用时，性能条件保留未完成，禁止把工具编译通过或调试程序计时当作发布性能达标。该切片不读取配置或连接，不改生产默认 profiler feature；Docker 不适用。

采样工具独立检查窗口与事件匹配、乱序/重复事件、10000 条容量、120 秒停止、丢失计数、缺失指标 `null`、JSONL 写入失败和退出恢复原 profiler 状态。GPUI 输入处理、draw 和平台提交日志只支持对应阶段的诊断；点击到可见反馈仍需平台显示追踪或同步录像。发布性能使用统一标准 `2.0.4` 的固定操作流程，不足 300 个活动帧时保留未完成，不请求额外装饰帧补足数量。

### 2026-10-03 公共 Pulse 样式推广阶段验证

当前主线已按工具边界形成独立提交：

- `235c60d3`：容器资源改为统一的 Pulse 表格列表，保留容器、镜像、网络、数据卷、日志和性能流程。
- `f0088d61`：VCS 仓库列表、Session 标签、错误状态和窄窗口边界复用 Pulse 标题、工具栏和状态面板。
- `19df8c90`：对象存储账号页复用 Pulse 页面标题、响应式工具栏和轻量账号列表面板。
- `3908bcf9`：DBClient 连接列表、对象树、查询工作区和结果边界复用 Pulse 视觉规则。
- `fee19b3f`：SSH 管理和工作区统一连接状态、终端退出状态和 Pulse 状态标签，保留 SFTP、终端和 JumpServer 行为。
- `456513a0`：系统监控 Settings 收口到 Ramag 全局 Settings，移除监控 Settings 页签和工具右上角设置入口，旧工作区状态迁移到 Summary；同时修复系统工具测试代码的 workspace Clippy 阻塞。

本轮目标测试均通过：容器 30 项、VCS 188 项、对象存储 27 项、DBClient 368 项、SSH 87 项、系统模型 9 项、系统监控 103 项、Ramag UI 132 项；对应目标 Clippy 均以 `-D warnings` 通过。`cargo test --locked --workspace -- --test-threads=1`、`cargo fmt --all -- --check`、workspace all-target Clippy 和 `cargo build --locked -p ramag-bin` 均已通过。构建使用 `F:\project\ramag-platform\target\debug\ramag.exe`，曾关闭旧实例后重新编译并启动，确认窗口标题为 `Ramag — 容器管理`。

本轮 Computer Use 的 Sky 服务返回 `Trusted RPC service is not configured: sky`，未能绑定真实窗口，因此当前新增工具切片只计入 headless 渲染/交互测试、源码构建和启动窗口检查；不把启动检查扩展为鼠标键盘原生验收，也不使用 `ui-preview` 作为验收程序。待 Computer Use 服务恢复后，按本计划从首页逐工具复核标题、表格、滚动、空/加载/失败状态和连接状态。

### 2026-10-03 API、Kafka、MQTT 与剪贴板切片验证

公共样式推广继续覆盖后续工具：

- `4eba6ca3`：API 工作区页头和本地工作区加载/失败状态复用 Pulse 页面标题与状态通知，保留 HTTP/gRPC、导入、保存、发送、取消和重试流程；目标测试 35 项。
- `7ac9083c`：Kafka 主工作区复用 Pulse 页面标题、连接状态徽章和状态通知，保留 Broker、Topic、消息、Consumer Group、Schema、Connect、ACL 和只读/管理权限流程；目标测试 38 项。
- `afd1134c`：MQTT 配置页复用 Pulse 页面标题和连接状态徽章，保留 MQTT 版本/传输配置、测试连接、保存、发布、订阅和本地服务流程；目标测试 32 项。
- `7a86487a`：剪贴板主视图增加 Pulse 页面标题，保留历史列表、详情、搜索、类型筛选、复制和清理流程；目标测试 21 项。
- `86913828`：JSON Path 提取器复用 Pulse 页面标题、执行状态徽标和响应式页头，保留 JSON5 输入、有界执行、结果和失败反馈流程；目标测试 5 项。
- `5567879d`：本机协作页复用 Pulse 页面标题、Relay/本机状态徽标和状态通知，保留加密草稿、人工导出、导入、复制、远端交接和撤销流程；目标测试 3 项。

六个切片的目标 Clippy、格式检查和提交钩子均通过；未改变协议、凭据或外部服务契约。Computer Use 服务仍未配置，本轮证据限于当前源码的 headless 视觉/交互测试、构建和提交钩子，未把静态截图或启动检查扩展为原生鼠标键盘验收。

### 2026-10-03 当前构建验收

使用当前源码重新执行 `cargo build --locked -p ramag-bin`，产物为 `F:\project\ramag-platform\target\debug\ramag.exe`；先关闭旧实例后启动该产物，窗口标题与进程路径均对应当前工作区。Computer Use 观察到系统监控 Summary 的 CPU、内存、磁盘、网络、能耗和温度均来自实时样本，图表标签与顶部读数一致，没有旧截图中的错误 `0` 首帧。容器资源表的真实数据链路通过 `ramag-infra-container-docker` 本机 Docker Engine 只读连接测试；容器、镜像、网络和数据卷表格的窄窗口、横向滚动、列标题和行边界由容器工具 30 项 headless 测试覆盖。当前窗口随后被用户切换到数据库客户端，未继续复用旧窗口坐标。

本次完整工作区测试 `cargo test --locked --workspace -- --test-threads=1`、`cargo fmt --all -- --check` 均通过；本机 Docker Engine ignored 测试 `reads_local_engine_without_write_operations` 通过。Computer Use 真实窗口证据与 headless 表格证据分别记录，不把其中一类替代另一类。

### 2026-10-03 当前进行中切片验收

容器资源页使用当前源码构建的 `F:\project\ramag-platform\target\debug\ramag.exe`（PID `27012`），先关闭已安装目录中的旧 Ramag，再启动并核对进程 `ExecutablePath`。Computer Use 在同一窗口查看 System-tool Summary、全局 Settings 的插件页和容器资源页；System-tool 显示 CPU `7.8%`、内存 `23.7 / 47.9 GiB` 的实时读数。连接本机 Docker Engine `npipe:////./pipe/docker_engine` 后读取到 Engine `29.7.2` 和 9 个容器，资源表的边框视口从工具栏下方延伸到窗口内容区底部，列标题、字体、字号、背景及交替行状态可见；未通过 UI 执行容器写操作。

插件页真实窗口显示运行概览（12 个已就绪、0 个待处理）及第一方目录列表，页面标题、指标卡、面板和状态点均可见并可纵向滚动。标准插件入口视图由 `ramag-ui` headless 测试覆盖，本机当前首页没有独立示例入口可供真实窗口打开，因此不扩展为该入口的原生验收。当前原生观察仅覆盖 System-tool Summary、插件设置页和容器资源表，不代表其余工具已逐一完成。

VCS 布局切片由 `f0c36752` 完成：文件栏模式按钮不再因弹性收缩而纵向堆叠，分支选择器与文件搜索/刷新操作保留清楚层级；没有选中文件时，右侧主区用居中的 Pulse 面板说明下一步操作。Computer Use 使用 `F:\project\ramag-platform\target\debug\ramag.exe`（PID `12776`，已核对 `ExecutablePath`）打开 VCS 仓库工作区；文件模式按钮横向排列，分支选择器和搜索各自对齐，README.md 预览打开后这些控件保持位置，未执行写入或 Git 操作。真实窗口复核也发现空状态说明末尾标点曾独占一行，文案已收短并在最终构建中确认单行显示。

验证：`cargo test --locked -p ramag-tool-container -p ramag-ui --lib -- --test-threads=1`（容器 31 项、`ramag-ui` 132 项）及此前 `cargo test --locked --workspace -- --test-threads=1` 通过；VCS `cargo test --locked -p ramag-tool-vcs --lib -- --test-threads=1` 188 项通过、0 失败、5 项性能观察用例 ignored；workspace all-target Clippy `-D warnings`、`cargo build --locked -p ramag-bin`、fmt、Windows 源码尺寸脚本和 `git diff --check` 通过。workspace 默认测试将 Docker 专项用例标记为 ignored；本轮容器列表真实数据来自完整程序只读连接，不把 ignored 测试计为集成测试通过。三个 UI 切片已分别提交并推送 `main`；接着按表格/列表、编辑/连接工作区顺序继续真实窗口验收。

### 2026-10-03 VCS 分支栏布局优化验收

VCS 文件栏在常规宽度下移除空闲时占满弹性空间的占位项，模式按钮与分支选择器保持同一行；分支栏按侧栏剩余宽度伸展，宽度限制在 140–320px，并将可见分支文本上限由 18 个字符提高到 28 个字符。最窄侧栏下分支栏完整换到模式行下方，搜索和固定操作仍留在下一层。未改变 Git 状态读取或任何仓库操作。

目标测试 `vcs_files_toolbar_wraps_controls_inside_supported_widths` 覆盖 180、280、600px 文件栏：断言控件不越界，280/600px 下分支选择器与模式按钮同排，180px 下换行且保持至少 140px 宽。`cargo test --locked -p ramag-tool-vcs --lib -- --test-threads=1` 通过（188 项通过、0 失败、5 项性能观察用例 ignored）；workspace all-target Clippy `-D warnings`、`cargo fmt --all -- --check`、`cargo build --locked -p ramag-bin`、Windows 源码尺寸脚本与 `git diff --check` 均通过。

本机时间 `2026-10-03 16:55 +08:00` 由 `Get-Date` 核对。Computer Use 使用工作区完整构建 `F:\project\ramag-platform\target\debug\ramag.exe`（PID `33796`，`ExecutablePath` 已核对），先关闭已安装版后再启动；默认暗色窗口截图为 1528×924。打开 `F:\project\ramag` 仓库后，模式图标、分支选择器和搜索工具栏层级清楚；分支栏在模式按钮同排填充可用宽度。打开 `README.md` 后布局保持稳定。此原生复核仅覆盖该默认窗口和侧栏宽度；窄/宽侧栏由 headless 测试覆盖，未执行 Git 写操作。

### A-UI-VCS-002：工作区导航层级优化（代码与完整程序验收完成，2026-10-03）

- 问题证据：用户截图指出 VCS 文件栏顶部布局失衡。修改前的完整构建只有三个图标模式入口，且与当前分支选择器共用一行；模式含义不直观，当前分支文字也受到横向空间限制。
- 实现：模式切换改为等宽“项目 / 变更 / 储藏”文字标签，分支选择器独占下一行并填充文件栏宽度；搜索和刷新、展开、历史及远程操作保留在第三行。分支菜单、模式状态与 Git 操作回调不变。
- Headless：`vcs_files_toolbar_uses_labeled_modes_and_full_width_branch_row` 在 180、280、600px 侧栏宽度检查模式标签同排等宽、分支行位于模式下方且占满可用宽度、搜索和固定操作均留在各自工具栏内。`cargo test --locked -p ramag-tool-vcs --all-targets -- --test-threads=1` 通过 188 项，5 项性能观察用例按设计忽略。
- 质量：`cargo fmt --all -- --check`、workspace all-target Clippy `-D warnings`、`cargo build --locked -p ramag-bin`、Windows 源码尺寸检查、UTF-8/LF 和 `git diff --check` 均通过。
- 原生窗口：本机 `2026-10-03 19:33 +08:00`，Computer Use 打开完整工作区构建 `F:\project\ramag-platform\target\debug\ramag.exe`（PID `12564`，`ExecutablePath` 已核对），在 1555×924 暗色窗口检查 `F:\project\ramag`。三个文字模式标签、活动态、独立全宽分支行、搜索及空 Diff 面板均可见；切换“变更”显示工作区干净状态，返回“项目”恢复文件树。打开分支菜单后以 Escape 关闭，没有执行 Git 写操作。长分支名仍使用省略显示，完整值保留在悬浮提示中。
- 回滚边界：仅调整 `ide_layout.rs`、对应工具栏布局测试与本记录；未改 Git 状态读取、分支菜单、文件选择、仓库会话或操作参数。

### A-UI-VCS-003：压缩并统一文件栏导航布局（代码、构建和完整程序验收完成，2026-10-03）

- 问题证据：最新用户截图中，文件模式、当前分支和搜索操作分散在多层工具栏；分支控件与模式区域的对齐不一致，导航占用过多垂直空间。
- 实施范围：在常规侧栏宽度下将等宽“项目 / 变更 / 储藏”模式组与分支选择器编排到同一导航行，模式组保持左侧、分支选择器使用有界弹性宽度并靠右；窄侧栏自动将分支选择器换到下一行。搜索、刷新、展开、历史和远程操作继续独立成行；统一内边距和分隔线。保持现有控件 ID、分支菜单、模式状态和 Git 回调。
- 验收条件：Headless GPUI 覆盖 180、280、600px 侧栏，确认 280/600px 导航同排、180px 自动换行，分支文本和控件不越界，搜索行始终位于导航组下方；原生验收使用当前源码构建的完整 Ramag 窗口核对默认尺寸和模式切换。运行 VCS 全量测试、fmt、目标 Clippy、完整程序构建、源码尺寸和差异检查。
- 回滚边界：只回滚 `ide_layout.rs`、对应 VCS 工具栏布局测试及本记录，不回滚已完成的 VCS 公共样式、空状态或 Git 操作实现。
- 代码验证：`vcs_files_toolbar_compact_navigation_wraps_at_supported_widths` 与 VCS 全量测试通过（188 项通过、0 失败、5 项性能观察用例 ignored）；VCS all-target Clippy `-D warnings`、workspace fmt、Windows 源码尺寸、UTF-8/LF 和 `git diff --check` 通过。标准 `target/debug/ramag.exe` 因现有窗口占用无法覆盖；PowerShell 使用独立输出目录执行 `$env:CARGO_TARGET_DIR='target/vcs-layout-validation'; cargo build --locked -p ramag-bin` 成功，生成 `target/vcs-layout-validation/debug/ramag.exe`。
- 原生验收：2026-10-03 22:04（`+08:00`）关闭旧实例后启动 `F:\project\ramag-platform\target\vcs-layout-validation\debug\ramag.exe`，进程路径与窗口标题 `Ramag — 版本管理` 已核对。1555×924 暗色窗口中打开 `F:\project\ramag` 仓库，确认仓库列表进入详情、模式标签“项目 / 变更 / 储藏”与分支选择器同排、搜索行独立位于下方；分支菜单可打开并显示本地/远程分支，切换“变更”显示工作区干净状态，返回“项目”后打开 README.md 预览，导航位置保持稳定。未执行 Git 写操作、提交、推送或凭据输入。

### 2026-10-03 GitHub CI 日志字段修复

GitHub Actions 的 `make size-check log-check` 在日志约定检查中失败，定位到内嵌 Michroma 字体加载失败时的 `tracing::warn!` 没有以 `operation` 字段开头。日志现已使用 `operation = "monitor_heading_font_register"`，并继续保留结构化 `error` 字段。

Windows 本机重跑 `bash -lc 'make size-check log-check'` 通过；`cargo test --locked -p ramag-ui --lib -- --test-threads=1` 的 132 项测试、该 crate 的 all-target Clippy、workspace fmt 和完整 `ramag-bin` 构建均通过。完整程序写入 `target/vcs-layout-validation/debug/ramag.exe`，避免覆盖被现有应用窗口占用的默认可执行文件；这项构建结果不作为新的原生 UI 验收。

### 2026-10-03 对象存储账号空状态优化验收

对象存储无账号状态从孤立的“新建”按钮改为 Pulse 轻量面板：增加存储图标、“暂无对象存储账号”标题、COS/OSS 后续操作说明和“新建账号”主操作，并在窄内容区保留 16px 外边距。新建入口仍打开原账号表单；未修改账号、凭据、Bucket、连接和存储行为。

`empty_account_state_stays_inside_supported_window_sizes_and_themes` 覆盖明暗主题与 `360x640`、`1024x768`、`1440x900`，检查面板、标题、说明和操作都留在内容区内且顺序稳定。`cargo test --locked -p ramag-tool-object-storage --lib -- --test-threads=1` 通过 28 项；`cargo fmt --all -- --check`、workspace all-target Clippy `-D warnings`、`cargo build --locked -p ramag-bin`、Windows 源码尺寸脚本和 `git diff --check` 均通过。此本地 GPUI 空状态切片不使用 Docker。

本机时间 `2026-10-03 17:20 +08:00` 由 `Get-Date` 核对。Computer Use 使用完整工作区构建 `F:\project\ramag-platform\target\debug\ramag.exe`（PID `34508`，`ExecutablePath` 已核对），在暗色 1555×924 窗口检查账号空状态；点击“新建账号”打开现有表单后取消，回到同一空状态，没有输入或保存凭据。此记录仅验收无账号状态；已配置账号的原生列表、Bucket/对象浏览和传输流程仍待逐项对比，不以 headless 测试替代。

### 2026-10-03 DBClient 结果表原生验收

Computer Use 使用同一完整构建（PID `34508`）检查 1555×924 暗色窗口中的 DBClient `ramag_ui_test.bulk_records` 结果页。结果表从查询工具栏下方铺满工作区，表头、分页和双向滚动条保持可见；本机只读翻页由 `1–100` 到 `101–200`，横向滚动可查看 `payload` 与 `binary_token` 列，纵向滚动仍在当前 100 行页内移动。返回第一页后恢复了表格的首屏和左侧列；未执行事务、写入 SQL 或数据修改。与同窗 System-tool Summary 对照，深色背景、边界和紧凑排版保持一致，结果表保留数据密度所需的较小行文。

本机 Docker 服务为 `ramag-visual-test-mysql84`（`mysql:8.4`，`127.0.0.1:13318 -> 3306/tcp`），检查时已运行约 10 小时；本轮未启动或停止该服务，也未执行独立 Docker 集成测试。`cargo test --locked -p ramag-tool-dbclient --lib -- --test-threads=1` 通过 368 项；`cargo fmt --all -- --check` 和 DBClient all-target Clippy `-D warnings` 通过。此记录只覆盖真实连接下的结果表、分页和滚动；连接管理、查询编辑器、其它驱动及写入/事务流程仍待独立验收。

### 2026-10-03 DBClient 连接列表视口高度修复计划

本机 `17:58 +08:00` 使用当前工作区完整构建的 Ramag（进程路径 `F:\project\ramag-platform\target\debug\ramag.exe`，PID `34508`）在 1555×924 暗色窗口打开“数据源管理”。标题、副标题、连接搜索和新建图标可见，但主体在等待异步加载后仍为空白；窗口保留一个已连接 MySQL 会话标签，因此原生观察不能单独判定持久化连接列表的数量。进一步检查发现非空列表分支把 `uniform_list(size_full)` 放在没有高度约束的横向容器中；新增的 `360x640` 单连接 headless 断言复现列表面板只有 `312x2`，确认视口高度塌缩是主要布局缺陷。空列表新建操作在 headless 三种窗口尺寸中已验证可见。

本切片只修复非空连接列表的高度塌缩：视口填充标题区下方工作区；已加载空列表的新建操作保持可见。不得改变连接存储、查询/预取、会话标签或数据库内容。Headless 测试覆盖单连接列表填充视口及空列表操作在 `360x640`、`1024x768`、`1440x900` 内可见；真实窗口复查已保存连接行和新建表单打开/取消，不保存连接、不运行数据库写操作。加载/失败状态、搜索无匹配和多行滚动仍按后续验收逐项检查；本切片不将它们标记为已验收。

### 2026-10-03 DBClient 连接列表视口修复验收

`ConnectionListPanel` 的非空主体改为填充标题区下方剩余高度，并约束 Pulse 面板和列表的最小高度；保留 `uniform_list` 与已有连接选择回调。回归测试此前在 `360x640` 复现 `312x2` 面板高度，修复后在 `360x640`、`1024x768`、`1440x900` 三种尺寸均验证面板高度至少 400px 且页面标题、工具栏、列表、搜索和新建入口不越界。空列表的新建操作在相同尺寸内保持可见。

Computer Use 使用完整构建 `F:\project\ramag-platform\target\debug\ramag.exe`（PID `19712`，进程路径已由 Windows 进程表核对），在 1555×924 暗色窗口打开“数据源管理”。已保存的 `127.0.0.1:13318` MySQL 连接行和版本 `8.4.9` 可见，列表面板从页头下方延伸至客户区底部；新建连接表单可打开并取消，回到原连接行，未保存表单或更改连接配置。查询编辑器中预存的 `SELECT * FROM ramag_ui_test.bulk_records` 只读查询显示结果；未启动事务、执行写入或修改测试数据。此轮原生检查没有多行连接可供滚动，加载/失败及搜索无匹配状态也未覆盖。

全量目标测试 `cargo test --locked -p ramag-tool-dbclient --lib -- --test-threads=1` 通过 369 项；目标 Clippy、workspace all-target Clippy `-D warnings`、`cargo fmt --all -- --check`、完整构建 `cargo build --locked -p ramag-bin`、Windows 源码尺寸脚本、UTF-8/LF 与 `git diff --check` 均通过。

本机 `2026-10-03 18:18 +08:00` 由 `Get-Date` 核对。MySQL 服务在检查开始前已运行约 10 小时，容器创建于 2026-09-26：`ramag-visual-test-mysql84`，镜像 `mysql:8.4`（窗口报告版本 `8.4.9`），端口 `127.0.0.1:13318 -> 3306/tcp`，专用具名卷 `ramag-visual-test-mysql84-data`。本轮未启动服务；原生列表读取及上述 SELECT 为只读。验收窗口关闭后已删除该测试容器和专用数据卷，并核实容器、卷均不存在；未运行单独的 Docker 集成测试。DBClient 其他驱动、写入/事务流程和原生多行滚动仍待验收。

### 2026-10-03 对象存储账号列表高度核验计划

本机 `2026-10-03 18:35 +08:00` 使用当前工作区构建查看对象存储工具：账号列表为空，真实窗口只显示已验收过的空状态；当前没有可用于原生账号列表、Bucket、对象和传输验收的已保存账号。不得创建或保存临时云凭据，也不把 Bucket 或对象操作替换成未经授权的远端资源操作。

继续核验已配置账号时的布局：工具标题和搜索栏下方的账号列表面板应填满剩余工作区，长列表在面板内滚动；账号选择、编辑、删除及既有确认回调保持不变。先用现有 headless 合成账号测试在三种窗口尺寸下测量面板高度，必要时只修列表面板高度并保留搜索、提供商、只读和 Bucket 数量呈现。真实窗口只复核当前空状态；没有真实账号时，配置账号、Bucket/对象浏览和传输保留为未验收。此切片不启动 Docker 或联系对象存储服务。

### 2026-10-03 对象存储账号列表高度修复验收

原生空状态仍在当前工作区完整构建中显示在页头下方。已配置账号分支的 headless 布局断言先复现 360×640 窗口中的 `328×147` 面板，确认短账号列表没有填充剩余工作区。修复后，账号列表面板占据剩余客户区，滚动条移至面板内部的账号行视口；搜索、提供商、只读标记、Bucket 数量及行操作未改变。

`account_manager_uses_shared_page_hierarchy_at_supported_widths` 在明暗主题及 `360x640`、`1024x768`、`1440x900` 下使用 32 个内存合成账号验证面板与内层视口高度、长列表末行越过视口以及各控件边界；这些账号只存在于测试进程，没有写入用户存储或远程服务。`cargo test --locked -p ramag-tool-object-storage --lib -- --test-threads=1` 通过 28 项；workspace all-target Clippy `-D warnings`、`cargo fmt --all -- --check`、Windows 源码尺寸脚本、UTF-8/LF 和差异检查通过。完整程序 `cargo build --locked -p ramag-bin` 通过。

本机 `2026-10-03 18:51 +08:00` 由 `Get-Date` 核对。Computer Use 使用完整构建 `F:\project\ramag-platform\target\debug\ramag.exe`（PID `31940`，Windows 进程路径已核对），在 1555×924 暗色窗口确认对象存储页头、搜索、新建入口及空状态面板保持可见。没有已保存云账号，因此真实配置账号、Bucket/对象浏览和传输仍未验收；本轮未连接对象存储服务、创建账号或保存凭据，Docker 不适用。

### 2026-10-03 剪贴板筛选详情状态修复验收

完整构建 `F:\project\ramag-platform\target\vcs-layout-validation\debug\ramag.exe`（PID `13864`，窗口 `Ramag — 剪贴板`）中复现了剪贴板筛选状态问题：选中历史条目后输入无匹配搜索词，左侧列表显示无匹配，但右侧仍残留旧详情。修复后，详情选择只保留当前搜索/类型筛选结果中的条目；筛选结果为空时同步清除选中 ID 和详情缓存，右侧回到“选择左侧条目查看详情”。

Computer Use 真实窗口验证了页面标题、搜索框、类型筛选、条目详情、无匹配状态和详情清空流程；使用已有本地测试条目，未写入外部服务、未删除历史、未输入凭据。`cargo test --locked -p ramag-tool-clipboard --lib -- --test-threads=1` 通过 22 项，新增选中项可见性回归；目标 all-target Clippy、workspace fmt、完整 `ramag-bin` 构建和 `git diff --check` 通过。

### 2026-10-03 插件目录真实窗口验收

完整构建 `F:\project\ramag-platform\target\vcs-layout-validation\debug\ramag.exe`（PID `28716`，窗口 `Ramag — 设置`）打开全局 Settings 的“插件”页面。运行概览显示插件总数 `12`、已就绪 `12`、待处理 `0`；第一方工具目录卡片使用统一 Pulse 标题层级、轻量面板、状态点、字体和间距，数据库、API、Kafka、MQTT、VCS、SSH、对象存储、容器、系统监控、本机协作和 JSON Path 等条目可见。

Computer Use 向下滚动目录，确认长列表在 Settings 内容区内部滚动，左侧设置导航保持固定，末端条目没有越出窗口；本轮只检查目录和状态展示，没有执行插件任务、连接、凭据或外部服务操作。插件标准入口的领域流程仍按既有 headless 测试边界记录，不把目录页面验收扩大为所有插件业务完成。

### 2026-10-03 API 工作区完整程序验收

完整构建 `F:\project\ramag-platform\target\vcs-layout-validation\debug\ramag.exe`（PID `21760`，窗口 `Ramag — API 测试`）从首页打开 API 工作区。真实窗口显示 Pulse 页面标题“API 测试”、HTTP/gRPC 请求工作区、请求编辑区、响应面板、断言/变量和历史记录；切换 HTTP 与 gRPC 后对应字段、标签和操作区保持在工作区内。

本机 Docker 服务保持运行并仅执行只读测试请求：`ramag-api-http-test`（`python:3.12.11-alpine-3.22`，`127.0.0.1:18089 -> 8080`）返回 HTTP `200` JSON；`ramag-api-grpc-test`（`rust:1.91.0-bookworm`，`127.0.0.1:18090 -> 50051`）返回 gRPC `docker echo: hello`，响应面板和历史记录均更新。未保存请求、未输入凭据、未修改服务或测试数据；API 断言、取消、导入/保存和 Collection 业务仍沿用既有目标测试与 Docker 证据边界。

### 2026-10-03 Kafka 概览关键数据摘要切片

Kafka 概览新增 System-tool 风格的“关键数据”轻量面板，集中显示 Topics、消费者组、Schema Registry、Kafka Connect、ACL 和 ksqlDB 的当前状态。Topic 数量来自已读取的 Broker 快照；消费者组新增明确的“未读取/已读取/读取失败”状态边界，避免空列表同时表示“没有数据”和“尚未请求”。Schema Registry、Kafka Connect 和 ksqlDB 继续根据保存的端点显示“未配置”，ACL 和其它懒加载资源显示“未读取”，异步加载或失败状态保留警告/错误语义。

本切片只改变概览呈现和消费者组快照状态，不在打开概览时预加载外部资源，不改变 Kafka 协议、配置、权限、Topic/消息操作或页签懒加载。摘要卡片使用公共 Pulse 状态标签、面板边界和响应式换行；关键区域在 `360`、`900`、`1200` 和 `1440` 宽度的 headless GPUI 测试中保持在内容区内，并验证新增面板不会覆盖指标分区点击入口。

验证结果：`cargo test --locked -p ramag-tool-kafka --lib -- --test-threads=1` 通过 38 项；Kafka 目标 all-target Clippy、`cargo fmt --all -- --check`、Windows 源码尺寸检查、`git diff --check` 和完整 `cargo build --locked -p ramag-bin` 通过。未启动 Docker、未连接外部 Kafka、未执行 GitHub CI 监测；完整程序构建产物为当前工作区的 `target/debug/ramag.exe`。本切片没有 Computer Use 原生窗口证据，保留既有运行时服务限制。

### 2026-10-03 MQTT 配置页头紧凑对齐

MQTT 配置页头原先在连接状态徽章和测试/保存操作组之间放置了额外的弹性空容器，导致宽窗口出现无意义的横向空白、窄窗口出现过大的纵向间距。移除该占位后，Pulse 页面标题、连接状态和操作组按同一层级排列；MQTT 配置、测试连接、保存、发布、订阅和本地服务流程保持不变。

新增 `mqtt_header_keeps_status_and_actions_compact_at_supported_widths`，覆盖 `360`、`768`、`1024` 和 `1440` 宽度，断言窄窗口状态徽章与操作组相邻换行、宽窗口状态位于操作组左侧且所有内容留在窗口内。`cargo test --locked -p ramag-tool-mqtt --lib -- --test-threads=1` 通过 33 项；目标 Clippy、workspace fmt、Windows 源码尺寸检查、`git diff --check` 和当前源码完整程序 `cargo build --locked -p ramag-bin` 均通过。Computer Use 当前只返回浏览器且没有原生窗口应用，本切片不宣称真实窗口鼠标/键盘验收，也不恢复 GitHub CI 监测。

### 2026-10-03 剪贴板窄窗口内容区填充

剪贴板主视图在窄窗口原先把列表和详情固定为 `320px` 与 `360px`，窗口高度变化时会留下不协调的空白，长内容也只能依赖外层整体滚动。列表和详情现在在内容区内使用弹性高度并保留 `240px` 最小可读区域，各自的内部滚动、搜索、类型筛选、详情选择和复制/清理操作不变。

扩展 `clipboard_content_reflows_list_and_detail_inside_supported_widths`，在 `360`、`800`、`1024` 和 `1440` 宽度检查窄窗口两 pane 的最小高度、上下顺序及常规窗口的左右布局。`cargo test --locked -p ramag-tool-clipboard --lib -- --test-threads=1` 通过 22 项；目标 Clippy、workspace fmt、Windows 源码尺寸检查、`git diff --check` 和当前源码完整程序 `cargo build --locked -p ramag-bin` 均通过。Computer Use 当前没有可操作的原生窗口应用，本切片不宣称真实窗口鼠标/键盘验收，也不恢复 GitHub CI 监测。
