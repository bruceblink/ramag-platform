# System Pulse 采集替换与 Ramag UI 吸收计划

> 状态：系统监控替换和公共 UI 已验证；其他工具的视觉推广及跨平台原生验收继续按独立切片执行。
> 设计确认：用户于 2026-10-01 确认按本计划实现。
> 来源：https://github.com/eas4ai/system-pulse/tree/f1be5d51d24c21fa8c740be79200bdda3df3a00c

## 术语表与命名约定

| 规范中文名 | English / Acronym | 职责边界 | 不代表什么 |
|---|---|---|---|
| 系统采集器 | System Collector | 读取本机硬件、计数器和进程数据 | 不负责 GPUI 渲染 |
| 系统快照 | System Snapshot | 一次采样产生的独立设备、传感器和诊断记录 | 不代表持久化日志 |
| 传感器读数 | Sensor Reading | 带稳定 ID、物理单位、来源时间和状态的单项指标 | 不代表把缺失数据估算为零 |
| 稳定进程身份 | Stable Process Identity | PID 与平台精确启动时间组合，发送信号时重新核对 | 不代表进程名称或秒级启动时间 |
| 系统监控视图 | System Monitor View | Ramag 内的 GPUI 页面、交互和偏好 | 不代表另一个应用窗口 |

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

整体 UI 必须逐页满足 [`01-development-plan.md`](01-development-plan.md) 的 `PULSE-V01` 至 `PULSE-G01`；视觉基准、图例和操作边界见 [`07-ui-acceptance-standard.md`](07-ui-acceptance-standard.md) 的 `2.0.1` 与 `2.0.2`。每个切片先更新验收条件再开发。已有采集接入和 UI 提交的验证仍按当时范围保留；新整体视觉、动态和资源条件未复验前不能宣称全部完成。当前设置改动处于未提交实现阶段，需按新条件继续验证。

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

## 后续视觉推广队列

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

`A-PULSE-SETTINGS-001` 按已确认范围继续实施：页面标题复用公共 Pulse 标题，设置分区改为带顶部轻量分隔线的自然布局，去除整段设置的装饰外框；统一导航项尺寸、选中标记和悬停状态，补齐键盘焦点，保留桌面侧栏和窄窗口横向滚动导航。互斥选项使用分段控件，页面切换采用 140ms 短过渡并遵守减少动画偏好。控件继续显示真实配置，修改与异步保存仍使用现有服务和存储键；连接导入导出、转换程序和剪贴历史清理流程不变。共享分区用于系统、监控、数据库、SSH 和剪贴板设置，来源状态复用公共状态提示。验收覆盖明暗主题三个尺寸、最大字号、所有可用设置页面的导航/滚动/长文字、监控刷新与系统偏好保存、SSH 兼容开关保存；真实窗口使用隔离预览，不读取用户配置或连接，不操作系统安全设置。不新增数据库或协议服务，Docker 不适用。
