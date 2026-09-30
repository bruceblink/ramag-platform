# Ramag Platform 执行计划与验收记录入口

> 状态：现行执行规则
> 更新日期：2026-09-30
> 主线：阶段 A 单机桌面收口与阶段 B 原生工作区迁移；阶段 C 继续后置
> 路线图：[`02-development-roadmap.md`](02-development-roadmap.md)
> 统一 UI 标准：[`07-ui-acceptance-standard.md`](07-ui-acceptance-standard.md)
> 历史执行记录：[`archive/2026-09-25-pre-datagrip-rebaseline/01-development-plan.md`](archive/2026-09-25-pre-datagrip-rebaseline/01-development-plan.md)

## 术语表与命名约定

| 规范中文名 | English / Acronym | 当前文件中的职责边界 | 不代表什么 |
|---|---|---|---|
| 设计确认 | Design Confirmation | 在实现前确认切片范围、界面规则和验收条件 | 不代表代码已完成 |
| 验收记录 | Acceptance Record | 记录命令、环境、结果、证据边界和未完成项 | 不代表所有产品线都通过 |
| 本机 Docker 集成 | Local Docker Integration | 使用本机容器验证真实数据库或协议服务 | 不代表远程集群或生产验证 |
| 真实窗口 | Native Window | 通过 Computer Use 在 Windows 窗口中完成的操作 | 不代表 headless 渲染或进程启动 |
| 虚拟视图 | Virtual View | 由数据库工具提供的只读对象树节点，通过系统目录或会话查询生成结果 | 不代表数据库中存在同名物理表或可写 View |
| 会话快照 | Session Snapshot | 用户展开 `sessions` 时读取的当前连接会话结果集 | 不代表持久化审计记录或可编辑数据表 |
| 原生工具插件 | Native Tool Plugin | 使用 Rust 逻辑和 GPUI 视图接入桌面宿主的插件 | 不代表允许使用 WebView 或外部窗口 |
| 计算核心 | Compute Core | 不依赖 GPUI 的解析、转换、校验或计算模块 | 不代表可直接访问凭据、数据库或任意文件 |
| 第一方工具目录 | First-party Catalog | 由项目维护、审查并随版本发布的工具清单 | 不代表第三方市场或不受信任代码已经开放 |

## 1. 每个切片的固定顺序

1. 阅读现行主线和对应专项路线图，确认没有把归档事项重新列为待办。
2. 写明问题证据、用户流程、设计、改动范围、不做事项和验收条件；先完成设计确认，再开始代码。
3. 实现代码、测试、必要注释和文档；所有新建/修改文本文件使用 LF 换行。
4. 先运行目标测试和 UI 验收，再运行 Rust workspace 的 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`git diff --check` 和适用的源码尺寸检查。
5. 涉及外部服务时使用本机 Docker，记录服务名、镜像版本、端口、启动/健康状态和停止/清理状态。
6. 测试通过后使用一个英文 Conventional Commit，立即推送 `main`；未经验证、部分完成或存在未说明无关改动时不得提交。

## 2. UI 验收顺序

- 先在 headless GPUI 中验证 `360x640`、`1024x768`、`1440x900`；弹窗追加 `360x240`。
- Computer Use 可用时，按真实用户流程完成启动、连接/加载、点击、键盘、滚动、编辑、取消和截图；每张截图对应一个验收条件。
- Computer Use 不可用时，先记录启动、窗口发现或交互失败原因，再使用 headless 或系统截图作为替代，并明确没有覆盖的真实窗口行为。
- 不得把系统截图、静态图片或仅进程启动描述为真实窗口交互完成。

## 3. 验收记录模板

每个切片在对应专项文档追加以下字段：

- 切片 ID、日期、提交和推送结果；
- 设计与改动范围、不做事项；
- 目标测试、UI 尺寸和交互步骤；
- Docker 服务、镜像、端口、启动/健康/清理状态；
- Headless、真实窗口、数据库/协议结果及各自边界；
- 未完成项、阻塞项和下一项依赖。

## 4. 当前主线与后续切片

### A-QUALITY-UI-02：公共视觉规范与工具入口优化（2026-09-28，代码与 headless 验收完成）

- 依据：用户要求统一美化和优化 UI；沿用现行原生 GPUI、紧凑工作台和明暗主题规则。
- 设计：共享面板、间距、圆角与标题组件；补齐两种主题的状态色、按钮文字对比和焦点反馈。首页改为清晰的品牌标题、工具总数与操作提示，卡片统一图标容器、标题和描述层级；设置页与 SSH 空状态复用相同的面板规范。
- 交互：保持原有工具点击、拖拽排序和导航事件；长标题省略并可查看全文，窄窗口自动收缩与换行，滚动内容保留边缘空间；已选导航项在悬停时保留选中反馈。
- 截图问题一：数据库列筛选与 WHERE 采用同一输入外框，统一字体、行高、内边距和焦点边框；保留列补全与 WHERE 执行，额外验证有值/清空状态。
- 截图问题二：设置导航将内边距放入内容层，滚动层使用无内边距的视口；内容可完整显示时滚动范围为零，矮窗口仍能滚到最后一项。
- 截图问题三：容器工作区占用标题栏之外的剩余高度，左右两列顶部对齐；侧栏撑满工作区，并在内容超高时独立滚动。
- 配色根因：应用直接修改主题颜色后没有重建组件令牌，按钮仍显示上游默认颜色；同步组件令牌和 Base 主题副本，并验证明暗主题往返切换后的文字对比。
- 验收：明暗主题文字对比与状态色检查，首页卡片点击/拖拽、设置页布局、容器侧栏和 SSH 空状态的 headless 回归；覆盖 360x640、1024x768、1440x900。`ramag-ui` 108 项、容器 29 项、SSH 82 项及数据库筛选 2 项通过；workspace 全量测试、fmt、Clippy 和源码尺寸检查通过。
- 真实窗口边界：Computer Use 当前仍返回 `Trusted RPC service is not configured: sky`，没有可操作的 Ramag 窗口；本切片保留 headless 证据，未把系统截图或进程启动当作真实窗口通过。
- 交付：容器/SSH 布局提交 `35d3410f`，首页/设置导航提交 `3766717f`，主题令牌提交 `7734d686`，均已推送 `main`。

### A-QUALITY-UI：跨工具视觉与交互统一（2026-09-27，开发中）

- 依据：用户要求统一布局、对齐、颜色、功能图标和滚动条，并继续沿用原生桌面工作区设计。
- 设计：沿用紧凑 IDE 密度；统一主题的面板、输入、标签、选中、悬停和焦点颜色；共享工具栏最小高度与内边距；工具入口使用各自的内嵌功能图标；窄窗口允许工具栏换行，滚动区域显式显示滚动条并保持独立尺寸约束。
- 实施顺序：先收口已有图标和滚动修复，再统一壳层与各工具主要操作区域，最后运行跨尺寸、浅色/深色及交互回归，修复暴露的布局问题。
- 验收范围：首页、活动栏、设置和已注册工具的主要工作区；重点检查 API 编辑/响应分区、SSH 空状态、工具导航和长内容滚动。覆盖 360x640、1024x768、1440x900，紧凑弹窗追加 360x240。
- 验证要求：目标 crate 渲染与交互测试、最终 workspace fmt/Clippy、源码尺寸和差异检查；涉及协议的新增行为才追加本机 Docker 集成。通过后按独立功能提交并推送 main。
- 当前限制：Computer Use 的 node_repl 初始化报 `failed to write kernel assets`（Windows os error 3），重置内核并重新初始化后仍失败；使用 headless 与系统截图作为替代证据，真实窗口完整鼠标/键盘流程不标记为通过。
- 当前已知问题收口：本轮已完成筛选输入框统一、未选中输入框错误聚焦边框、首页/侧栏工具入口、内嵌工具图标、设置导航伪滚动条、容器/SSH 工作区对齐、主题令牌和数据库结果/Server Objects 布局修复；对应代码与 headless 回归已通过。后续跨工具新界面仍按本节标准逐项验收，不把这条记录扩大为所有工具的真实窗口完成。

### A-QUALITY-SETTINGS：公共偏好与工具设置归位（2026-09-28，代码与 headless 验收完成）

- 用户补充要求：必要公共配置集中在全局系统设置面板，必要工具配置归入对应工具设置面板。
- 系统设置：主题、界面字号、滚动条显示方式、窗口关闭行为；保留旧 `theme_mode` 和 `system_settings` 存储键，旧配置缺少新字段时使用兼容默认值。变更实时生效，主题切换不能重置滚动条或字号偏好。
- 工具设置：数据库、SSH、剪贴板沿用已有专属页；系统监控新增专属设置页，接收从监控工具栏移出的刷新频率，独立保存并通知已打开的监控视图。监控工具栏保留立即刷新和设置入口。
- 作用域规则：连接地址、凭据、API 单次请求参数、Kafka/MQTT 集群配置、Git 仓库配置与云存储账号仍在所属对象的编辑/设置面板内，不复制为应用级默认值；所有入口说明其作用范围。
- 验收：旧配置解析、保存后读取、主题往返切换、设置页跨尺寸布局、工具配置即时应用与工具入口跳转；最终 fmt、Clippy 和对应 UI 回归通过后独立提交并推送。
- 验收结果：公共系统设置、数据库/剪贴板/SSH/监控专属设置页和即时应用路径已接入；`ramag-ui` 设置、主题和入口回归 108 项通过，`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口流程仍受 Computer Use 环境限制，保留替代证据边界。

### A-TOOL-NAV-001：剪贴板入口可见与首页卡片导航（2026-09-27，已修复与 headless 验收）

- 问题证据：剪贴板采集/全局热键开关曾同步设置 `ToolRegistry` 的入口启用状态，关闭后台采集会同时从首页和活动栏移除入口；首页卡片原先通过 action 分发打开工具，点击后不能稳定切换到详情视图。
- 设计：采集开关只控制后台采集与热键，不改变工具可见性；首页卡片发出带工具 ID 的打开事件，由 `Shell::set_home_view` 统一订阅并导航，保持首页入口与活动栏使用同一工具详情路由。
- 验收条件：首页和侧边栏均能定位剪贴板入口；点击首页卡片后 `Shell` 选中剪贴板并显示工具详情；工具注册、Shell UI/headless 交互、workspace fmt/Clippy、源码尺寸和差异检查通过。
- 不做事项：不改变剪贴板存储、采集策略或热键默认值；不依赖 WebView，也不要求采集开启后才可查看已有剪贴板历史。
- 实施：启动时不再将剪贴板采集设置同步成工具隐藏状态；运行时热键设置变化只更新采集与唤起行为。`Shell::set_home_view` 持有并订阅首页事件，首页卡片按工具 ID 导航。
- 验收结果：GPUI headless 测试 `clipboard_is_visible_in_home_and_sidebar_and_opens_detail` 检查剪贴板卡片、活动栏入口、点击后的选中 ID 和详情视图；`ramag-bin` 的 `clipboard_tool_is_registered_last` 确认 Windows 内置注册表包含剪贴板。相关 UI 106 项、系统工具 21 项、主程序 15 项测试通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 窗口证据：Windows 最新调试程序启动并响应，系统截图 `target/ui-fallback/clipboard-activity-bar.png` 显示剪贴板工具入口；Computer Use 仍返回 `apps: []`，没有将该截图描述为 Computer Use，也没有宣称完成真实鼠标点击流程。首页卡片点击由 headless 交互测试验证。
- Git：修复提交 `12b8728e` 已推送 `main`；本节对应的专项 UI 回归和记录在后续提交中单独推送。

### A-CI-UI-002：修复跨平台桌面 UI 回归（2026-09-28，跨平台 CI 验收完成）

- 问题证据：`main` 的 GitHub Desktop CI 在 Windows、macOS 和 Linux workspace 测试中发现，表设计器 DDL 预览窗口高度 620px 时底部操作区超出视口；Linux 另有单元格值查看器测试点击关闭容器而非实际关闭按钮，导致关闭状态断言失败。
- 设计：表设计器按对话框实测标题、工具栏、分区标题、底部操作和内边距调整正文预留高度，保留 DDL 内部滚动；值查看器测试定位实际关闭按钮中心执行点击，验证关闭对话框，不改关闭业务逻辑。
- 验收条件：两个定向 GPUI 测试和 `ramag-tool-dbclient` 全量测试通过；workspace fmt、Clippy、源码尺寸和差异检查通过；推送后 Desktop CI 的 Windows/macOS/Linux workspace 测试均通过。
- 不做事项：不改变表结构 SQL、数据写入、对话框最大宽高策略或值查看器内容行为；不通过放宽断言或跳过平台测试规避回归。
- 实施与本机验收：DDL 预览正文预留高度从 170px 调整为 194px，给响应式标题、工具栏和底部操作留足空间；值查看器关闭测试现在点击带调试选择器的实际按钮。两个定向 GPUI 测试通过，`ramag-tool-dbclient` 全量 354 项和 `cargo test --locked --workspace --all-targets -- --test-threads=1` 通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 跨平台验收：提交 `39a577ee` 的 GitHub Desktop CI run `36331218913` 在 Windows x64、Linux x86_64 和 macOS 三个平台全部通过，包括 workspace 全量测试、fmt、Clippy 和源码检查。本次没有使用 WSL 验证。

### 阶段 A：单机桌面功能收口（当前主线，与阶段 B 并行推进）

当前已完成 `SHELL-001`、`DB-RED-01`、`DB-RED-03`、`DB-RED-04`、`DB-RED-05A` 至 `DB-RED-07`、`DB-UX-003A` 至 `DB-UX-003C-2`、`DB-UX-004A`、`DB-UX-004B-1` 至 `DB-UX-004B-3B`、`DB-UX-005A` 至 `DB-UX-005F`、`PLAT-004`、`PLAT-005-A` 至 `PLAT-005-F`、`TOOL-MIG-001`、`DUAL-CORE-001`、`CATALOG-001` 和 `COLLAB-001-A/B1/B2/B3` 的代码与专项验证。涉及真实数据库的切片另有 MySQL 8.4/PostgreSQL 17 Docker 证据。

阶段 A 的执行顺序固定为：

1. `A-DB-005`：已完成 MySQL 8.4 与 PostgreSQL 17 的迁移脚本回放和 PostgreSQL 失败回滚验证。
2. `A-DB-RED-02`：已完成 MySQL 8.4/PostgreSQL 17 Docker 元数据和 `table_tree` headless 复验；真实 Windows 流程仍待补。
3. `A-UI-REAL`：真实窗口探测受 Computer Use 环境阻塞，状态保持未完成；不把替代证据写成真实窗口通过。
4. `A-P0C`：Windows 主线已完成系统凭据库、主密钥和秘密上下文真实环境验收；Linux Secret Service 与 macOS Keychain 仍待各自环境验收。
5. `A-PLAT-005`：已完成不依赖窗口接管的真实 JSON Path 入口运行指标、任务回收和 WSL 当前进程内存多场景记录；Windows UI Automation/截图替代验收已完成，Linux/macOS、发布构建和 Computer Use 真实窗口证据仍待补。
6. `A-QUALITY`：完成性能、主题一致性和发布证据收口。

Computer Use 当前仍无法发现可操作的原生窗口，因此 `A-UI-REAL` 的鼠标/键盘证据保持未完成。`A-PLAT-005` 已补齐 WSL headless 进程内存基线和 Windows UI Automation/截图替代证据，但 Linux/macOS、发布构建和 Computer Use 真实窗口证据仍待分别验收。阶段 B 已在平台壳层、权限边界和 headless/Docker 证据满足后并行推进；真实窗口缺口只限制对应窗口证据，不阻止已设计并通过专项验证的原生工作区切片。

未完成的旧 `UI-001`、`M1-M4`、`R` 系列或工具专项事项必须先映射到以上切片 ID，并重新满足统一 UI 标准，不能只修改状态文字宣称完成。

### 阶段 B：复杂工具原生工作区迁移（当前主线，与阶段 A 收口并行）

按以下顺序逐个推进：`B-API-001`、`B-KAFKA-001`、`B-SSH-001`、`B-CONTAINER-001`、`B-GIT-001`。每个切片只覆盖一个工具，保留自己的连接、取消、错误、权限和领域数据模型；通过 headless 与真实服务或本机 Docker 验收后再进入下一项，真实窗口证据在 Computer Use 恢复后补齐。

### 阶段 C：原生协同画布（后置）

`CANVAS-001..006` 继续后置，不进入当前开发队列。未来在 Rust/GPUI 中参考 Excalidraw 的无限画布、场景元素、撤销重做、导出和协同体验；先使用 Ramag 自有 JSON 格式，再评估 `.excalidraw` 兼容和实时 Relay。桌面端不使用 WebView。

`COLLAB-001-B4`、生产 Relay、账号权限、第三方动态插件、插件市场、签名、沙箱和升级回滚同样后置；现有 B1/B2/B3 只作为本机优先共享能力和验证服务保留。

### CI-LINUX-001：Linux 构建缺少协作基础设施依赖（2026-09-27，已修复）

- 问题：`ramag-bin` 在 Linux CI 中无条件导入 `ramag_infra_collaboration::HttpCollaborationRelay`，但该 crate 只声明在 macOS/Windows 的 target 依赖段，导致 `error[E0432] unresolved import ramag_infra_collaboration`。
- 修复：将 `ramag-infra-collaboration` 移到 `ramag-bin` 的跨平台依赖区；保留剪贴板依赖的 macOS/Windows 条件，不修改 Relay 协议或运行时行为。
- Linux 回归修复：跨平台依赖生效后，Linux 插件注册表会包含协作工具；同步更新 Linux 专项断言，避免把已注册的 `collaboration` 误判为多余工具。
- 验证：在本机 WSL2 `Ubuntu-24.04`（x86_64）按 Linux CI 使用 `cargo check-all` 通过；修复断言后运行 `cargo test --locked -p ramag-bin --all-targets -- --test-threads=1`，20 项通过。Windows MSVC 同一目标测试 15 项通过，`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查、日志约定检查和 `git diff --check` 通过。
- Git：提交 `490e4e18`（`fix: include collaboration transport on linux`）和 `97a108f4`（`fix: update linux plugin registry expectation`）均已推送 `main`。

### A-DB-005：迁移差异回放收口（2026-09-27，Docker 验证完成）

- 目标：用真实 MySQL/PostgreSQL 服务回放当前迁移生成器覆盖的多语句脚本，确认字段、索引、主键和外键动作能够回读；确认 PostgreSQL 事务失败时保留旧结构。
- Docker：复用本机已运行且健康的 `ramag-db-test-mysql`（`mysql:8.4`，`127.0.0.1:13306`）和 `ramag-db-test-postgres`（`postgres:17-alpine`，`127.0.0.1:15432`）。测试创建带进程后缀的临时表/Schema，并在测试末尾删除；本次不停止用户正在复用的容器、网络或数据卷。
- 验收结果：设置 `RAMAG_TEST_MYSQL_*` 后运行 `cargo test --locked -p ramag-infra-mysql --test migration_replay -- --nocapture`，1 项通过；设置 `RAMAG_TEST_PG_*` 后运行 `cargo test --locked -p ramag-infra-postgres --test migration_replay -- --nocapture`，2 项通过，包含失败回滚。未把未设置环境变量时的跳过结果计为通过。
- 未完成项：迁移对话框的真实 Windows 鼠标/键盘流程仍受 Computer Use 环境限制；`A-DB-RED-02` 已完成，下一项进入 `A-UI-REAL`。

### A-DB-RED-02：对象树真实元数据复验（2026-09-27，Docker 与 headless 验证完成）

- Docker：复用本机健康的 `ramag-db-test-mysql`（`mysql:8.4`，`127.0.0.1:13306`）和 `ramag-db-test-postgres`（`postgres:17-alpine`，`127.0.0.1:15432`）；MySQL `list_` 集成测试 4 项、PostgreSQL `list_` 集成测试 4 项、两端临时触发器测试各 1 项通过。测试创建的临时对象由测试清理，未停止用户复用的容器、网络或数据卷。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib table_tree -- --nocapture` 通过 42 项，覆盖 Schema 刷新保留旧树、表/列/键/索引/触发器分组、搜索、过滤、连接隔离和窄窗口布局。
- 结果：`DB-RED-02` 的真实驱动元数据、表树代码和 headless 交互达到当前矩阵要求；真实 Windows 鼠标/键盘流程仍未验收，不能把本记录写成完整 UI 通过。
- 下一项：`A-UI-REAL`，先确认 Computer Use 能否稳定发现并操作 Ramag 原生窗口。

### A-UI-REAL：真实 Windows 窗口证据探测（2026-09-27，未完成）

- Computer Use 探测：启动前后 `cua.getState()` 均返回 `apps: []`；本机实际启动 `target/debug/ramag.exe` 后，进程存在且窗口标题为 `Ramag — 数据库客户端`，但 Computer Use 仍无法发现可操作窗口。
- 替代证据：在确认 Computer Use 不可用后，使用系统窗口截图 `target/ui-fallback/ramag-window.png` 和 Windows UI Automation 检查原生窗口，发现 34 个按钮节点并成功调用“数据结果”页签一次；headless 交互测试继续作为布局和状态证据。
- 证据边界：替代证据只确认窗口可启动、主要控件可被系统 UI Automation 发现并完成一次安全页签调用，不能替代 Computer Use 的完整鼠标/键盘、滚动、编辑和截图流程。探测结束后已停止临时 Ramag 进程。
- 状态：`A-UI-REAL` 继续保持未完成，等待 Computer Use 能稳定发现窗口后再补验完整数据库、插件目录和 JSON Path 流程；不因此修改既有功能完成状态。

### A-P0C：系统凭据库、主密钥和秘密上下文真实环境验收（2026-09-27，Windows 已完成）

- 设计：只使用临时 redb 文件验证真实 Windows Credential Manager、主密钥派生的加密存储和插件上下文装配；不删除或重建现有主密钥，不触碰生产数据库和用户保存的插件偏好。
- 环境证据：`cmdkey /list` 只检查凭据元数据，确认存在 `LegacyGeneric:target=master-key.ramag`、账户名为 `master-key` 的 Credential Manager 条目；没有读取或输出凭据值。
- 真实验收：临时辅助程序运行 `RedbStorage::open`，使用现有系统主密钥打开临时数据库；`PluginSecretStore::save` 写入敏感设置后，原始偏好值以 `encrypted-v1:` 开头且不包含明文；`StaticPluginHost` 获得 `storage.plugin` 授权后成功把秘密快照装入 `PluginContext`；释放并重新打开 `RedbStorage` 后再次读取相同快照。辅助程序、临时数据库和生成的临时文件均已清理。
- 结果：Windows Credential Manager → redb AES-GCM → `PluginSecretStore` → `StaticPluginHost`/`PluginContext` 的真实链路通过；本次没有记录秘密正文，也没有调用删除主密钥的调试接口。
- 边界：本记录只覆盖当前 Windows 桌面主线；Linux Secret Service、macOS Keychain、正式安装包升级/迁移和真实窗口操作仍需独立验收，不把本记录扩展为跨平台或发布完成声明。
- 状态：`A-P0C` Windows 主线完成；`A-UI-REAL` 仍受环境阻塞，下一项推进 `A-PLAT-005` 的 headless 运行指标切片。

### A-PLAT-005：真实工具入口运行指标和任务回收（2026-09-27，headless 运行链路完成）

- 设计：沿用 `StaticPluginHost::execute_entry` 的输入、输出、超时和生命周期边界，在不改变现有 `join()` 返回类型的前提下增加 `join_with_metrics()`；成功结果提供执行耗时和输出字节数，供后续首帧、资源和发布记录使用。
- 实现：`PluginTaskExecution` 在提交任务时记录单调时钟；成功结果通过 `PluginTaskCompletion` 暴露受预算保护的结果、`elapsed()` 和 `output_bytes()`，失败、取消和超时通过 `PluginTaskOutcome` 保留同样的耗时与输出边界。普通工具继续使用 `join()`，不会改变现有插件调用方。
- 真实入口：`ramag-tool-json-path` 注册并初始化 `it-tools.json-path/json-path-extractor`，执行 JSON5 请求，读取指标后再次提交同一入口，确认 `join()` 会释放活动任务名额。
- 验证：`cargo test --locked -p ramag-app plugin_tasks --lib -- --test-threads=1`（8 项通过）；`cargo test --locked -p ramag-tool-json-path real_entry_reports_headless_metrics_and_releases_task_slot --lib -- --nocapture --test-threads=1` 通过，当前运行记录为 activation `557.7µs`、execution `409.5µs`、output `7` bytes（数值随机器变化，不作为固定性能承诺）。
- 证据边界：该记录覆盖真实静态插件入口的 headless 执行、结果预算和任务回收，宿主取消路径同时记录 `PluginTaskOutcome`；不覆盖真实窗口首次打开、稳定的空闲内存长期基线或 Computer Use 鼠标/键盘。进程内存阶段样本由 `A-PLAT-005-F` 补充，跨平台和发布环境测量仍需分别进行。
- 状态：`A-PLAT-005` 的 JSON Path 运行指标子切片完成；后续进程内存基线和多场景回放已由 `A-PLAT-005-F` 补充，真实窗口缺口仍由 `A-UI-REAL` 单独跟踪。

### A-PLAT-005-UI：JSON Path 原生入口与资源采样验收（2026-09-27，替代窗口证据完成）

- 真实流程：启动最新 x64 MSVC `ramag.exe`，切换到 JSON Path 提取器，通过 Windows UI Automation 调用“提取”；输入 `$.users[*].name` 返回 `Alice`、`Bob` 两项，未访问网络或远端服务。
- 证据：系统窗口截图 `target/ui-fallback/json-path-extract.png` 保存 JSON5 输入、路径、提取按钮、完成状态和结果；UI Automation 发现入口控件并成功调用一次提取。
- 资源采样：同一进程在 JSON Path 窗口激活后记录 Working Set 约 `113.9 MiB`、Private Bytes 约 `112.3 MiB`；另一次冷启动采样为 Working Set `110.7 MiB`、Private Bytes `111.2 MiB`。数值只作为当前 Windows 调试构建的可复查样本，不作为跨机器预算承诺。
- 证据边界：Computer Use 运行时仍只返回浏览器且 `apps: []`，本记录使用真实 Windows 窗口截图、UI Automation 和进程采样作为替代证据；Linux/macOS、发布构建和 Computer Use 完整流程仍未验收。

### A-PLAT-005-F：真实入口进程内存基线与多场景回放（已完成，2026-09-29）

- 问题证据：插件入口运行指标目前只有任务耗时和输出字节数；Windows 替代验收虽然记录过 Working Set 与 Private Bytes，但 Rust headless 回放没有统一的当前进程采样接口，也没有把首次初始化、首次执行和再次执行的内存样本放在同一份记录中。
- 设计：在 `ramag-app` 增加只读取当前进程的跨平台内存采样函数，返回 PID、常驻内存和虚拟内存；操作系统无法提供数据时返回 `None`，不改变插件任务预算、取消、超时或结果上限。扩展 JSON Path 真实入口回放，在初始化前、初始化后、首次执行后和再次执行后记录采样，同时保留入口结果、输出字节数和任务名额释放检查。
- 验收条件：应用层单元测试确认采样结果只对应当前 PID；JSON Path 真实入口测试在 WSL 中输出四个阶段的内存字节数，确认入口执行成功、再次执行成功并释放任务名额；超出入口输入上限时被宿主拒绝；目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不把采样值转成固定跨机器预算，不在运行时 UI 增加常驻监控面板，不改变任务预算或插件权限，不把单个真实入口回放扩展成动态插件、外部进程或真实 Windows Computer Use 流程。
- 实现：`ramag-app::current_process_memory` 只采样当前进程，返回 PID、常驻内存和虚拟内存；操作系统无法提供当前进程数据时返回 `None`。JSON Path 真实入口回放在初始化前、初始化后、首次执行后和再次执行后记录样本，并保留成功结果、输出字节数、任务名额释放和超大输入拒绝检查。
- 验收结果：`cargo test --locked -p ramag-app --lib -- --test-threads=1` 的 285 项测试通过；`cargo test --locked -p ramag-tool-json-path --all-targets -- --test-threads=1` 的 5 项测试通过；workspace fmt、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 均通过。
- WSL 采样记录：同一 JSON Path 入口的常驻内存为 `13.0 → 15.6 → 17.8 → 17.8 MiB`，虚拟内存为 `162.8 → 162.8 → 295.0 → 295.0 MiB`；首次和再次执行都返回 `Alice`，输入超过 `MAX_JSON_INPUT_BYTES` 时由宿主拒绝。字节数是本次 WSL 回放的复查样本，不是跨机器预算承诺。
- 证据边界：本切片证明 WSL 中当前进程采样、真实静态入口初始化、首次/再次执行、结果上限和任务回收链路；不证明 Windows/Linux/macOS 发布构建的统一数值、真实窗口首帧或 Computer Use 鼠标/键盘流程。
- 提交：设计确认已在 `26cad8fc` 提交，实现和测试已在 `b2f77e0f` 提交；两个提交均已推送到 `origin/main`。

### B-API-001-A：协议切换取消活动请求（设计确认，2026-09-27）

- 问题证据：API 工作区在 HTTP/gRPC 请求执行期间切换协议时，原逻辑只清理编辑器和 gRPC 发现状态；活动请求的取消标记与 `request_generation` 没有同步失效，迟到的旧响应仍可能覆盖新协议工作区。
- 设计：`ApiView::set_protocol` 在切换协议前取消当前请求，递增请求代际并清除活动句柄；已有 gRPC Service/Method 发现取消逻辑保持不变。请求驱动、认证、环境变量和响应模型不改动。
- 验收条件：headless 测试先创建活动请求标记，再切换协议，必须确认旧标记已取消、请求代际变化、加载状态结束和迟到结果不会回写；HTTP/gRPC 控件仍在 1024x768 布局中可见。
- 不做事项：不改变远端协议、重试策略、响应格式或工作区持久化；真实 Docker HTTP/gRPC 回放继续由 `B-API-001` 总切片负责。
- 验收结果：新增协议切换回归测试，确认旧请求取消标记、请求代际、加载状态和句柄均失效；API crate 34 项测试、workspace fmt、Clippy、源码尺寸和 diff 检查通过。未宣称真实窗口或 Docker 回放，下一项为 `B-API-001-B`。

### B-API-001-B：用户取消后的请求代际清理（设计确认，2026-09-27）

- 问题证据：用户取消请求时，当前逻辑会设置取消标记、递增请求代际并结束加载状态，但没有清除 `cancelled` 句柄；迟到回调因代际不匹配直接返回后，旧句柄会残留在视图中，后续操作仍会把它误认为活动请求。
- 设计：`ApiView::cancel` 将“加载中或存在取消句柄”视为活动请求，先设置取消标记，再递增请求代际、结束加载并清除句柄；gRPC Service/Method 发现继续使用独立代际和取消句柄。迟到请求只能因代际不匹配丢弃，不能修改新状态。
- 验收条件：headless 测试创建活动请求后点击取消，必须确认取消标记已设置、加载结束、句柄清除、请求代际递增和“请求已取消”提示保留；现有 HTTP/gRPC 取消和 1024x768 工具栏布局回归通过。
- 不做事项：不改变远端协议、请求重试、响应格式、历史持久化或 gRPC 发现语义；真实 Docker HTTP/gRPC 回放继续由 `B-API-001` 总切片负责。
- 验收结果：取消回归测试确认旧句柄清除、请求代际递增、加载状态结束、提示保留和取消标记设置；API crate 34 项测试、workspace fmt、Clippy、源码尺寸和 diff 检查通过。未宣称真实窗口或 Docker 回放，下一项进入 `B-API-001` 的请求/响应边界复验。

### B-API-001-C：有界响应正文复制（设计确认，2026-09-27）

- 问题证据：API 响应面板已经按 `MAX_API_RESPONSE_BODY_BYTES` 保留正文并显示截断状态，但正文没有复制入口；用户无法把当前已保留内容带到日志、工单或后续请求中。
- 设计：在响应正文标题行增加复制按钮；点击时从当前 `ApiResponseSnapshot.body` 生成 UTF-8 有损文本并写入系统剪贴板，不重新读取网络响应，也不突破响应缓冲区上限。正文是否截断仍由耗时页明确显示，复制动作只复制当前已保留内容。
- 验收条件：headless 测试确认有响应时复制控件可见，点击后剪贴板等于有界正文；无响应时不显示控件。API 响应标签、窄窗口布局和已有正文截断测试继续通过。
- 不做事项：不增加完整响应缓存、不改变传输层上限、不承诺二进制无损复制，不引入 WebView 或远端服务。
- 验收结果：API 工作区 35 项测试通过；本机 Docker HTTP `ramag-api-http-test`（`python:3.12.11-alpine3.22`，`127.0.0.1:18089`，代理 `18093`）驱动测试 1 项通过；gRPC `ramag-api-grpc-test`（`rust:1.91.0-bookworm`，`127.0.0.1:18090`，代理 `18094`）驱动测试 1 项通过；API GPUI 工作区 HTTP/gRPC 联调 1 项通过。容器保持运行供后续复用，未宣称真实窗口验收；下一项进入 `B-KAFKA-001` 设计确认。

### B-API-001-UI：API 工作区 Windows 窗口与 Docker 请求验收（2026-09-27，替代窗口证据完成）

- 环境：复用本机健康的 `ramag-api-http-test`（`python:3.12.11-alpine3.22`，`127.0.0.1:18089`）和 HTTP 代理 `ramag-api-http-proxy-test`（`127.0.0.1:18093`）；容器未停止，未改变测试数据。
- 真实流程：启动最新 x64 MSVC `ramag.exe`，通过 Windows UI Automation 切换到 API 工作区并调用“发送”；工作区使用已有 `base_url=http://127.0.0.1:18089` 环境变量完成 GET 请求，窗口回显 `HTTP 200 · 3 ms · 43 bytes`、断言通过和 JSON 正文 `ok=true`。
- 证据：系统窗口截图 `target/ui-fallback/api-send-docker.png` 保存了请求编辑、环境变量、发送按钮、响应状态和 JSON 结果；UI Automation 在发送前后均发现 47 个可访问控件，并通过 `InvokePattern` 调用发送按钮。
- 证据边界：Computer Use 运行时仍只返回浏览器且 `apps: []`，因此本记录使用真实 Windows 窗口的系统截图、UI Automation 和本机 Docker 回包作为替代证据；不宣称 Computer Use 鼠标/键盘流程已经恢复。API 工作区真实窗口仍需在 Computer Use 恢复后补验，下一项继续 `B-KAFKA-001`。

### B-KAFKA-001-A：Kafka 集群运行上下文可取消（设计确认，2026-09-27）

- 问题证据：Kafka 工作区切换集群或刷新时会并行读取 Metadata 和 Topic；概览页只有“同步中”骨架和刷新入口，用户无法主动停止较慢的 Broker 请求。取消只能依赖切换集群或销毁窗口，反馈不清晰。
- 设计：在概览页加载状态的状态条增加“取消同步”按钮；按钮调用 `KafkaView::cancel_runtime_load`，设置底层取消标记、递增运行请求代次、清理刷新指标并保留已经存在的旧快照。取消后的迟到 Metadata/Topic 结果因代次和集群上下文不匹配而丢弃。已有“刷新元数据”按钮继续用于重新发起完整读取。
- 验收条件：headless 测试在 `360x640`、`1024x768` 和 `1440x900` 验证取消按钮位于加载状态条内且不越界；点击后确认加载状态结束、取消提示保留、取消标记已设置、运行代次递增，迟到结果不能写回。Kafka 工作区回归、fmt、Clippy、源码尺寸和 `git diff --check` 全部通过。
- 不做事项：不修改 Kafka 协议、Broker 超时、重试策略、旧快照内容或消息读取取消语义；不引入 WebView、远程 Relay 或新的连接配置。
- 验收结果：`cargo test --locked -p ramag-tool-kafka --lib -- --test-threads=1` 38 项通过；新增 `kafka_loading_tables_keep_stable_geometry` 覆盖取消标记、运行代次、提示、旧快照边界和 `360x900`/`1200x780` 加载布局；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。最新 `ramag-bin` 已重新构建。
- 证据边界：本会话的 Computer Use 运行时只暴露浏览器 API，原生 `@oai/sky` 的 `list_apps`/`get_window` 未提供，无法执行真实窗口点击；该切片保留 headless 通过，真实窗口证据待运行时恢复后补验。

### B-KAFKA-001-A-UI：Kafka 集群同步取消 Windows 窗口验收（2026-09-27，替代窗口证据完成）

- 真实流程：启动最新 x64 MSVC `ramag.exe`，切换到 Kafka 工作区并选择已有 `Ramag-Docker-Kafka` 配置（`127.0.0.1:19092`），在概览同步状态出现后通过 Windows UI Automation 调用“取消同步”。
- 结果：窗口显示“集群同步已取消；已有快照保持不变，迟到结果不会写入当前页面”；系统窗口截图保存为 `target/ui-fallback/kafka-cancel-flow.png`，UI Automation 在同步中状态发现 30 个控件并成功调用取消按钮。
- 服务边界：本次取消流程在 Broker 请求返回前完成，未把失败回包写成成功；Kafka Docker 回读另由 `B-KAFKA-001-B` 记录，不把取消流程本身扩大为成功连接验收。
- 证据边界：Computer Use 运行时仍只返回浏览器且 `apps: []`，本记录使用真实 Windows 窗口截图、UI Automation 和取消状态作为替代证据；Computer Use 完整流程待运行时恢复。

### B-KAFKA-001-B：Kafka 本机 Docker 连接、消息回读与工作区依赖验收（2026-09-28，代码与 Docker 验证完成）

- 设计确认：使用仓库现有 Kafka 工作区和生产驱动完成真实 Broker 元数据、Topic、消息、消费组、偏移、生产回读、Connect、Schema Registry、ksqlDB 和 Broker 指标边界验证；不改变 Kafka 协议、连接配置模型、读写权限或工作区状态结构。
- 测试环境：本机 Docker Compose 项目 `ramag-kafka-test`，`apache/kafka:4.0.0`（`127.0.0.1:19092`）、Kafka Connect（`127.0.0.1:18083`）、Schema Registry（`127.0.0.1:18081`）、ksqlDB（`127.0.0.1:18088`）、OpenMetrics fixture（`127.0.0.1:19100`）和 JMX exporter（`127.0.0.1:19101`）。启动前构建 `ramag-kafka-jmx-exporter:1.6.0`，所有服务健康检查通过。
- 测试数据：`scripts/kafka-test/kafka-test.ps1 -Command seed -MessageCount 5000` 创建 3 分区的 `ramag.integration.messages` 并写入 5,000 条确定性消息，同时创建 60 个不同名称/分区数的 UI Topic；准备 ksqlDB 流和两个 Schema Registry 版本。
- 验收命令：`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/kafka-test/kafka-test.ps1 -Command test`。结果为 native `docker_kafka.rs` 12 项通过，pure-Rust `docker_kafka_pure_rust.rs` 1 项通过；覆盖元数据、61 个 Topic、5,000 条消息按 offset/time 读取与搜索、消费组和 offset 重置、生产后回读、Topic 管理、Connect、Schema Registry、ksqlDB 和 JMX 指标。
- 清理结果：测试结束执行 `scripts/kafka-test/kafka-test.ps1 -Command clean`，专用容器、网络和 `ramag-kafka-test-data` 卷均已删除；未影响其他本机 Docker 服务。
- 证据边界：本机 Docker 和 headless/替代窗口证据通过；Computer Use 仍无法发现原生窗口，因此 Kafka 工作区完整鼠标/键盘流程仍未验收，不将系统截图或 UI Automation 描述为 Computer Use 证据。
- 状态：`B-KAFKA-001` 的代码、headless、取消流程替代证据和本机 Docker 成功回读已完成；保留原生 Computer Use 窗口证据缺口。下一项进入 `B-SSH-001` 设计确认。

### B-SSH-001-A：本机 OpenSSH/SFTP 工作区端到端验收（设计确认，2026-09-28）

- 问题证据：SSH 工具已有终端、SFTP、文件预览、传输队列和端口转发代码及 82 项 headless 测试，但当前主线缺少可重复的本机 Docker OpenSSH 服务验收记录；没有真实服务证据就不能确认连接、目录、上传下载、保存、重命名、归档和清理链路。
- 设计：新增专用 `scripts/ssh-test` Docker 测试环境，使用固定的 Debian 12 OpenSSH 服务和临时 Ed25519 测试密钥；测试脚本负责生成密钥、启动健康检查、设置 `RAMAG_TEST_SSH_*` 环境变量、运行现有 `ramag-infra-ssh/tests/integration.rs`，最后删除容器、网络、卷、密钥和临时目录。测试只访问本机 Docker，不使用远程集群、真实账号或生产主机。
- 验收范围：真实 SSH 连接探测、SFTP 目录创建与列表、256 KiB 上传及进度、完整预览与尾部读取、远程保存、下载回读、重命名、目录归档、归档内容核验、远端清理和驱动关闭；失败路径必须保留有界错误并执行清理。
- 不做事项：不在本切片新增 SSH 协议、不改变 Host Key 策略、认证模型、生产只读策略或终端 UI；端口转发、JumpServer 和真实窗口鼠标/键盘流程继续分别记录，不把 Docker 驱动回读扩大为 Computer Use 证据。
- 验收条件：headless SSH UI、Docker OpenSSH/SFTP 集成、`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 全部通过；记录镜像、端口、健康状态、启动和清理结果。Computer Use 不可用时明确保留原生窗口缺口。
- 实施顺序：先提交本设计确认，再实现 Docker 测试脚本和 compose 配置，随后运行真实集成测试并独立提交；通过后进入 `B-SSH-001-B` 的终端/端口转发回读。

### B-SSH-001-A：114 服务器 OpenSSH/SFTP 端到端验收（代码与远端验证完成，2026-09-28）

- 代码：集成测试支持通过当前进程的 `RAMAG_TEST_SSH_PASSWORD` 选择密码认证，并可指定构建后的 `ramag` AskPass 辅助程序；密码只在内存中的一次性 AskPass 通道内使用，不写入命令参数、仓库或测试输出。生产驱动默认行为不变。
- 测试服务器：使用 `10.17.17.114` 的 `csnt` 账户和 SSH/SFTP 服务，密码由既有 `CODEX_DEPLOY_SSH_PASSWORD` 系统环境变量经 `SSH_ASKPASS` 读取；测试没有输出密码，也没有访问部署目录 `/home/csnt/architecture/docker-compose-all/`。
- 验收范围：在服务器 `/tmp` 下创建带唯一后缀的测试目录，完成连接探测、SFTP 目录创建与列表、256 KiB 上传及进度、完整预览、尾部读取、远程保存、下载回读、重命名、目录归档、归档内容核验、远端文件删除和驱动关闭；测试结束后删除该测试目录。
- 验收结果：`cargo test --locked -p ramag-infra-ssh --test integration -- --nocapture` 在 114 服务器上 1 项通过；`cargo test --locked -p ramag-infra-ssh --lib -- --test-threads=1` 65 项通过；`cargo test --locked -p ramag-tool-ssh --lib -- --test-threads=1` 85 项通过；`cargo build --locked -p ramag-bin`、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- 清理结果：114 上的唯一临时目录已删除；本机 SSH 测试容器、临时公钥和临时文件已清理，没有停止或修改其他本机服务。
- 证据边界：本次证明了 SSH/SFTP 驱动对指定服务器的真实回放和原生工作区 headless 测试；终端、端口转发、JumpServer 和 Computer Use 原生窗口流程仍属于 `B-SSH-001-B` 或后续验收，不把本次结果扩展为完整 SSH 工作区交付。

### B-SSH-001-B：114 服务器终端与端口转发回读（设计确认，2026-09-28）

- 问题证据：现有 SSH 工作区已经生成带 PTY 的终端命令，并由独立进程管理端口转发；现有 headless 测试覆盖命令参数、状态转换和进程回收，但还没有在 114 服务器上读取远端终端输出，也没有通过本地转发端口读取远端服务数据。
- 设计：集成测试使用 `OpenSshDriver` 生成终端和端口转发命令。终端测试通过 114 的 `csnt` 账户启动带 PTY 的远端 Shell，写入固定输出标记和退出命令，只保留有界输出并检查进程退出。端口转发测试在 114 的 `/tmp` 下启动只监听 `127.0.0.1` 的临时 TCP 回显服务，使用生成的本地 `-L` 转发命令连接本地端口，回读固定标记后停止转发进程并删除远端服务。
- 测试边界：密码继续由 `CODEX_DEPLOY_SSH_PASSWORD` 经 `SSH_ASKPASS` 读取；测试不得输出密码，不得写入仓库或命令行，不得访问 `/home/csnt/architecture/docker-compose-all/`，不得改变 114 上的常驻服务、SSH 配置或防火墙规则。
- 验收条件：114 真实终端回读确认标记和退出状态；114 真实端口转发回读确认本地连接、远端固定数据和停止后的端口释放；`ramag-infra-ssh`、`ramag-tool-ssh` 目标测试、`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 全部通过。Computer Use 不可用时，只记录真实服务和 headless 证据，不把它扩展为原生窗口鼠标/键盘验收。
- 不做事项：不新增 SSH 协议、不改变 Host Key 策略、认证模型、端口转发配置格式、终端 UI 或 JumpServer 行为；不把 114 的部署服务作为测试目标，不保留远端临时进程和目录。
- 实施顺序：先提交本设计确认，再补充真实终端/端口转发回读测试并独立提交；通过后进入 `B-CONTAINER-001` 设计确认。

### B-CONTAINER-001-A：Docker 资源筛选入口（设计确认，2026-09-28）

- 问题证据：容器应用服务和 Docker 适配器已经支持 `ContainerListQuery.search`，但工作区始终发送空筛选词；用户只能读取整页容器、镜像、网络或数据卷，无法按名称、镜像、标签或地址快速缩小结果。
- 设计：在 Docker 资源列表工具栏增加一个有界筛选输入和明确的“筛选”按钮。按钮读取当前输入并重新提交 `ContainerListQuery`；后端继续负责 trim、大小限制和不区分大小写匹配。切换资源、修改 Docker 地址或重新筛选时递增已有 `request_id`，迟到结果不能覆盖当前页面。
- 验收条件：headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认筛选输入及按钮位于内容区内且可见；应用服务测试确认筛选请求原样传到 `ContainerDriver`，Docker 适配器测试确认筛选结果和分页边界；`ramag-tool-container`、`ramag-app`、`ramag-infra-container-docker` 目标测试、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不新增日志、容器生命周期、Docker exec、Kubernetes 资源、Registry 凭据或真实窗口验收；不把本地 UI 筛选扩展成无上限的全量加载。
- 实施顺序：先提交本设计确认，再实现筛选入口和目标测试；通过后独立提交并进入容器详情/日志的下一项设计确认。

### B-CONTAINER-001-A：Docker 资源筛选入口（代码与 headless 验证完成，2026-09-28）

- 实现：容器、镜像、网络和数据卷页面新增有界筛选输入与“筛选”按钮；工作区把输入放入 `ContainerListQuery.search`，应用服务保持原值转交，Docker 适配器继续执行大小写不敏感匹配、分页和响应数量限制。输入和按钮在没有外部服务时仍显示为禁用状态，连接切换和迟到结果继续使用已有 `request_id` 隔离。
- 测试：`ramag-tool-container` 7 项通过，覆盖 `360x640`、`1024x768`、`1440x900` 下的筛选控件边界和查询构造；`ramag-app` 容器服务专项 7 项通过，确认筛选请求到达 `ContainerDriver`；`ramag-infra-container-docker` 7 项通过、1 项本机 Docker 测试按设计保持忽略，新增名称、镜像、标签、网络和端口筛选命中测试。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- 本机 Docker 回读：当前 Engine `29.7.2` 运行正常；`cargo test --locked -p ramag-infra-container-docker reads_local_engine_without_write_operations --lib -- --ignored --nocapture --test-threads=1` 通过，完成连接、概览、容器/镜像/网络/数据卷列表和分页读取。测试只读，没有创建、修改或删除本机资源。
- 证据边界：本切片的代码、headless、本地适配器单元和本机 Docker 只读回读均通过；没有宣称真实 Windows 原生窗口验收，镜像操作、容器生命周期和日志仍待独立切片。

### B-CONTAINER-001-B：Docker 历史日志读取（设计确认，2026-09-28）

- 问题证据：容器详情目前只能显示路径、环境变量键、挂载和网络数量，Docker 适配器没有日志读取接口，用户无法在工作区内查看容器输出，也没有统一的行数和字节上限。
- 设计：新增 Docker 历史日志查询模型，支持 `tail`、`since`、`until` 和 `timestamps`；从容器详情进入“日志”页面后读取 `stdout`/`stderr`，按日志流保留行信息。单次结果最多保留 5,000 行和 2 MiB，超出部分只记录丢弃数量并显示截断状态。应用层校验容器 ID、时间范围和查询上限，迟到结果继续由现有 `request_id` 隔离。
- 安全边界：本切片只在工作区内显示有界历史日志，不提供复制、导出或持续跟随；应用层对疑似密码、Token、Authorization 和私钥样式日志行整行隐藏，隐藏失败时不把原文送入 UI。日志不写入操作记录或普通配置。
- 验收条件：领域测试覆盖查询边界和时间顺序；应用服务测试确认查询转发与敏感行隐藏；Docker 适配器测试覆盖 stdout/stderr 映射、行数/字节上限和协议错误；headless 测试覆盖容器详情入口、日志页面和 `360x640`、`1024x768`、`1440x900` 内容边界；本机 Docker 读取专用日志容器并清理资源，fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现 follow、暂停/恢复、复制、导出、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再实现领域/应用/适配器日志接口和历史日志页面；本机 Docker 回读通过后独立提交，下一项再处理日志跟随和取消。

### B-CONTAINER-001-B：Docker 历史日志读取（代码与 Docker 验证完成，2026-09-28）

- 实现：新增 `ContainerLogQuery`、`DockerContainerLogs` 和 `ContainerDriver::container_logs`；Docker 适配器读取 stdout/stderr，按 5,000 行和 2 MiB 保留结果，并返回丢弃行数和截断状态。容器详情新增“查看日志”入口和独立日志页面，应用层隐藏密码、Token、Authorization、Bearer 和私钥样式日志行。
- 测试：`ramag-domain` 全量 234 项通过，`ramag-app` 全量 282 项通过，`ramag-infra-container-docker` 普通测试 8 项通过、2 项保持忽略，`ramag-tool-container` 8 项通过；headless 覆盖日志导航、日志输出和 `360x640` 内容边界。
- 本机 Docker：`cargo test --locked -p ramag-infra-container-docker reads_dedicated_container_logs_and_cleans_resource --lib -- --ignored --nocapture --test-threads=1` 通过；测试使用本地 `alpine:3.20`、`ramag.test-suite=container-logs` 标签读取 stdout/stderr，结束后专用容器已删除，现有本机服务未改变。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：本切片完成领域、应用、Docker 适配器、headless 和本机 Docker 历史日志回读；follow、停止/恢复、复制/导出、Kubernetes Pod 日志、Docker exec、容器生命周期写操作和真实 Windows 原生窗口仍未验收。

### B-CONTAINER-001-C：停止 Docker 历史日志读取（设计确认，2026-09-28）

- 问题证据：历史日志读取虽然有行数和字节上限，但当前页面没有停止入口；在 Docker 流响应较慢或连接异常时，用户只能等待请求结束或切换页面，旧结果也没有独立取消标记。
- 设计：为日志读取复用 `ContainerOperationCancellation`，应用服务保留无取消参数的兼容入口，并增加带取消标记的日志读取入口。日志流每次等待 Docker 响应前检查取消标记；页面点击“停止读取”后设置标记、递增 `request_id`、结束加载状态并丢弃迟到结果。
- 验收条件：应用层测试确认已取消请求不会进入 Docker；Docker 适配器测试确认流读取返回 `ContainerErrorCategory::Cancelled`；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认停止按钮位于日志工具栏内，点击后加载结束、提示保留且迟到结果不能覆盖页面；workspace 目标测试、fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现 follow、暂停/恢复、复制、导出、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再实现取消标记、页面停止入口和目标测试；通过后独立提交，下一项处理 follow 的持续输出边界。

### B-CONTAINER-001-C：停止 Docker 历史日志读取（代码与 headless 验证完成，2026-09-28）

- 实现：日志读取增加 `ContainerOperationCancellation` 入口，保留无取消参数的兼容方法；Docker 流等待期间使用取消分支，页面新增“停止读取”按钮。停止时设置标记、递增 `request_id`、结束加载并保留提示，迟到日志不会写回。
- 测试：`ramag-app` 容器服务专项 8 项通过，确认预先取消不会进入驱动；`ramag-infra-container-docker` 普通测试 10 项通过、2 项真实 Docker 测试保持忽略，新增待处理日志流取消测试；`ramag-tool-container` 8 项通过，覆盖日志停止按钮在窄窗口内的边界。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：本切片证明了应用层预先取消、适配器等待中的取消分支和 headless 停止入口；follow、暂停/恢复、复制/导出、Kubernetes Pod 日志、Docker exec、容器生命周期写操作和真实 Windows 原生窗口仍未验收。

### B-CONTAINER-001-D：Docker 日志持续读取（设计确认，2026-09-28）

- 问题证据：当前工作区只能读取一次性的历史日志；容器继续输出新日志时，用户必须离开日志页面再重新打开，无法观察正在运行的容器。
- 设计：新增 Docker 日志持续读取接口，复用 `ContainerLogQuery`、`ContainerOperationCancellation` 和现有敏感行隐藏规则。适配器使用 Docker `follow=true` 读取日志流，把每个完整日志行交给有界 sink；应用层只转发已校验的 Docker 请求，页面用有界异步通道接收行并追加到当前日志窗口。
- 背压和生命周期：页面通道固定容量；通道暂时写满时，适配器暂停读取下一条日志并重试，不丢弃已经从 Docker 取出的日志，也不继续增加内存。用户停止、切换容器、切换页面或页面销毁时设置取消标记并关闭接收端；适配器遇到关闭的 sink 时结束远端日志读取。
- 窗口限制：持续窗口最多保留 `MAX_CONTAINER_LOG_LINES` 行和 `MAX_CONTAINER_LOG_BYTES` 字节，超过限制时从最早行开始移除并累计移除数量；历史日志的“读取截断”状态与持续窗口的“滚动移除”状态分开显示。持续读取不写入历史记录、普通配置或操作日志。
- 验收条件：领域测试覆盖持续读取 sink 结果和窗口滚动边界；应用服务测试确认 Docker 校验、取消转发和敏感行隐藏；Docker 适配器测试确认 `follow=true`、有界 sink 回压、关闭 sink 和取消均能结束读取；headless 测试确认 `360x640`、`1024x768` 和 `1440x900` 下持续读取/停止按钮及日志窗口不越界；本机 Docker 使用专用日志容器回读至少一条初始日志和一条后续日志，停止后无测试容器残留；目标测试、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现暂停展示、复制、导出、自动重连、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收；不把持续窗口扩展为无界日志存储。
- 实施顺序：先提交本设计确认，再实现领域 sink、应用转发、Docker 流读取、页面控制和目标测试；本机 Docker 回读通过后独立提交，下一项再处理暂停展示或复制导出中的一个明确边界。

### B-CONTAINER-001-D：Docker 日志持续读取（代码与 Docker 验证完成，2026-09-28）

- 实现：新增 `ContainerLogSink` 和 `follow_container_logs` 接口；Docker 适配器使用 `follow=true` 保持日志 HTTP 流，按 stdout/stderr 分别组装跨网络分片的完整日志行。应用层沿用敏感行隐藏，页面通过容量为 128 的有界通道接收日志，并在 5,000 行、2 MiB 窗口内滚动移除最早内容。
- 生命周期：通道满时适配器暂停读取并重试当前日志行；停止按钮、切换容器、切换页面和页面销毁设置取消标记，关闭通道或取消流读取后远端连接结束。持续读取和历史读取共用 `request_id` 隔离，迟到行不会写入新的日志页面。
- 测试：`ramag-app` 容器服务专项 9 项通过，覆盖敏感行隐藏；`ramag-infra-container-docker` 普通测试 14 项通过、2 项既有本机 Docker 测试和 1 项持续日志 Docker 测试按设计标记忽略；`ramag-tool-container` 10 项通过，覆盖 `360x640`、`1024x768`、`1440x900` 控件边界和窗口滚动上限。
- 本机 Docker：`cargo test --offline -p ramag-infra-container-docker follows_dedicated_container_logs_and_cleans_resource --lib -- --ignored --nocapture --test-threads=1` 通过；专用 Alpine 容器先输出 `initial-line`，延迟后输出 `follow-line`，持续连接收到两条日志，测试后没有 `ramag-container-log-follow-test` 残留。
- 质量检查：代码提交前还需通过 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check`。
- 证据边界：本切片证明 Docker 单容器的实时 tail、分片组装、回压、停止和窗口边界；它不提供 VictoriaLogs 级别的集中索引、LogSQL 查询、跨容器聚合或自动重连，也没有宣称真实 Windows 原生窗口验收。

### B-CONTAINER-001-E：持续日志暂停展示（设计确认，2026-09-28）

- 问题证据：持续读取开始后，当前页面只能继续刷新或停止连接；用户查看较早日志、复制内容或等待某个事件时，无法暂时冻结可见行。直接停止消费接收通道又会让 Docker 流触发回压，暂停操作和停止连接混在一起。
- 设计：在日志页面增加“暂停展示”和“恢复展示”。暂停只冻结当前可见日志行，不设置 Docker 取消标记；接收任务继续从有界通道取行，并把待显示行放入独立的有界待显示窗口。恢复时按接收顺序把待显示行合并到当前日志窗口。
- 窗口限制：待显示窗口复用 `MAX_CONTAINER_LOG_LINES` 和 `MAX_CONTAINER_LOG_BYTES`；超出时移除最早待显示行，并在日志摘要中累计显示被移除的数量。当前可见窗口仍沿用持续读取的 5,000 行和 2 MiB 限制。停止、切换容器、切换页面或页面销毁时清空待显示窗口并取消 Docker 流。
- 验收条件：应用层不改变持续读取接口和敏感行隐藏；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认暂停/恢复/停止按钮在日志工具栏内；状态测试确认暂停期间可见行不变、待显示行有界，恢复后按顺序合并；适配器回压和本机 Docker 持续回读回归通过；目标测试、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现查询过滤、自动滚动开关、复制、导出、自动重连、VictoriaLogs/LogSQL、跨容器聚合、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再实现页面暂停状态、待显示窗口和 headless 测试；本机 Docker 不新增写操作，代码验证通过后独立提交，下一项再处理复制或导出中的一个明确边界。

### B-CONTAINER-001-E：持续日志暂停展示（代码与 headless 验证完成，2026-09-28）

- 实现：持续读取工具栏新增“暂停展示”“恢复展示”和“停止持续读取”控制。暂停只冻结当前可见窗口，后台接收任务继续消费有界通道；待显示行使用独立的 `VecDeque`，按 5,000 行和 2 MiB 限制滚动移除最早内容，恢复时按接收顺序合并到可见窗口。
- 测试：`ramag-tool-container` 13 项通过，覆盖 `360x640`、`1024x768`、`1440x900` 控件边界、暂停后的可见行保持、恢复后的顺序合并以及待显示窗口上限；`ramag-app` 283 项通过；`ramag-infra-container-docker` 普通测试 14 项通过、3 项本机 Docker 测试按设计保持忽略。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：本切片证明暂停展示不会停止 Docker 流读取，待显示内容仍有界且能按顺序恢复；没有新增本机 Docker 写操作，也不覆盖复制、导出、自动重连、VictoriaLogs/LogSQL、跨容器聚合和真实 Windows 原生窗口验收。

### B-CONTAINER-001-F：复制当前容器日志窗口（设计确认，2026-09-28）

- 问题证据：当前日志页面已能显示历史窗口和持续读取窗口，但用户无法把当前已保留内容带到工单或终端；持续读取时复制不应重新请求 Docker，也不能把暂停期间已移除或尚未恢复的原始数据偷偷加入剪贴板。
- 设计：在日志工具栏增加“复制日志”按钮，复制当前可见 `DockerContainerLogs.lines` 的文本。每行按 `stream: message` 输出并以换行分隔；历史日志和持续日志都经过应用层敏感信息隐藏后才进入可见窗口，因此复制只读取已经展示的数据。按钮不读取 Docker、不改变持续连接、暂停状态或请求代次。
- 大小和状态：复制文本使用与日志窗口相同的有界内容；构造文本时按 UTF-8 字符边界限制在 `MAX_CONTAINER_LOG_BYTES` 内，超出的尾部不进入剪贴板，并在按钮提示中说明复制的是“当前已保留内容”。空日志窗口不显示可用复制按钮。
- 验收条件：纯函数测试确认 stdout/stderr 标签、换行和 UTF-8 边界；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认复制按钮位于日志工具栏内，暂停时只复制当前可见行而不复制待显示窗口；复制操作使用现有 `ramag_ui::copy_text_with_notification` 并显示统一成功通知；目标测试、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不复制待显示窗口、Docker 原始流、日志查询条件、容器详情或敏感原文；不实现文件导出、查询过滤、自动重连、VictoriaLogs/LogSQL、跨容器聚合、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再实现复制文本构造、日志工具栏入口和 headless 测试；不新增本机 Docker 写操作，代码验证通过后独立提交，下一项再处理文件导出或容器资源状态中的一个明确边界。

### B-CONTAINER-001-F：复制当前容器日志窗口（代码与 headless 验证完成，2026-09-28）

- 实现：日志工具栏新增“复制日志”按钮；复制文本按 `stdout: message`/`stderr: message` 格式生成，只读取当前可见 `DockerContainerLogs.lines`，不读取暂停中的待显示窗口。文本构造按 `MAX_CONTAINER_LOG_BYTES` 和 UTF-8 字符边界限制，使用现有 `ramag_ui::copy_text_with_notification` 写入系统剪贴板并显示统一成功通知。
- 测试：`ramag-tool-container` 15 项通过，覆盖三种窗口尺寸的复制按钮边界、UTF-8 截断、stdout/stderr 标签以及实际点击后的剪贴板回读；暂停状态测试确认待显示行不会被复制。`ramag-app` 283 项通过；`ramag-infra-container-docker` 普通测试 14 项通过、3 项本机 Docker 测试按设计保持忽略。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：本切片证明复制的是经过敏感信息处理且已在当前窗口保留的内容；不提供原始 Docker 流复制、待显示队列复制、文件导出、查询过滤、自动重连、VictoriaLogs/LogSQL、跨容器聚合和真实 Windows 原生窗口验收。

### B-CONTAINER-001-G：实时 tail 不重复历史日志（设计确认，2026-09-28）

- 问题证据：日志页面先读取最近 200 行，再点击“持续读取”时仍使用默认 `tail=200`。Docker `follow=true` 会先返回这 200 行，页面再把它们追加到已有窗口，导致历史日志重复，和 VictoriaLogs 这类“已有内容加后续新行”的 tail 体验不一致。
- 设计：历史读取继续使用 `ContainerLogQuery::validate`，要求 `tail` 在 1 到 `MAX_CONTAINER_LOG_TAIL` 之间；持续读取增加单独的 `validate_for_follow`，只允许 `tail=0` 表示不回放已有日志，或使用合法的正数查询。容器日志页面点击持续读取时传入 `tail=0`，Docker 流只向有界 sink 发送连接建立后的新日志。敏感信息隐藏、分片组装、回压、暂停展示和窗口上限保持不变。
- 生命周期：持续读取仍由 Docker `follow=true` 长连接提供；停止、切换容器、切换页面和页面销毁继续设置取消标记。这个切片不把 `tail=0` 扩展为历史读取默认值，也不改变用户主动刷新历史日志的语义。
- 验收条件：领域测试确认普通历史查询拒绝 `tail=0`、持续查询接受 `tail=0` 并仍校验时间范围；应用服务测试确认 `tail=0` 能传到驱动且敏感行仍被隐藏；本机 Docker 持续读取测试确认预先输出的历史行不进入 follow 结果、连接后的新行可以实时收到；容器工作区回归、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现日志时间游标、断线自动重连、跨容器聚合、VictoriaLogs/LogSQL 查询、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再实现 follow 查询校验、页面查询和回归测试；本机 Docker 证明历史行不重复后独立提交，下一项再处理文件导出或容器资源状态中的一个明确边界。

### B-CONTAINER-001-G：实时 tail 不重复历史日志（代码与 Docker 验证完成，2026-09-28）

- 实现：`ContainerLogQuery` 增加 `follow_new_lines()` 和 `validate_for_follow()`；普通历史读取仍拒绝 `tail=0`，持续读取专门使用 `tail=0`。容器页面点击“持续读取”时不再重复请求最近 200 行，Docker `follow=true` 只把连接建立后的新日志交给有界 sink。
- 测试：领域测试确认历史查询与持续查询对 `tail=0` 的边界不同；应用服务测试确认 `tail=0` 传给驱动且敏感行仍被隐藏；Docker 适配器的分片、回压、关闭和取消测试通过；容器工作区 15 项测试通过。
- 本机 Docker：`cargo test --offline -p ramag-infra-container-docker follows_dedicated_container_logs_and_cleans_resource --lib -- --ignored --nocapture --test-threads=1` 通过。专用容器先写入 `initial-line`，再延迟写入 `follow-line`；follow 结果严格只收到 `follow-line`，测试后专用容器已清理。
- 质量检查：目标测试和 `cargo fmt --all -- --check`、`git diff --check` 通过；提交前继续执行 workspace Clippy 和源码尺寸检查。
- 证据边界：本切片证明单容器 Docker follow 的“历史窗口接后续新行”语义；不提供日志时间游标、断线自动重连、跨容器聚合、VictoriaLogs/LogSQL 查询、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。

### B-CONTAINER-001-H：导出当前容器日志窗口（设计确认，2026-09-28）

- 问题证据：当前日志窗口已经支持历史读取、实时 tail、暂停展示和复制，但复制结果仍需要手动转存；用户需要把当前内容作为 `.log` 或 `.txt` 文件交给工单、审查或离线分析。
- 设计：在日志工具栏增加“导出日志”按钮。点击后打开系统保存对话框，默认文件名使用容器标识和 `.log` 扩展名；用户选择路径后，应用把当前可见 `DockerContainerLogs.lines` 按 `stream: message` 格式写入 UTF-8 文本文件。导出复用 `container_logs_copy_text`，因此继续使用已经脱敏、按 `MAX_CONTAINER_LOG_BYTES` 限制的内容。
- 文件写入：复用 `ramag_app::usecases::export::write_atomic`，在目标目录先完整写入临时文件、同步并替换目标文件；取消保存对话框不产生文件，写入失败清理临时文件并保留原目标文件。保存文件名和路径由用户明确选择，不自动写入工作区或临时目录。
- 实时读取边界：导出只读取当前可见窗口，不读取 Docker、不读取暂停中的待显示队列、不改变 follow 连接、暂停状态、请求代次或日志窗口内容。导出期间按钮进入忙碌状态，完成或失败通过统一通知反馈。
- 验收条件：纯函数测试确认导出文本和复制文本完全一致；文件写入测试确认 UTF-8 内容、原子替换和失败清理；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认导出按钮位于日志工具栏内，暂停时只导出当前可见行；目标测试、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不导出待显示队列、Docker 原始流、历史查询条件、容器详情或敏感原文；不实现滚动归档、自动命名批量导出、VictoriaLogs/LogSQL、跨容器聚合、自动重连、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再实现系统保存对话框、原子写入调用、通知和 headless/文件测试；验证通过后独立提交，下一项再处理容器资源状态中的一个明确边界。

### B-CONTAINER-001-H：导出当前容器日志窗口（代码与 headless 验证完成，2026-09-28）

- 实现：容器日志工具栏新增“导出日志”按钮，使用 `rfd::AsyncFileDialog` 让用户选择 `.log` 或 `.txt` 文件；导出调用 `ramag_app::usecases::export::write_atomic`，成功和失败均通过统一通知反馈，保存对话框取消不会写文件。
- 内容边界：导出复用 `container_logs_copy_text`，只读取当前可见 `DockerContainerLogs.lines`，沿用敏感信息隐藏结果、stdout/stderr 标签、UTF-8 边界和 `MAX_CONTAINER_LOG_BYTES` 限制；不读取暂停中的待显示队列，也不改变实时 tail 或暂停状态。
- 测试：`ramag-tool-container` 17 项通过，新增文件内容和原子临时文件清理测试；`360x640`、`1024x768`、`1440x900` 下导出按钮与日志控制区边界测试通过；`ramag-domain` 234 项、`ramag-app` 283 项、`ramag-infra-container-docker` 14 项通过，3 项本机 Docker 测试按设计保持忽略。
- 质量检查：`cargo fmt --all -- --check`、源码尺寸检查和 `git diff --check` 通过；提交前继续执行 workspace Clippy。
- 证据边界：本切片证明当前可见日志可以安全写入用户选择的本地文件；不提供真实系统保存对话框点击、滚动归档、自动命名批量导出、VictoriaLogs/LogSQL、跨容器聚合、自动重连、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。

### B-CONTAINER-001-I：容器详情状态与健康信息刷新（设计确认，2026-09-28）

- 问题证据：Docker 适配器已经从 `inspect` 响应解析容器的 `State`、`Status`、`Health` 和创建时间，但详情面板只显示路径、环境变量键、挂载数量和网络数量。容器停止、重启或健康状态变化后，用户必须离开详情再重新加载列表，无法在当前上下文确认结果。
- 设计：在容器详情面板增加状态、状态说明、健康检查和创建时间；保留 Docker 返回的可选字段，缺失时显示“未知”，不把缺失数据当成健康或运行成功。在详情操作区增加“刷新状态”按钮，复用当前容器 `get_container` 查询和 `request_id` 隔离，只替换当前详情，不改变列表筛选、日志 follow 或任何容器生命周期状态。
- 只读边界：本切片只调用 Docker `inspect`，不新增启动、停止、重启、删除、exec、资源限制或写配置接口。刷新失败保留原详情并显示错误，不把失败回包写成新状态。
- 验收条件：Docker 适配器单元测试确认 `State`、`Status`、`Health` 解析；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认状态详情和“刷新状态”按钮位于内容区内；应用和容器工作区回归、只读 Docker 回放、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现容器启动/停止/重启/删除、Docker stats 实时 CPU/内存曲线、Kubernetes Pod 状态、自动刷新定时器、VictoriaLogs/LogSQL、跨容器聚合或真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再补充详情状态文本、刷新入口和边界测试；只读 Docker 回放确认详情字段后独立提交，下一项再处理容器生命周期或资源指标中的一个明确边界。

### B-CONTAINER-001-I：容器详情状态与健康信息刷新（代码与 Docker 验证完成，2026-09-28）

- 实现：容器详情面板新增状态、状态说明、健康检查和 RFC3339 创建时间；`running`、`paused`、`exited`、`healthy` 等 Docker 值转换为直接可读的中文，缺失健康检查明确显示“未配置健康检查”。详情操作区新增“刷新状态”，复用当前容器 `get_container` 和 `request_id` 隔离，不增加容器写操作。
- 测试：`ramag-tool-container` 19 项通过，覆盖状态/健康文本以及 `360x640`、`1024x768`、`1440x900` 详情面板和刷新按钮边界；`ramag-app` 283 项通过；`ramag-infra-container-docker` 普通测试 14 项通过、3 项本机 Docker 测试按设计保持忽略。
- 本机 Docker：`cargo test --offline -p ramag-infra-container-docker reads_local_engine_without_write_operations --lib -- --ignored --nocapture --test-threads=1` 通过，真实 Engine `29.7.2` 的连接、概览、列表、分页和容器详情只读回放成功，没有创建、修改或删除本机资源。
- 质量检查：目标测试和 `cargo fmt --all -- --check`、源码尺寸检查、`git diff --check` 通过；提交前继续执行 workspace Clippy。
- 证据边界：本切片证明容器详情可以显示并重新读取 Docker inspect 状态；不提供 Docker stats 实时 CPU/内存曲线、自动刷新定时器、容器生命周期写操作、Kubernetes Pod 状态、VictoriaLogs/LogSQL、跨容器聚合或真实 Windows 原生窗口验收。

### B-CONTAINER-001-J：实时日志跟随最新行（代码与测试完成，2026-09-28）

- 问题证据：持续读取会把新行追加到有界日志窗口，但日志输出区没有独立滚动句柄。用户查看历史窗口后启动 follow，或窗口内容超过可视高度后接收新行时，视口可能停在旧位置，无法像 VictoriaLogs tail 一样持续看到最新输出。
- 设计：为日志输出区增加独立 `ScrollHandle`，历史日志读取完成后滚到底部；持续读取且未暂停展示时，每次追加新行都请求滚到底部，保持最新日志可见。暂停展示时不滚动，恢复时按待显示行顺序合并并回到底部；停止 follow 只停止远端读取，不清空当前窗口或改变当前滚动位置。
- 状态边界：暂停展示是用户明确冻结视口的状态；滚动操作不修改 Docker 流、待显示队列、日志窗口上限或请求代次。滚动句柄只服务当前日志输出区，离开容器或切换页面时随视图状态一起重置。
- 验收条件：headless 测试确认 `360x640`、`1024x768` 和 `1440x900` 下日志输出区及工具栏不越界；状态测试确认未暂停时追加新行请求跟随底部，暂停和恢复仍保持既有待显示顺序；容器工作区、Docker 流回归、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现自动重连、日志搜索/高亮、跨容器聚合、VictoriaLogs/LogSQL、滚动历史归档、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实现：日志输出区改为绑定独立 `ScrollHandle`，历史日志读取完成后滚到底部；持续读取未暂停时，每次新增日志都会请求跟随底部；暂停期间不改变视口，恢复时按原顺序合并待显示行并回到底部；切换容器或离开资源状态时重置滚动句柄。
- 测试：新增 `scroll_tests.rs`，在 `360x640` 窄窗口中验证 49 行日志产生滚动范围，并在追加 `latest` 后确认当前偏移到达底部；容器工具 20 项测试通过；`cargo fmt --all`、源码尺寸检查、`git diff --check` 和 workspace Clippy 通过。
- 提交：代码提交为 `d5eb19aa feat(container): follow live log tail`，已推送到 `origin/main`。
- 证据边界：本切片证明 GPUI 日志输出区会在当前有界窗口内跟随新增行；不提供自动重连、日志搜索/高亮、跨容器聚合、VictoriaLogs/LogSQL、滚动历史归档、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。

### B-CONTAINER-001-K：当前日志窗口本地筛选（代码与测试完成，2026-09-28）

- 问题证据：当前日志页面可以持续读取、暂停、复制和导出有界窗口，但日志行数达到上限后只能依靠滚动查找目标内容；没有针对当前日志窗口的文本筛选入口。
- 设计：在日志工具栏增加有界单行输入框，按大小写不敏感的包含关系筛选当前已保留日志行；搜索内容匹配日志正文或 `stdout`/`stderr` 标签时保留该行，输入为空时恢复全部日志。结果摘要显示“匹配行数/总行数”，方便确认筛选是否生效。
- 状态边界：筛选只改变渲染的行集合，不修改 `DockerContainerLogs`、待显示队列、持续读取连接、暂停状态、日志窗口上限或复制/导出的数据来源；输入变化不发起 Docker 请求，也不改变请求代次。筛选条件变化时重置当前日志输出区的滚动句柄，持续读取新增匹配行时仍按实时跟随规则定位到底部。
- 验收条件：状态测试覆盖空条件、大小写不敏感正文匹配、流标签匹配和无匹配结果；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认筛选工具栏与日志输出区不越界；容器工作区、Docker 流回归、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现 VictoriaLogs/LogSQL、正则表达式、时间范围、跨容器聚合、服务端索引、日志高亮、自动重连、滚动历史归档、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实现：日志工具栏增加带清空按钮的单行筛选框；筛选值限制为 `MAX_CONTAINER_QUERY_BYTES`，按大小写不敏感的包含关系匹配日志正文和流标签；日志摘要显示匹配行数，空结果显示明确提示；筛选期间持续读取、暂停队列、复制和导出仍使用未筛选的当前日志窗口。
- 测试：新增 `filter_tests.rs`，覆盖正文、流标签、空条件和无匹配条件；在 `360x640`、`1024x768`、`1440x900` 确认筛选工具栏、输入框、输出区和匹配行边界；容器工具 22 项测试通过；`cargo fmt --all`、源码尺寸检查、`git diff --check` 和 workspace Clippy 通过。
- 提交：代码提交为 `78231b4e feat(container): filter retained log window`，已推送到 `origin/main`。
- 证据边界：本切片证明当前已保留日志窗口可以本地筛选；不提供 VictoriaLogs/LogSQL、正则表达式、时间范围、跨容器聚合、服务端索引、日志高亮、自动重连、滚动历史归档、Kubernetes Pod 日志、Docker exec、容器生命周期写操作或真实 Windows 原生窗口验收。

### B-CONTAINER-001-L：Docker Engine 容量摘要（代码与测试完成，2026-09-28）

- 问题证据：Docker 概览请求已经从 Engine `info` 响应读取 `NCPU` 和 `MemTotal`，领域对象也保存为 `DockerOverview.cpu_count` 与 `DockerOverview.memory_bytes`，但概览页面只显示容器、镜像、网络和数据卷数量，用户看不到当前 Engine 的基础容量信息。
- 设计：在现有概览卡片组增加“CPU 核数”和“内存”两项，直接显示已读取的可选值；Docker 未返回字段时显示“未知”，内存使用现有 GiB/MiB/B 规则格式化，不把缺失值当成零。
- 只读边界：只复用已有 `overview` 请求和 `DockerOverview` 字段，不新增 Docker API、定时刷新、容器写操作或跨主机数据；卡片只描述 Engine 容量，不把它表示为容器当前使用量。
- 验收条件：文本测试覆盖缺失值、KiB/MiB/GiB 格式和已知 CPU 核数；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认 7 张概览卡片和 Engine 信息位于内容区内；容器工作区、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现 Docker `stats`、容器 CPU/内存使用量、网络吞吐、磁盘 IO、历史曲线、自动刷新、阈值告警、Kubernetes 资源指标或真实 Windows 原生窗口验收。
- 实现：概览卡片组新增“CPU 核数”和“内存”，分别读取 `DockerOverview.cpu_count` 与 `DockerOverview.memory_bytes`；缺失值显示“未知”，内存按 B/KiB/MiB/GiB 格式化，并为 7 张卡片增加稳定调试选择器。
- 测试：新增 `overview_tests.rs`，覆盖缺失值和容量格式化，以及 `360x640`、`1024x768`、`1440x900` 下 7 张卡片位于概览面板内；容器工具 24 项测试通过；`cargo fmt --all`、源码尺寸检查、`git diff --check` 和 workspace Clippy 通过。
- 提交：代码提交为 `b52940da feat(container): show engine capacity summary`，已推送到 `origin/main`。
- 证据边界：本切片证明概览页可以显示 Docker Engine 的基础 CPU/内存容量；不提供 Docker `stats`、容器 CPU/内存使用量、网络吞吐、磁盘 IO、历史曲线、自动刷新、阈值告警、Kubernetes 资源指标或真实 Windows 原生窗口验收。

### B-CONTAINER-001-M：单次容器资源指标快照（代码与 Docker 验证完成，2026-09-28）

- 问题证据：容器详情当前显示状态、健康检查、路径、环境变量、挂载和网络数量，但用户无法在当前详情中查看容器的 CPU、内存和网络收发情况；Docker 适配器尚未向应用层暴露 `stats` 快照。
- 设计：在容器详情操作区增加“刷新指标”，调用 Docker `stats` 的单次读取模式；详情面板展示 CPU 使用率、内存使用/限制、内存使用率、网络接收/发送字节和采样时间。字段缺失时显示“未知”，保留上一次成功指标直到新请求成功，刷新失败只显示错误。
- 只读边界：只调用单次 `stats` 查询，不启动持续指标流，不修改容器，不创建定时器；CPU 使用率按 Docker 的前后 CPU 时间和系统时间计算，若 Docker 没有足够样本则显示“未知”。指标属于当前详情容器，不跨容器聚合。
- 验收条件：领域/应用测试覆盖 CPU 计算、内存和网络字段、缺失字段以及刷新失败保留旧指标；Docker 适配器测试覆盖 stats 响应转换；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认指标区域和刷新按钮位于详情面板内；工作区、只读 Docker 回放、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现持续指标流、历史曲线、定时刷新、阈值告警、磁盘 IO/PID 细分、跨容器聚合、VictoriaLogs/LogSQL、Kubernetes 指标、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实现：新增 `DockerContainerStats` 领域模型和 `ContainerDriver::container_stats` 只读接口；Docker 适配器使用 `stats(stream=false, one-shot=true)`，计算 CPU 使用率、内存使用/限制/比例并汇总网络收发字节；容器详情增加“刷新指标”和资源指标面板，失败时保留上一次成功快照。
- 测试：领域 234 项通过；Docker 适配器 16 项普通测试通过、3 项本机 Docker 测试按设计保持忽略；应用 284 项通过；容器工具 27 项通过，覆盖格式化、刷新失败保留旧指标以及 `360x640`、`1024x768`、`1440x900` 边界；`cargo fmt --all`、源码尺寸检查、`git diff --check` 和 workspace Clippy 通过。
- 本机 Docker：`cargo test --offline -p ramag-infra-container-docker reads_local_engine_without_write_operations --lib -- --ignored --nocapture --test-threads=1` 通过，真实 Docker Engine 的只读资源回放增加了运行中容器单次指标读取，没有创建、修改或删除资源。
- 提交：代码提交为 `0e2a4369 feat(container): add single stats snapshot`，已推送到 `origin/main`。
- 证据边界：本切片证明当前容器详情可以读取并展示一次 Docker stats 快照；不提供持续指标流、历史曲线、定时刷新、阈值告警、磁盘 IO/PID 细分、跨容器聚合、VictoriaLogs/LogSQL、Kubernetes 指标、容器生命周期写操作或真实 Windows 原生窗口验收。

### B-CONTAINER-001-N：容器指标快照历史窗口（代码与测试完成，2026-09-28）

- 问题证据：当前详情只保留最后一次成功的 Docker stats 快照，用户连续点击“刷新指标”时无法比较前后采样结果，难以判断 CPU 或内存是否正在变化。
- 设计：在当前容器详情内保留最近 20 次成功手动刷新结果，按采样到达顺序显示最新记录和历史记录；每条记录显示采样时间、CPU 使用率、内存使用率、内存用量和网络收发字节。最新快照继续作为详情摘要，历史窗口只读展示。
- 状态边界：历史数据只存在于当前 UI 会话和当前容器详情，不写入本地存储、不上传服务端、不跨容器合并；切换容器或清空资源状态时清空历史；刷新失败不追加记录，也不覆盖上一条成功记录；历史达到上限时移除最早记录。
- 验收条件：状态测试确认成功刷新按顺序追加、最多保留 20 条、失败刷新保留原历史；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认历史面板、指标摘要和详情操作区不越界；工作区、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现定时刷新、持续 stats 流、持久化、服务端指标存储、折线图、阈值告警、跨容器聚合、VictoriaLogs/LogSQL、Kubernetes 指标、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实现：容器详情保留最近 20 条成功手动刷新结果，最新记录仍显示在资源指标摘要，历史面板按最新到最早显示采样时间、CPU、内存和网络收发字节；切换容器或清空资源状态时清空，失败刷新只设置错误。
- 测试：容器工具 28 项测试通过，覆盖成功刷新顺序、20 条上限、失败刷新保留旧数据，以及 `360x640`、`1024x768`、`1440x900` 下历史面板、指标摘要和操作区边界；`cargo fmt --all`、源码尺寸检查、`git diff --check` 和 workspace Clippy 通过。
- 提交：代码提交为 `46854448 feat(container): retain stats history`，已推送到 `origin/main`。
- 证据边界：本切片证明当前 UI 会话可以查看当前容器最近的有限指标快照；不提供定时刷新、持续 stats 流、持久化、服务端指标存储、折线图、阈值告警、跨容器聚合、VictoriaLogs/LogSQL、Kubernetes 指标、容器生命周期写操作或真实 Windows 原生窗口验收。

### B-CONTAINER-001-O：容器指标趋势条（代码与测试完成，2026-09-28）

- 问题证据：指标历史目前以文本列表显示，用户可以看到采样值但需要逐行比较，CPU 和内存变化没有直接的视觉趋势。
- 设计：在当前容器指标历史下增加 CPU 和内存两行有界趋势条，每个柱对应一条成功快照，按最早到最新排列；百分比按 0% 到 100% 映射，超过 100% 的 CPU 按 100% 显示，缺失值显示为固定高度的未知柱并保留文本历史。
- 状态边界：趋势条只读取当前内存中的 `container_stats_history`，不改变历史数据，不触发刷新，不改变最新摘要；切换容器、离开容器页面或清空资源状态时同时清理摘要、历史和趋势；趋势只表达 CPU/内存比例，不把网络字节误画成百分比。
- 验收条件：状态测试覆盖 0%、50%、100%、超过 100% 和缺失值的柱高映射；headless 测试在 `360x640`、`1024x768` 和 `1440x900` 确认趋势面板、CPU/内存趋势行及详情操作区不越界；工作区、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不实现图表库、SVG、鼠标悬停读数、缩放、时间范围选择、定时刷新、持续 stats 流、持久化、服务端指标存储、阈值告警、跨容器聚合、VictoriaLogs/LogSQL、Kubernetes 指标、容器生命周期写操作或真实 Windows 原生窗口验收。
- 实现：在指标历史面板下增加 CPU 和内存两行有界趋势条，柱子按最早到最新排列；百分比映射到 4px 到 48px，超过 100% 封顶，缺失值使用固定的未知柱；切换容器、切换资源页面和清空资源状态会清理摘要、历史和趋势状态。
- 测试：容器工具 29 项测试通过，覆盖趋势柱高度边界以及 `360x640`、`1024x768`、`1440x900` 下趋势面板、CPU/内存行、历史面板和详情操作区边界；`cargo fmt --all`、源码尺寸检查、`git diff --check` 和 workspace Clippy 通过。
- 提交：代码提交为 `6343e5f4 feat(container): show stats trend bars`，已推送到 `origin/main`。
- 证据边界：本切片证明当前 UI 会话可以把已有 CPU/内存快照显示为轻量趋势条；不提供图表库、SVG、鼠标悬停读数、缩放、时间范围选择、定时刷新、持续 stats 流、持久化、服务端指标存储、阈值告警、跨容器聚合、VictoriaLogs/LogSQL、Kubernetes 指标、容器生命周期写操作或真实 Windows 原生窗口验收。

### B-GIT-001-A：批量工作区操作使用路径快照（设计确认，2026-09-28）

- 问题证据：Git 变更树的“全暂存”和“全取消”按钮当前保存 `FileStatus` 数组下标，点击时再从最新状态数组取路径。文件监听或异步刷新在按钮渲染后增删、重命名或重排文件时，旧下标可能指向另一条路径，批量操作就会修改错误文件。
- 设计：渲染变更组按钮时直接保存当前筛选结果中的路径字符串；点击按钮把这组路径交给已有 `run_file_op`，不再按下标回查最新状态。操作完成后继续由现有状态刷新和文件标签同步逻辑更新界面；空路径集合不发起 Git 操作。
- 验收条件：单元测试确认状态数组重排后仍使用按钮渲染时的路径集合，并保留重复路径过滤和空集合保护；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不新增 Git 命令，不改变暂存、取消暂存和丢弃的驱动语义，不增加批量丢弃入口，不调整远程同步或提交模型，不把本项扩展为真实窗口验收。
- 实施顺序：先提交本设计确认，再修改批量按钮的路径传递和回归测试；定向测试通过后独立提交并推送，随后补充 `B-GIT-001` 的真实临时仓库回放记录。

### B-GIT-001-A：批量工作区操作使用路径快照（代码与本地 Git 回放完成，2026-09-28）

- 实现：变更组的“全暂存”和“全取消”按钮在渲染时保存去重后的相对路径集合；点击时直接把路径交给已有 `run_file_op`，不再使用状态数组下标回查，避免文件监听刷新后把操作落到另一条路径。暂存、取消暂存、丢弃、状态刷新和文件标签同步的原有语义保持不变。
- 定向回归：新增测试覆盖状态数组重排、重复下标和无效下标，确认批量操作仍使用原先渲染时的 `src/first.rs`、`src/second.rs` 路径集合；`ramag-tool-vcs` 全量 133 项通过，5 项性能观察测试按设计忽略。
- 本地 Git 回放：使用测试创建的临时 Git 仓库和 bare remote，运行 `cargo test --locked -p ramag-infra-git --test integration -- --test-threads=1`，40 项通过；运行 `cargo test --locked -p ramag-infra-git --lib -- --test-threads=1`，73 项通过，1 项性能观察测试忽略。回放覆盖仓库打开/关闭、工作区状态、暂存/取消暂存、提交、分支、差异、stash、tag、rebase、冲突继续/中止、bare remote 推送和跟踪分支。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过；提交 `a399426f docs: define git batch path snapshot acceptance`、`2e4cec85 fix(vcs): snapshot bulk operation paths` 已推送 `origin/main`。
- 证据边界：本切片证明批量工作区操作在本地状态刷新期间使用稳定路径，并证明 Git 驱动的临时仓库回放；不提供真实 Windows 原生窗口鼠标/键盘证据，不验证 114 服务器，不改变远程仓库数据，也不把 Computer Use 缺口写成已完成。

### B-GIT-001-B：Pull 远程回放与非交互合并（设计确认，2026-09-29）

- 问题证据：Git 驱动已有 `push`、`fetch` 和界面 Pull 调用，但集成测试没有覆盖 Pull 的快进和分叉历史；Pull 的合并路径也没有明确传入 `--no-edit`，桌面进程只能依赖环境中的编辑器设置完成合并提交。
- 设计：普通 Pull 和带进度的 Pull 共用参数构造；快进和 rebase 保持原有行为，普通合并显式加入 `--no-edit`，让 Git 使用默认合并信息并直接结束，不等待编辑器。新增临时 bare remote 回放：先验证远程提交快进到本地，再制造本地与远程各有一个提交的分叉历史，确认 Pull 能完成合并并保留两侧文件与提交。
- 验收条件：集成测试必须覆盖普通 Pull 和带进度 Pull 的成功路径、快进后的工作区状态、分叉合并后的提交历史和文件内容；Git 远程参数单元测试确认合并 Pull 含 `--no-edit`、rebase Pull 不含该参数；`ramag-infra-git` 目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变 Pull 的 rebase 选择、冲突处理、凭据助手、远程地址或分支跟踪规则；不访问 114 服务器，不修改 GitHub/Gitea 远程仓库，不把真实 Windows 原生窗口流程写入本切片。
- 实施顺序：先提交本设计确认，再修改 Pull 参数构造和临时远程回放测试；验证通过后独立提交并推送，随后补充本切片验收记录。

### B-GIT-001-B：Pull 远程回放与非交互合并（代码与本地 Git 回放完成，2026-09-29）

- 实现：普通 Pull 显式传入 `--no-rebase --no-edit`，在分叉历史下选择合并并直接采用 Git 默认合并信息；rebase Pull 保持 `--rebase`；带进度 Pull 同步使用对应参数，继续通过 stderr 读取进度和取消状态。
- 单元回归：新增 Pull 参数测试，确认普通 Pull 含 `--no-rebase --no-edit`，带进度普通 Pull 同时含 `--progress`，rebase Pull 不带普通合并参数。
- 本地 Git 回放：使用临时 bare remote 和临时工作目录生成远程提交，验证普通 Pull 快进后读取远程文件、分叉 Pull 合并后同时保留本地和远程文件及两侧提交、带进度 Pull 快进后读取远程文件；三项测试均通过。
- 质量检查：`cargo test --locked -p ramag-infra-git --all-targets -- --test-threads=1` 通过，结果为驱动单元测试 74 项、集成测试 43 项，性能测试 1 项按环境条件忽略；`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。设计提交 `f2112aea docs: define git pull acceptance`、代码提交 `302b3f12 fix(git): make pull merge noninteractive` 已推送 `origin/main`。
- 证据边界：本切片证明 Git 驱动可在本地临时远程中完成快进、分叉合并和带进度 Pull；不提供真实 Windows 原生窗口鼠标/键盘证据，不访问 114 服务器，不修改 GitHub/Gitea 远程仓库，也不把 Computer Use 缺口写成已完成。

### B-GIT-001-C：普通工作区显示远程操作入口（设计确认，2026-09-29）

- 问题证据：Push/Pull/Fetch 和取消按钮目前只挂在底部 History 工具栏；用户停留在 Project 或 Changes 视图且未打开 History 面板时，看不到远程同步入口，只能依赖快捷键。
- 设计：把现有远程快速操作和远程菜单移动到文件工作区工具栏；快速操作继续只在存在 ahead/behind 时显示，远程菜单继续提供 Fetch、Pull、Push 和强推，进行中的远程操作继续显示进度和取消按钮。History 工具栏只保留历史搜索、筛选和分页相关控件，避免同一操作出现两个入口。
- 验收条件：headless GPUI 在 180px、280px、600px 文件栏宽度以及 360px、800px、1440px 窗口宽度确认远程快速操作、远程菜单和取消按钮均留在文件工具栏内；关闭 History 面板时远程菜单仍可见，打开 History 面板时不出现重复远程控件；已有 VCS 渲染与远程操作语义不变。
- 不做事项：不改变 Fetch/Pull/Push 的驱动参数、确认对话框、取消机制、错误处理或远程数据；不新增自动同步，不把快捷键作为唯一入口，不把真实 Windows 原生窗口流程写入本切片。
- 实施顺序：先提交本设计确认，再调整工具栏归属和窄窗口回归；目标测试通过后独立提交并推送，随后补充本切片验收记录。

### B-GIT-001-C：普通工作区显示远程操作入口（代码与 headless 验证完成，2026-09-29）

- 实现：把远程快速操作、远程菜单和进行中的取消入口移到文件工作区工具栏；用户不打开 History 面板也能看到 Fetch、Pull、Push、强推和当前同步进度。History 工具栏保留搜索和筛选控件，去掉重复的远程入口。
- 验收结果：VCS headless 布局测试覆盖 History 关闭、History 打开、180px/280px/600px 文件栏和 360px/800px/1440px 窗口；确认远程快速操作和远程菜单位于文件工具栏内，History 工具栏不再重复渲染。`ramag-tool-vcs` 全量 133 项通过，5 项性能观察测试按设计忽略。
- 质量检查：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过；代码提交 `a980e50a fix(vcs): expose remote actions in workspace toolbar` 已推送 `origin/main`。
- 证据边界：本切片证明远程操作入口在 headless 文件工作区中可见且布局受控；不提供真实 Windows 原生窗口鼠标/键盘证据，不改变 Fetch/Pull/Push 语义，不访问 114 服务器，也不把 Computer Use 缺口写成已完成。

### B-GIT-001-D：Stash 操作用提交 ID 重新定位（设计确认，2026-09-29）

- 问题证据：Stash 列表按钮当前保存 `stash@{n}` 下标。用户筛选列表、另一进程新增或删除 stash 后，旧按钮仍可能把应用、弹出或删除操作发送到新的同一位置。
- 设计：`StashOp` 保存 stash 的提交 ID；按钮和删除确认框都捕获该 ID。执行操作前，视图从当前 stash 列表按提交 ID重新定位索引，再调用现有 `GitDriver::stash_apply` 或 `stash_drop`；找不到对应提交时拒绝操作并提示列表已更新，驱动接口和 Git 命令保持不变。
- 验收条件：代码检查确认 Apply、Pop、Drop 和删除确认都不直接捕获旧下标；提交 ID 重新定位后仍调用正确的当前索引；Stash 行按钮 ID 使用稳定提交 ID；`ramag-tool-vcs` 编译、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不修改 `GitDriver` 的索引接口，不改变 stash 列表顺序、搜索、冲突处理或临时 checkout 备份流程，不访问远程仓库，不把真实 Windows 原生窗口流程写入本切片。
- 实施顺序：先提交本设计确认，再调整 `StashOp`、行按钮和执行前索引定位；完成编译与质量检查后独立提交并推送，随后补充验收记录。

### B-GIT-001-D：Stash 操作用提交 ID 重新定位（代码与 headless 回归完成，2026-09-29）

- 实现：Stash 行按钮和删除确认捕获完整 stash 提交 ID；执行 Apply、Pop、Drop 前重新读取当前列表，按提交 ID找到最新索引，再调用原有驱动接口。目标已被外部删除时返回“列表已更新，请刷新后重试”，不会把旧位置当作新目标。
- 验收结果：`ramag-tool-vcs` 全量 133 项通过，5 项性能观察测试按设计忽略；`cargo check --locked -p ramag-tool-vcs`、`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 提交：代码提交 `e3896583 fix(vcs): resolve stash actions by commit` 已推送 `origin/main`。
- 证据边界：本切片证明 UI 操作目标与当前 stash 提交保持一致，并保留 Git 驱动的索引接口；不提供真实 Windows 原生窗口鼠标/键盘证据，不访问远程仓库，也不把 Computer Use 缺口写成已完成。

### B-GIT-001-E：历史侧栏行使用对象快照（设计确认，2026-09-29）

- 问题证据：历史左栏的本地分支、远程分支、Tag 和远程仓库行目前只保存数组下标。异步刷新或外部 Git 操作改变列表顺序后，旧的虚拟列表行仍可能按旧下标读取另一对象，导致显示内容和菜单操作目标不一致。
- 设计：生成历史左栏行时保存对应的分支、Tag 或远程仓库对象副本；渲染行和创建菜单都直接使用这份对象快照，不再按下标回查当前数组。行标识改用分支类型加名称、Tag 名称或远程名称，保证旧行的操作仍指向渲染时的对象。列表刷新后由新的数据快照重新生成行。
- 验收条件：单元测试确认行快照在源列表重排后仍保留原对象名称和远程地址；源码检查确认历史左栏的分支、Tag 和远程仓库渲染不再从 `idx` 回查数组；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变分支、Tag 或远程仓库的 Git 命令和确认流程，不改变历史过滤规则、折叠状态或列表排序，不处理提交历史列表本身的分页目标，不把本项扩展为真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再改造历史左栏行数据和稳定行标识，补充单元回归；定向测试通过后独立提交并推送，随后补充本切片验收记录。

### B-GIT-001-E：历史侧栏行使用对象快照（代码与 headless 回归完成，2026-09-29）

- 实现：历史左栏的本地分支、远程分支、Tag 和远程仓库行现在保存渲染时的对象副本；渲染、历史过滤和操作菜单直接使用副本，不再按数组下标读取当前列表。分支、Tag 和远程仓库的行标识改为使用类型和名称，列表重排不会把旧行复用成另一个对象。
- 定向回归：新增测试在生成行后重排分支列表并修改远程地址，确认已有行仍保留原分支顺序、原远程地址和 Tag 名称；`ramag-tool-vcs` 全量 139 项测试中 134 项通过，5 项性能观察测试按设计忽略。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过；代码提交 `0dfb012e fix(vcs): stabilize history sidebar rows` 已推送 `origin/main`。
- 证据边界：本切片证明历史左栏的异步刷新不会让旧行按下标改指向另一分支、Tag 或远程仓库；不提供真实 Windows 原生窗口鼠标/键盘证据，不访问 114 服务器，不改变 Git 命令语义，也不把 Computer Use 缺口写成已完成。

### B-GIT-001-F：工作区单文件行使用状态快照（设计确认，2026-09-29）

- 问题证据：变更树缓存的单文件行目前只保存 `status.files` 下标，渲染虚拟列表时再从当前状态数组取文件。文件监听或 Git 操作完成后如果状态数组重排，旧行可能把已暂存、未暂存或冲突类型套到另一文件，单文件操作就可能使用错误的路径和操作类型。
- 设计：构建变更树时继续只用下标完成目录树整理，但写入 `ChangeRow` 后立即保存 `FileStatus` 副本；分组表头同时保存当次生成的路径集合。虚拟列表渲染、单文件按钮、历史入口和冲突操作全部使用这些副本，不再从当前状态数组按下标回查。目录折叠和批量操作的 Git 命令保持不变。
- 验收条件：单元测试确认行生成后源状态重排或改变文件信息，已有单文件行仍保留原路径、状态类型和分组；源码检查确认渲染变更行不再按 `file_index` 回查当前状态；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变暂存、取消暂存、丢弃、冲突解决和文件历史的 Git 命令，不改变目录排序、折叠状态、搜索过滤或批量路径去重，不把本项扩展为真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再改造变更树行的文件和路径快照，补充重排回归；定向测试通过后独立提交并推送，随后补充本切片验收记录。

### B-GIT-001-F：工作区单文件行使用状态快照（代码与 headless 回归完成，2026-09-29）

- 实现：变更树表头保存生成时的路径集合，单文件行保存生成时的完整 `FileStatus`；虚拟列表渲染、暂存/取消暂存、文件历史和冲突操作直接使用这些快照，不再按 `file_index` 从当前工作区状态回查。
- 定向回归：新增测试在生成文件行后重排状态数组并修改新位置的路径，确认旧行仍保留 `src/first.rs`、原始修改状态、深度和 `Unstaged` 分组；批量路径去重和状态重排回归继续通过。`ramag-tool-vcs` 全量 140 项测试中 135 项通过，5 项性能观察测试按设计忽略。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过；代码提交 `ad2724e0 fix(vcs): snapshot workspace file rows` 已推送 `origin/main`。
- 证据边界：本切片证明虚拟变更行在异步状态刷新期间仍使用生成时的文件和操作类型；不提供真实 Windows 原生窗口鼠标/键盘证据，不访问 114 服务器，不改变 Git 驱动命令语义，也不把 Computer Use 缺口写成已完成。

### B-GIT-001-G：自动 Stash 清理按提交 ID重新定位（设计确认，2026-09-29）

- 问题证据：脏工作区选择“切换并丢弃”时，Ramag 会先创建带唯一标记的临时 Stash，切换成功后从列表找到该标记并直接把当次下标交给 `stash_drop`。外部进程在两次列表读取之间新增或删除 Stash 时，这个下标可能改指向其他条目。
- 设计：切换成功后先按唯一标记保存临时 Stash 的提交 ID，再重新读取 Stash 列表，按提交 ID 找到当前下标后才调用现有 `stash_drop`。任一列表读取失败、提交 ID 消失或重新定位失败，都不执行删除，并把临时备份保留给用户手动恢复。
- 验收条件：单元测试确认列表重排并插入新条目后仍解析原临时提交，并解析其最新下标；找不到提交或读取失败时走保留备份分支；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变脏工作区切换、临时 Stash 创建、分支检出或失败恢复语义，不修改 Git 驱动的 Stash 索引接口，不访问远程仓库，不把本项扩展为真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再改造临时 Stash 的提交 ID 解析和二次定位，补充列表重排回归；定向测试通过后独立提交并推送，随后补充本切片验收记录。

### B-GIT-001-G：自动 Stash 清理按提交 ID重新定位（代码与 headless 回归完成，2026-09-29）

- 实现：脏工作区切换成功后的临时 Stash 清理先按唯一标记取得提交 ID，再重新读取列表并按提交 ID解析最新下标；列表读取失败、临时提交消失或重新定位失败时不执行 `stash_drop`，保留备份并报告原因。
- 定向回归：新增测试覆盖外部条目插入导致列表重排，以及临时提交消失的情况；前者仍解析到原提交的新下标，后者不会改指向其他条目。`ramag-tool-vcs` 全量 142 项测试中 137 项通过，5 项性能观察测试按设计忽略。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过；代码提交 `a2444817 fix(vcs): relocate temporary stash cleanup` 已推送 `origin/main`。
- 证据边界：本切片证明自动临时 Stash 清理按稳定提交重新定位并在无法确认目标时保留备份；不提供真实 Windows 原生窗口鼠标/键盘证据，不访问 114 服务器，不改变 Git 驱动的 Stash 索引接口，也不把 Computer Use 缺口写成已完成。

### B-GIT-001-H：文件标签关闭按稳定目标定位（代码与 headless 回归完成，2026-09-29）

- 问题证据：文件标签的关闭按钮当前保存渲染时的数组下标并调用 `close_file_tab`。工作区状态刷新会重建 Changes 标签顺序；如果旧界面事件在刷新后到达，下标可能对应另一份文件，关闭动作就会误关标签。
- 设计：文件标签保存路径和完整 `FileTabSource` 作为稳定目标，关闭按钮按当前标签列表重新定位目标后再关闭；目标不存在时不按下标猜测，也不关闭其它标签，只提示标签列表已更新。标签和关闭按钮的调试选择器同步使用目标标识，列表重排不会复用成另一目标的节点。
- 状态边界：键盘关闭当前活动标签继续使用当前活动状态；文件标签的选择、异步 diff 回写、草稿保存和缓存预算保持现有路径/来源匹配规则。稳定目标只覆盖关闭入口，不改变标签顺序、仓库状态刷新或 Git 命令。
- 验收条件：纯函数测试确认标签重排后仍定位原路径/来源、目标消失时返回无结果；headless 渲染检查确认标签和关闭按钮使用稳定目标标识；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变文件标签的选择语义、Git 驱动接口、Changes 状态刷新、Project Files 草稿保存、提交/暂存/丢弃行为，不把本项扩展为真实 Windows 原生窗口验收。
- 实现：新增 `FileTabTarget`，以文件路径和标签来源构成稳定身份；关闭按钮按目标重新查找当前标签，目标消失时只保留提示，不按旧下标关闭其它标签。标签和关闭按钮的调试选择器改用目标哈希，提交标签的 `change_kind` 变化不会改变其身份。
- 验收结果：`ramag-tool-vcs` 全量 146 项测试中 141 项通过、5 项性能观察测试按设计忽略；新增回归覆盖标签重排、目标消失、提交标签显示字段变化和稳定选择器 ID。`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 提交：设计确认提交为 `dfbb7fd6`，代码提交为 `1e18ee42`，均已推送到 `origin/main`。
- 证据边界：本切片证明 headless 标签模型在列表重排和目标消失时不会把关闭动作改指向其它标签；不提供真实 Windows 原生窗口鼠标/键盘证据，不访问 114 服务器，也不改变 Git 驱动命令语义。

### B-GIT-001-I：交互式 Rebase 行按提交 ID重新定位（代码与 headless 回归完成，2026-09-29）

- 问题证据：交互式 Rebase 计划的操作菜单、上移和下移按钮当前保存渲染时的数组下标。用户先移动一行后，旧菜单或迟到的点击事件仍可能把该下标应用到另一条提交，错误修改 Rebase 计划。
- 设计：使用 `RebaseTodo.hash` 作为行的稳定目标；操作回调只保存提交 ID，执行时从当前 `rebase_todos` 重新定位索引，再校验当前位置是否允许 `Squash`/`Fixup` 和上移/下移。目标已经不存在时不修改其它行，只提示计划已更新。行、操作菜单和上下移按钮的调试选择器改用提交 ID派生的稳定标识。
- 状态边界：Rebase 计划仍按当前列表顺序提交给 `interactive_rebase_execute`；提交 ID只用于 UI 目标定位，不改变 Git 协议、RebaseAction 规则、计划顺序或执行确认。第一条提交的合并限制在操作执行时再次检查，避免旧界面状态绕过限制。
- 验收条件：纯函数测试确认列表重排后按提交 ID找到原提交、目标消失时没有回退到其它索引、第一条提交不能被改成 `Squash`/`Fixup`；headless 渲染检查确认行和控制使用稳定标识；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变 Git Rebase 脚本、提交 ID校验、冲突处理、执行确认、分支状态或真实 Windows 原生窗口验收，不把本项扩展为远程仓库操作。
- 实现：Rebase 操作菜单、上移和下移按钮都保存 `RebaseTodo.hash`，执行时按当前列表重新定位；提交目标消失时保留计划不变并提示用户。当前位置的 `Squash`/`Fixup` 限制在执行时重新检查，行和控制的调试选择器改用完整提交 ID。
- 验收结果：`ramag-tool-vcs` 全量 150 项测试中 145 项通过、5 项性能观察测试按设计忽略；新增提交 ID重排/目标消失测试，以及 GPUI headless 重排渲染测试 `vcs_view_renders_rebase_plan_after_todo_reorder`。`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 提交：设计确认提交为 `277ead41`，代码提交为 `1d9861f1`，均已推送到 `origin/main`。
- 证据边界：本切片证明 Rebase 计划的 UI 操作在列表重排后仍指向原提交，并保留第一条提交的合并限制；不提供真实 Windows 原生窗口鼠标/键盘证据，不执行远程仓库操作，也不改变 Git Rebase 驱动语义。

### B-GIT-001-J：工作区变更行使用稳定路径标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：Changes 面板的文件行、历史按钮、暂存/取消暂存/丢弃按钮和冲突处理按钮当前使用变更数组下标生成调试选择器。外部文件刷新、筛选或分组变化后，同一个界面节点可能被复用给另一条路径，造成自动化定位和状态复用不稳定；操作回调虽然保存路径，但节点身份仍不稳定。
- 设计与实现：新增稳定 ID 辅助函数，以相对文件路径、变更组和操作前缀生成有界 ID；文件行、目录行、历史入口、普通文件操作和冲突操作不再使用渲染下标。相同路径同时出现在已暂存和未暂存组时，组类型参与文件行和目录行 ID；同一操作类型的按钮使用路径作为目标，操作回调继续直接保存路径。
- 状态边界：Changes 的排序、过滤、目录折叠、批量路径快照和 Git 操作语义不变；稳定 ID只用于 GPUI 节点身份和调试选择器，不作为 Git 路径输入，也不改变状态刷新或冲突处理。
- 验收结果：稳定 ID 纯函数测试确认相同路径/组生成相同 ID、不同路径或组不会冲突；headless 测试 `vcs_view_renders_workspace_rows_after_status_reorder` 在 360×640、720×640 和 1200×800 下重排文件状态并重新渲染，未发生崩溃。`ramag-tool-vcs` 全量 153 项测试中 148 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变暂存、取消暂存、丢弃、冲突解决、查看历史、批量操作或 Git 驱动接口，不访问远程仓库，不把本项扩展为真实 Windows 原生窗口验收。
- 提交：设计确认提交为 `8818843a`，代码提交为 `a7388c63`，均已推送到 `origin/main`。
- 证据边界：本切片证明状态顺序变化后，工作区节点标识不再依赖渲染下标；headless 渲染覆盖三种窗口尺寸，但不提供真实 Windows 原生窗口的像素边界、鼠标和键盘证据。

### B-GIT-001-K：分支比较文件行使用稳定目标标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：分支比较面板的文件行使用比较文件数组下标生成 GPUI 节点 ID。筛选、异步刷新或比较范围重载后，旧行可能被复用给另一条路径；点击回调虽然保存路径和比较两端，但节点选择状态和自动化定位仍可能随下标变化。
- 设计与实现：以比较起点 revision、终点 revision 和文件路径生成有界稳定 ID；渲染回调继续保存当前 `FileStatus` 的路径与比较范围，不再把行下标作为节点身份。文件标签选择仍按路径和 `FileTabSource::Compare` 匹配，比较 Diff 请求和 Git 驱动接口不变。
- 状态边界：只调整比较文件行的 GPUI 节点身份和调试选择器，不改变比较文件排序、文本筛选、文件标签、Diff 加载、请求代次或 Git 命令；同一路径在不同比较范围内必须产生不同 ID。
- 验收结果：稳定 ID 纯函数测试确认相同比较范围/路径生成相同 ID，不同 revision 或路径不会冲突；headless 测试 `vcs_view_renders_compare_rows_after_file_reorder` 在文件列表重排后于 `360x640`、`1024x768` 和 `1440x900` 重新渲染比较面板，未发生崩溃。`ramag-tool-vcs` 全量 155 项测试中 150 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变分支选择、比较范围计算、Diff 内容、文件标签关闭、远程操作或真实 Windows 原生窗口验收；不把该切片扩展为 SSH 终端/端口转发集成测试。
- 提交：设计确认提交为 `a6c15395`，代码提交为 `c510f705`，均已推送到 `origin/main`。
- 证据边界：本切片证明比较文件行在筛选范围重排和比较范围变化时不再依赖文件数组下标；headless 渲染覆盖三种窗口尺寸，但不提供真实 Windows 原生窗口的像素边界、鼠标和键盘证据。

### B-GIT-001-L：Project Files 行使用路径快照和稳定标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：Project Files 的文件行保存 `project_files` 数组下标，渲染时再按下标读取路径和变更状态；目录行也使用虚拟列表行下标生成 ID。异步文件刷新、搜索结果变化或迟到的旧行渲染可能让节点身份和状态显示依赖数组位置，而不是具体路径。
- 设计与实现：构建 Project Files 行时直接保存目录路径和文件完整路径；渲染、选中状态和打开文件回调只使用行快照，不再按 `path_index` 回查当前路径数组。变更状态缓存改按完整路径保存，目录行和文件行 ID 按目录路径或文件路径生成，路径刷新后由版本化缓存生成新的行集合。
- 状态边界：继续使用排序后的路径数组构建目录树，保留搜索、目录展开/收起、状态优先级和文件标签逻辑；路径快照和节点 ID 只解决异步渲染期间的对象定位，不改变文件读取、编辑保存或 Git 状态刷新。
- 验收结果：单元测试确认行生成后源路径数组重排，已有文件行仍保留原路径和层级；路径 ID 沿用稳定 ID 辅助函数，按目录/文件路径区分节点；headless 测试 `vcs_view_renders_project_rows_after_source_reorder` 在 `360x640`、`1024x768` 和 `1440x900` 重排 Project Files 后重新渲染，未发生崩溃。`ramag-tool-vcs` 全量 157 项测试中 152 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变 Project Files 的排序、搜索、目录展开规则、文件编辑器、远程 Git 操作或真实 Windows 原生窗口验收；不把本切片扩展为文件内容读取协议或发布构建验证。
- 提交：设计确认提交为 `ee613e46`，代码提交为 `a009acd6`，均已推送到 `origin/main`。
- 证据边界：本切片证明 Project Files 的虚拟列表行在源路径数组变化时仍按生成时路径渲染，并且节点标识不再依赖数组下标；headless 渲染覆盖三种窗口尺寸，但不提供真实 Windows 原生窗口的像素边界、鼠标和键盘证据。

### B-GIT-001-M：提交详情文件树使用稳定路径标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：提交详情文件树的目录行和文件行使用扁平虚拟列表下标生成节点 ID。提交文件列表刷新、目录折叠或旧渲染回调继续完成时，同一节点 ID 可能被复用给另一条路径；点击回调虽然已经保存目录路径或文件路径，但自动化定位和节点复用仍依赖行位置。
- 设计与实现：以提交 ID和目录路径生成目录行 ID，以提交 ID和文件路径生成文件行 ID；渲染回调继续使用已捕获的 `FileStatus` 快照和路径，不改变提交详情文件树的排序、折叠、选中和 Diff 加载行为。
- 状态边界：稳定标识只用于 GPUI 节点身份和调试选择器，不作为 Git 命令参数，也不改变提交文件来源、提交详情请求代次、文件选择或右侧 Diff 内容；同一路径在不同提交中必须产生不同 ID。
- 验收结果：稳定 ID 纯函数测试确认相同提交/路径生成相同 ID，不同提交或路径不会冲突；headless 测试 `vcs_view_renders_commit_detail_rows_after_file_reorder` 在提交文件列表重排后于 `360x640`、`1024x768` 和 `1440x900` 重新渲染详情面板，未发生崩溃。`ramag-tool-vcs` 全量 159 项测试中 154 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变提交详情请求、目录树排序、折叠规则、文件 Diff、远程操作或真实 Windows 原生窗口验收；不把本切片扩展为提交内容协议或远程仓库回放。
- 提交：设计确认提交为 `3a757e0d`，代码提交为 `6ef3bad2`，均已推送到 `origin/main`。
- 证据边界：本切片证明提交详情目录/文件行在文件列表重排时不再依赖虚拟列表下标；headless 渲染覆盖三种窗口尺寸，但不提供真实 Windows 原生窗口的像素边界、鼠标和键盘证据。

### B-GIT-001-N：Reflog 行和 Checkout 入口使用稳定记录标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：Reflog 经过客户端筛选后，行和 Checkout 按钮使用过滤结果中的可见下标生成节点 ID。搜索条件变化、异步刷新或列表重排后，旧节点可能被复用给另一条记录；Checkout 回调虽然保存提交 ID，但节点定位仍依赖下标。
- 设计与实现：以 Reflog 记录的提交 ID、selector、操作、主题和时间组成记录键，再为行和 Checkout 按钮生成固定长度的稳定 ID；Checkout 回调继续只使用提交 ID，不改变检出确认和 detached HEAD 处理。
- 状态边界：只调整 GPUI 节点身份和调试选择器，不改变 Reflog 搜索字段、排序、分页、提交目标、历史面板切换或 Git 命令；同一记录在不同过滤结果中的 ID保持不变。
- 验收结果：Reflog 记录键测试确认相同记录生成相同 ID，不同提交或记录字段不会冲突；已有 Reflog 查询字段测试继续通过，headless 测试 `vcs_view_renders_reflog_rows_after_entry_reorder` 在列表重排后于 `360x640`、`1024x768` 和 `1440x900` 重新渲染 Reflog，未发生崩溃。`ramag-tool-vcs` 全量 161 项测试中 156 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变 Reflog 数据读取、Checkout 确认、分支状态、远程操作或真实 Windows 原生窗口验收；不把本切片扩展为真实仓库 Reflog 回放。
- 提交：设计确认提交为 `fe2cddaa`，代码提交为 `6859457b`，均已推送到 `origin/main`。
- 证据边界：本切片证明 Reflog 行和 Checkout 入口在列表重排后不再依赖可见数组下标，并保留原有查询字段匹配测试；headless 渲染覆盖三种窗口尺寸，但不提供真实 Windows 原生窗口的像素边界、鼠标和键盘证据。

### B-GIT-001-O：仓库列表行使用 RepoId 稳定标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：仓库管理页经过搜索和最近打开时间排序后，行、名称、路径、操作区和移除按钮使用可见行下标生成节点 ID。搜索条件变化或仓库顺序刷新后，旧节点可能被复用给另一仓库；打开和移除回调虽然保存路径，但自动化定位仍依赖行位置。
- 设计：使用每个 `RepoConfig.id` 作为仓库行的稳定目标，为行、名称、路径、操作区和移除按钮生成稳定 ID；回调继续捕获仓库路径，不改变最近仓库排序、搜索和移除确认逻辑。
- 状态边界：稳定标识只用于 GPUI 节点身份和调试选择器，不作为磁盘路径或持久化键，不改变仓库打开、移除、搜索或排序；同一个 `RepoId` 在搜索结果变化后保持相同节点标识。
- 验收结果：仓库行、名称、路径、操作区和移除按钮改为使用 `RepoId` 生成稳定 ID；现有稳定 ID 单元测试与仓库列表 headless 回归通过。回归测试在仓库顺序反转后于 `360x720`、`1024x720` 和 `1440x720` 重新渲染，并通过稳定选择器找到原仓库行及名称子控件；`ramag-tool-vcs` 全量 162 项测试中 157 项通过、5 项性能观察测试按设计忽略，workspace fmt、Clippy、源码尺寸和 `git diff --check` 均通过。
- 不做事项：不改变仓库列表排序算法、搜索字段、打开/移除行为、持久化数据或真实 Windows 原生窗口验收；不把本切片扩展为真实仓库打开或远程 Git 回放。
- 提交：设计确认提交为 `b3760852`，代码提交为 `b3738a96`，均已推送到 `origin/main`。
- 证据边界：本切片证明仓库列表节点和调试选择器在顺序变化后继续绑定同一个 `RepoId`，并覆盖三种 headless 窗口尺寸；不提供真实 Windows 原生窗口的像素边界、鼠标和键盘证据。

### B-GIT-001-P：首次 Push 远程选择按钮使用远程名稳定标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：首次 Push 需要用户从多个远程仓库中选择目标时，选择按钮使用弹窗内的可见数组下标生成 ID。按钮回调虽然保存远程名，但界面定位依赖当前位置，远程列表顺序变化后无法继续用同一目标标识定位。
- 设计：以 Git 远程名作为按钮的稳定目标，使用现有稳定 ID 生成规则创建 GPUI 按钮 ID，同时保留远程名、当前分支和强推确认回调；按钮调试选择器也使用远程名，便于 headless 验收按实际目标定位。
- 状态边界：稳定标识只用于首次 Push 选择弹窗的按钮节点和调试选择器，不改变远程选择、分支设置、强推确认或 Push 执行；Git 远程名仍由现有输入限制和 Git 校验负责约束。
- 验收结果：首次 Push 选择按钮改为使用远程名经过稳定 ID 规则生成节点 ID，并增加远程名调试选择器；稳定 ID 单元测试确认同一远程名稳定、不同远程名分离，headless 测试打开包含 `upstream` 和 `fork` 的选择弹窗并按远程名找到两个按钮。`ramag-tool-vcs` 全量 164 项测试中 159 项通过、5 项性能观察测试按设计忽略，workspace fmt、Clippy、源码尺寸和 `git diff --check` 均通过。
- 不做事项：不改变远程仓库配置、Push 参数、upstream 设置、强推风险提示或真实远程 Git 回放；不把弹窗 headless 证据写成真实 Windows 原生窗口验收。
- 提交：设计确认提交为 `25f3e059`，代码提交为 `7dab7ad0`，均已推送到 `origin/main`。
- 证据边界：本切片证明首次 Push 弹窗按钮可按远程名稳定定位，并覆盖两个远程目标的 headless 弹窗渲染；不提供真实 Windows 原生窗口的鼠标、键盘和远程 Git 操作证据。

### B-GIT-001-Q：Diff 改动片段操作使用稳定目标标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：Split Diff 中“暂存此段”和“丢弃此段”按钮使用当前 hunk 数组下标生成 ID，并把下标直接传给异步操作；当前 Diff 刷新或改动片段顺序变化后，旧回调可能把操作应用到另一段改动。
- 设计：用文件路径、旧路径、hunk 头信息和完整行内容计算当前改动片段的稳定键；按钮 ID、调试选择器和点击回调都保存该键，执行操作前在当前 Diff 中按稳定键重新找到 hunk。找不到时提示 Diff 已更新并拒绝操作，不使用相邻下标兜底。
- 状态边界：稳定键只用于当前 Diff 的 hunk 操作节点和回调重新定位，不改变 Git patch 内容、暂存/丢弃分流、确认弹窗或工作区刷新；同一段内容在刷新和列表顺序变化后保持相同目标。
- 验收结果：新增稳定 hunk 键，内容包含文件路径、旧路径、hunk 头信息和完整行内容；暂存/丢弃按钮和回调改为保存稳定键，执行前按当前 Diff 重新定位，找不到时提示 Diff 已更新并停止操作。稳定键测试确认两段 hunk 反转后仍能定位原片段，缺失键不会回退到其它下标；headless 测试在两段 hunk 顺序反转后仍找到原片段的暂存和丢弃按钮。`ramag-tool-vcs` 全量 166 项测试中 161 项通过、5 项性能观察测试按设计忽略，workspace fmt、Clippy、源码尺寸和 `git diff --check` 均通过。
- 不做事项：不改变 Git patch 格式、暂存/丢弃权限、远程 Git 行为、真实仓库回放或真实 Windows 原生窗口验收。
- 提交：设计确认提交为 `7ecf7350`，代码提交为 `8a2ba3bf`，均已推送到 `origin/main`。
- 证据边界：本切片证明 Diff hunk 节点和暂存/丢弃回调在顺序变化后继续绑定同一段内容，并覆盖 headless 重排回归；不提供真实仓库 Git 操作、真实 Windows 原生窗口鼠标和键盘证据。

### B-GIT-001-R：Diff 行节点使用稳定目标标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：Unified 和 Split Diff 的行、行号、代码内容及左右栏节点 ID 使用 hunk 下标和行下标生成。Diff 刷新或 hunk 顺序变化后，同一个 GPUI 节点 ID 可能被复用给另一行，影响调试定位和节点状态复用；行号点击回调本身使用实际行号，不改变这次切片的业务语义。
- 设计：使用当前文件路径、旧路径、hunk 内容和 hunk 内行位置生成稳定行目标；Unified 行以及 Split 的左右行号、代码内容节点统一使用稳定 ID，并保留复制文本、inline blame 和差异渲染逻辑。hunk 重新排序时，同一 hunk 内相同行继续保持同一目标。
- 状态边界：稳定 ID 只用于 Diff GPUI 节点和调试选择器，不改变 patch、复制、blame、滚动、语法高亮或 hunk 暂存/丢弃行为；不把行位置 ID 用作 Git 文件路径、持久化键或异步请求代次。
- 验收结果：Unified 行以及 Split 的左右行号、代码内容节点改为使用文件 Diff 的稳定 hunk 键和 hunk 内行位置生成 ID，并增加行节点调试选择器。稳定 ID 单元测试确认 hunk 重排后同一行 ID 不变、不同位置分离；headless 测试覆盖 Split hunk 顺序反转和 Unified 行渲染，并按原行选择器找到节点。`ramag-tool-vcs` 全量 166 项测试中 161 项通过、5 项性能观察测试按设计忽略，workspace fmt、Clippy、源码尺寸和 `git diff --check` 均通过。
- 不做事项：不改变 Diff 对齐算法、语法高亮、blame 请求、Git patch 内容、hunk 操作或真实 Windows 原生窗口验收。
- 提交：设计确认提交为 `ed81d6c8`，代码提交为 `d309828e`，均已推送到 `origin/main`。
- 证据边界：本切片证明 Unified/Split Diff 行节点在 hunk 顺序变化后继续绑定原 hunk 内容，并覆盖 headless 行选择器回归；不提供真实仓库操作、真实 Windows 原生窗口鼠标和键盘证据。

### B-GIT-001-S：Diff 折叠占位行使用稳定目标标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：Split Diff 的长 Context 折叠占位行使用 `hunk_idx` 和 Context 段首行下标生成节点 ID，用户点击后也把这两个下标写入 `expanded_diff_spacers`。Diff 刷新或 hunk 顺序变化后，原来的展开状态可能落到另一段内容，节点调试定位也会跟着数组位置变化。
- 设计：以文件路径、旧路径、hunk 完整内容和 Context 段首行位置计算稳定占位键；`SplitKey::Spacer`、折叠占位行节点 ID、调试选择器和展开状态统一使用该键。hunk 重排后按稳定键保留原占位行的展开状态，未找到对应 hunk 时不按相邻下标兜底。
- 状态边界：只调整折叠占位行的 GPUI 节点身份和展开状态索引，不改变 Context 折叠阈值、首尾保留行、左右栏对齐、点击展开语义、滚动或 Git Diff 内容。
- 验收条件：稳定占位键测试确认 hunk 重排后仍定位同一 Context 段，且展开第一段不会误展开另一段；headless 测试在长 Context Diff 中按稳定选择器找到占位行，并在 hunk 顺序变化后继续找到原目标；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变 Diff 对齐算法、语法高亮、hunk 暂存/丢弃、Git patch 内容、真实仓库回放或真实 Windows 原生窗口验收。
- 实现：新增稳定折叠占位键，把 `expanded_diff_spacers` 和 `SplitKey::Spacer` 的展开状态从 `(hunk_idx, run_start)` 改为稳定 `u64` 键；占位行 ID 和调试选择器使用稳定键，hunk 重排后继续绑定原 Context 段。
- 验收结果：稳定键单元测试确认 hunk 重排后键不变、Context 段位置变化后键分离；折叠布局测试确认已展开的第一段不会误展开另一段；headless 测试 `diff_spacer_keeps_stable_selector_and_expansion_after_reorder` 在两段长 Context Diff 中按稳定选择器定位占位行，并在 hunk 顺序变化后确认原目标展开、另一目标仍显示。`ramag-tool-vcs` 全量 169 项测试中 164 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 提交：设计确认提交为 `b1d704f1`；代码提交为 `959ecb3c fix(vcs): stabilize diff spacer expansion`，验收记录提交为 `9ccdeb22 docs: record diff spacer identity evidence`，均已推送到 `origin/main`。
- 证据边界：本切片证明折叠占位行的状态和节点定位不再依赖 hunk 数组下标，并覆盖 headless 重排回归；不提供真实仓库 Git 操作、真实 Windows 原生窗口鼠标和键盘证据。

### B-GIT-001-T：分支、远程和标签侧栏使用安全稳定标识（代码与 headless 回归完成，2026-09-29）

- 问题证据：分支、远程配置和标签侧栏把用户可创建的名称直接拼进 GPUI 节点 ID。Git 分支允许包含 `/` 等路径字符，远程名和标签也可能包含空格或标点；列表刷新、重排或调试选择器解析时，原始名称会让节点身份依赖输入格式，且不同引用类别的标识规则不统一。
- 设计：按引用类别和完整名称计算固定格式的稳定 ID；本地分支与远程分支明确区分，远程配置和标签使用独立前缀。行、更多操作按钮和 headless 调试选择器统一使用稳定 ID，点击、右键菜单和业务回调继续捕获完整名称，不改变 Git ref 参数。
- 状态边界：只调整侧栏 GPUI 节点身份和调试定位，不改变分支/远程/标签排序、折叠、历史过滤、比较、检出、合并、变基、推送、删除或地址修改语义；稳定 ID 不作为 Git 命令参数或持久化键。
- 验收条件：稳定 ID 纯函数测试确认同名稳定、引用类别隔离、包含 `/`/空格的名称不会生成原始名称节点；headless 历史侧栏测试在本地分支、远程分支、远程配置和标签重排后按稳定选择器找到原目标；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变 Git ref 校验、远程协议、历史查询、真实仓库回放或真实 Windows 原生窗口验收；SSH 终端/端口转发集成测试仍由用户自行验证。
- 实现：新增分支、远程配置和标签的稳定 ID 辅助函数；行节点、名称节点和更多操作按钮统一使用按类别与完整名称计算的稳定标识，不再把 `/`、空格或其它用户名称直接拼入节点 ID；保留完整名称用于点击、右键菜单和 Git 操作参数。侧栏名称文件不再直接导入 `SharedString`。
- 验收结果：稳定 ID 单元测试 `sidebar_ref_ids_are_stable_and_category_specific` 确认同名稳定、本地与远程分支隔离、名称中的 `/` 与空格不会造成类别混淆；headless 测试 `history_ref_rows_use_safe_stable_selectors_for_named_refs` 覆盖四类引用、名称重排以及行和操作按钮选择器。`ramag-tool-vcs` 全量 171 项测试中 166 项通过、5 项性能观察测试按设计忽略；`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 提交：设计确认提交为 `ef4a0033`；代码提交为 `bad8ed3c fix(vcs): stabilize git ref sidebar selectors`，已推送到 `origin/main`。
- 证据边界：本切片只证明 GPUI 节点身份和 headless 调试定位在名称边界、类别区分及列表重排后保持稳定；不提供真实仓库 Git 操作、真实 Windows 原生窗口鼠标和键盘验收，SSH 集成测试仍由用户自行验证。

### B-GIT-001-U：提交历史行窄窗口布局与稳定定位（设计确认，2026-09-29）

- 问题证据：提交历史行把作者、相对时间和短哈希固定为 140/96/70px，历史面板在 360px 窗口中还要分配左侧导航栏，固定列会挤出内容区；行节点 ID 只取提交 ID 的前 12 个字符，极短 ID 或相同前缀会造成定位不稳定。
- 设计：提交行保留提交说明、引用标签、作者、时间和短哈希的信息层级；作者、时间和哈希列允许收缩并对文本做省略，引用标签容器在窄宽度内裁剪，主体说明继续占用剩余空间。行 ID 和调试选择器使用完整提交 ID 的有界哈希，点击与上下文菜单仍携带完整 ID。
- 验收条件：headless 渲染在 360px、720px 和 1440px 窗口中确认提交行不超出历史内容区域；稳定标识测试确认相同提交稳定、仅前缀相同的不同提交仍分离；`ramag-tool-vcs` 目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不改变历史查询、分页、选中、复制、比较、Cherry-pick、Checkout、Revert、Reset 或远程 Git 语义，不把本项扩展为真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再调整提交行布局和稳定标识，补充窄窗口 headless 回归；验证通过后独立提交并推送，随后补充本切片验收记录。

### B-GIT-001-U：提交历史行窄窗口布局与稳定定位（代码与 headless 验收完成，2026-09-29）

- 实现：提交行的作者、时间和短哈希列改为可收缩且允许文本省略，引用标签容器在窄宽度裁剪；行节点和调试选择器改为完整提交 ID 的有界哈希，点击和上下文菜单继续携带完整 ID。
- 验收结果：`cargo test --locked -p ramag-tool-vcs --lib -- --test-threads=1` 通过 166 项，5 项性能观察测试按设计忽略；新增稳定 ID 回归和 360/720/1440px 提交行边界测试通过。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- 真实窗口：Computer Use 初始化后 `cua.getState()` 在启动前后均返回 `apps: []`，虽然本机进程可启动并显示 `Ramag — 容器管理` 窗口，但运行时没有可操作的原生窗口；本切片只保留 headless 证据，不把进程启动描述为真实窗口验收。
- 证据边界：本切片只证明提交历史行在支持窗口宽度内保持布局边界，并且节点定位不依赖短哈希前缀；不提供真实 Windows 鼠标/键盘流程或远程 Git 回放。

### B-GIT-001-V：创建 Tag 与远程仓库弹窗自适应（设计确认，2026-09-29）

- 问题证据：历史侧栏的两种创建弹窗使用 520/560px 固定宽度和 160px 顶部偏移，字段仅有占位提示；短窗口下底部操作缺少空间，输入后字段用途也不清晰。
- 设计确认：沿用已确认的紧凑 GPUI 弹窗规范，按当前视口计算宽度、顶部偏移和最大高度；正文独立滚动，标题、取消与创建操作保留在可视范围。输入框增加持久字段标签，复用共享底部操作区。
- 验收条件：在 360x240、360x640、1024x768、1440x900 下打开两种弹窗，检查标题、正文视口、底部按钮边界与重叠；输入、取消和关闭不发起 Git 写操作，重新打开清空旧输入；最终 VCS 回归、workspace fmt/Clippy、源码尺寸、LF 和 diff 检查通过后单独提交并推送 main。
- 范围：仅处理两个创建表单的呈现和取消流程；真实窗口证据以当前 Computer Use 能力为准，headless 证据单独记录。本切片不改变 Git 驱动，因此无需 Docker 集成。
- 实现与验证：两种表单改用响应式宽度和顶部偏移，正文设置独立有界滚动区；字段增加“标签名称/备注”“远程名称/仓库地址”标签；底部操作复用共享响应式操作区。headless 测试覆盖四种窗口尺寸、标题/字段/操作区边界、输入清空和取消路径；`ramag-tool-vcs` 167 项通过，5 项性能观察测试按设计忽略，fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。提交 `eaf872c7` 已推送 `origin/main`。
- 真实窗口：本轮 Computer Use `cua.getState()` 仍返回 `apps: []`，没有可操作的 Ramag 原生窗口；保留 headless 证据，不把进程启动或静态结果写成真实窗口验收。

### B-GIT-001-W：首次 Push 远程选择弹窗响应式布局（设计确认，2026-09-29）

- 问题证据：首次 Push 远程选择弹窗仍使用 520px 固定宽度和 150px 顶部偏移；远程列表直接按内容增长，远程名称或分支较长、窗口高度较小时，取消按钮和选择项可能被推到弹窗可视区域之外。
- 设计确认：复用共享响应式弹窗宽度和顶部偏移；远程选项放入独立有界滚动区，按钮保持整行命中和长文本裁剪，取消操作固定在底部。远程名仍作为稳定选择目标，Push 与强推回调保持原有参数和确认流程。
- 验收条件：在 360x240、360x640、1024x768、1440x900 下打开首次 Push 选择，确认弹窗、远程按钮和取消按钮位于窗口内；多远程列表可滚动，取消不发起 Git 操作；VCS 回归、workspace fmt/Clippy、源码尺寸、LF 和 diff 检查通过后独立提交并推送 main。
- 范围：只调整首次 Push 选择弹窗的布局和可见性，不改变远程选择、Push、强推确认或 Git 驱动语义；本切片不改变 Git 协议，因此无需 Docker 集成。
- 实现与验证：首次 Push 选择改用响应式宽度和顶部偏移，远程按钮放入独立有界滚动区，取消区和选择器均提供稳定调试选择器；headless 测试覆盖四种窗口尺寸、弹窗/选项/取消边界和取消路径。`ramag-tool-vcs` 167 项通过，5 项性能观察测试按设计忽略，fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 真实窗口：Computer Use 当前仍返回 `apps: []`，没有可操作的 Ramag 原生窗口；本切片保留 headless 证据，不把静态结果写成真实窗口验收。

### B-GIT-001-W：首次 Push 远程选择弹窗响应式布局（代码与 headless 验收完成，2026-09-29）

- 实现：首次 Push/强推远程选择弹窗复用响应式宽度和顶部偏移；远程按钮放入独立有界滚动区，底部取消区固定可见，长远程名和分支名不把操作区推出窗口；点击远程仍使用完整远程名执行原有 Push 或强推确认流程。
- 验收结果：`first_push_remote_picker_uses_remote_name_selectors` 在 `360x240`、`360x640`、`1024x768` 和 `1440x900` 检查弹窗、远程选项、取消区边界及取消关闭；`ramag-tool-vcs` 当时 167 项通过、5 项性能观察测试按设计忽略，fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 真实窗口：Computer Use 当前没有可操作的 Ramag 原生窗口；本记录只保留 headless 证据，不把系统截图或进程启动描述为真实窗口验收。
- Git：代码提交 `d175e215`、验收记录提交 `3bfa2b3a` 均已推送 `main`。

### B-GIT-001-X：提交历史分页位置与重复提交保护（设计确认，2026-09-30）

- 问题证据：历史侧栏目前用已保留行数作为 Git `log` 的下一页 `skip`。如果异步刷新或远程回放返回了已经显示过的完整提交 ID，过滤重复项后保留行数就不再等于 Git 分页位置，下一次加载可能重复请求同一页；提交图和行节点也缺少统一的重复保护。
- 设计：每次历史请求记录本次请求的 `skip`，成功回包后按“请求位置 + 实际返回条数”计算下一次分页位置，不用本地保留行数推算。历史替换和分页追加都按完整提交 ID 去重，保留首次出现的提交和现有 `Rc` 分配；仅因达到条数或内存上限停止时才标记历史缓存已到上限。请求代际继续作为回包写入条件，旧查询结果不能写入当前搜索、引用或路径过滤结果。
- 验收条件：补充历史缓存替换和追加的重复 ID 测试、分页位置和无新增结果停止加载测试；`ramag-tool-vcs` 目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。测试证明重复回包不会重复绘制提交行或提交图，也不会因重复项误报缓存上限。
- 范围：只调整历史列表的分页位置、重复保护和对应 headless/单元测试；不改变 Git `log` 参数含义、搜索语法、引用/路径过滤、提交详情、比较和任何 Git 写操作。
- 不做事项：不引入服务端游标，不改变 Git 驱动接口，不把本地 headless 结果写成真实 Windows 原生窗口证据；SSH 集成测试仍由用户自行验证。
- 实施顺序：先提交本设计确认，再实现分页位置和重复保护；目标测试通过后独立提交并推送 `main`，随后进入阶段 A 的质量与发布检查。

### B-GIT-001-X：提交历史分页位置与重复提交保护（代码与 headless 验收完成，2026-09-30）

- 实现：新增独立的下一页 `skip` 状态，加载更多时使用 Git 已返回条数推进分页位置，不再使用本地去重后的行数；历史替换和追加按完整提交 ID 去重，重复回包不会重复生成提交图行；重复项不会误报条数或内存上限，真正达到缓存边界时仍停止继续加载。
- 验收结果：历史缓存重复替换/追加、`Rc` 复用、分页位置溢出保护和无新增结果停止加载测试通过；`ramag-tool-vcs` 全量 178 项中 173 项通过、5 项性能观察测试按设计忽略。workspace fmt、Clippy、源码尺寸、LF 和 `git diff --check` 在提交前通过。
- 真实窗口：当前环境没有可操作的 Computer Use 原生窗口；本切片只保留 headless 和单元测试证据，不把静态测试或进程启动描述为真实 Windows 窗口验收。
- 证据边界：本切片只证明 Git 历史分页位置、完整提交 ID 去重和异步请求代际保护；不改变 Git 驱动接口、搜索/引用/路径过滤或 Git 写操作，SSH 集成测试仍由用户自行验证。
- Git：设计确认提交 `c072dfb2` 已推送；代码提交完成后进入阶段 A 的 `QUALITY-UX-001` 质量与发布检查。

### B-GIT-001-Y：HEAD 变化后提交详情目标失效（设计确认，2026-09-30）

- 问题证据：checkout、merge、rebase、revert、reset 或外部 HEAD 变化会刷新历史和工作区，但当前刷新流程只清理 diff/文件缓存，不主动关闭已打开的提交详情；旧提交可能继续显示，正在进行的详情回包也可能把旧文件树写回当前工作区。
- 设计：在 `refresh_after_head_change` 开始时统一关闭提交详情，递增详情请求代际并清空提交文件树、选中文件和加载状态；现有 Commit 文件标签仍按当前 HEAD 刷新逻辑重新定位，不改变 Git 操作参数。
- 验收条件：新增 headless 回归，注入提交详情和加载状态后触发 HEAD 刷新，确认详情目标、文件树、选中文件、加载状态被清理且详情请求代际递增；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 范围：只处理 HEAD 变化时的提交详情生命周期和旧回包边界，不改变历史分页、Git 驱动接口、提交详情读取内容或文件标签的 Git 语义。
- 不做事项：不重新构建 `v0.4.0` 安装包，不运行 Kubernetes；SSH 集成测试和真实 Windows 窗口仍由既有验收边界覆盖。
- 实施顺序：先提交设计确认，再补详情失效和 headless 回归；通过后独立提交并推送，继续阶段 B Git 工作区边界开发。

### B-GIT-001-Y：HEAD 变化后提交详情目标失效（代码与 headless 验收完成，2026-09-30）

- 实现：`refresh_after_head_change` 在刷新工作区和历史前关闭当前提交详情，递增 `commit_detail_request_seq`，清空旧提交文件树、选中文件和加载状态；HEAD 变化期间到达的旧详情回包因此不能继续写入界面。
- 验收结果：新增 `head_change_invalidates_commit_detail_target` headless 回归，确认旧提交目标、文件树和加载状态均清理且请求代际递增；`ramag-tool-vcs` 全量 180 项中 175 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 真实窗口：Computer Use 当前没有可操作的 Ramag 原生窗口；本切片只保留 headless 证据，不把进程启动描述为真实窗口验收。
- Git：设计确认已由 `cc967cea` 推送；代码提交完成后继续阶段 B Git 工作区边界开发。

### B-GIT-001-Z：关闭分支比较后恢复既有文件标签（设计确认，2026-09-30）

- 问题证据：`clear_compare_state` 移除比较文件标签后无条件把 `active_file_tab_idx` 设为 `None`，但保留的 Changes、Project Files 或 Commit 标签仍在列表中；关闭比较或切换离开比较页后，主区因此显示空状态，用户必须重新点击文件才能回到原来的工作内容。
- 设计：清理比较标签前记录当前活动标签的 `FileTabTarget` 和原位置。移除比较标签后，优先按稳定目标恢复原来的非比较标签；如果当前活动标签本身属于比较范围，则按原位置选择相邻的剩余标签；没有剩余标签时才清空主区。恢复时同步 `active_file_tab_idx`、选择状态和已有缓存，不改变比较请求代际、Git 驱动参数或其它标签顺序。
- 改动范围：只调整比较会话关闭后的文件标签恢复和 headless 回归；不改变比较文件列表、Diff 读取、文件标签稳定 ID、Git 协议或写操作。
- 验收条件：测试覆盖活动非比较标签位于比较标签前后、活动比较标签关闭后选择相邻标签和没有剩余标签三种情况；关闭比较后保留标签数量、稳定目标、活动下标和主区选择状态一致；`ramag-tool-vcs` 目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不重新构建或上传 `v0.4.0` 安装包，不运行 Kubernetes，不新增 Docker 或远端 Git 服务；不把 headless 结果扩大为真实 Windows 原生窗口验收，SSH 集成测试继续由用户自行验证。
- 实施顺序：先提交设计确认，再实现比较标签清理后的稳定恢复和回归测试；验证通过后独立提交并推送 `main`，继续阶段 B Git 工作区边界开发。

### B-GIT-001-Z：关闭分支比较后恢复既有文件标签（代码与 headless 验收完成，2026-09-30）

- 实现：`clear_compare_state` 在移除比较标签前保存当前非比较标签的 `FileTabTarget` 和原位置；清理后先按稳定目标恢复标签，当前标签属于比较范围时按原位置选择相邻标签，最后通过既有 `activate_file_tab_state` 同步选择状态和缓存内容。没有剩余标签时继续清空主区。
- 验收结果：新增 `closing_compare_restores_stable_file_tab_selection` 回归，覆盖比较标签前后的非比较标签、活动比较标签关闭后的相邻标签和没有剩余标签三种情况；`cargo test --locked -p ramag-tool-vcs --all-targets -- --test-threads=1` 共 181 项，其中 176 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：headless 回归证明文件标签数量、稳定目标、活动下标和主区选择状态在关闭比较后可恢复；没有把 headless 结果扩大为真实 Windows 原生窗口验收，SSH 集成测试仍由用户自行验证。
- 未完成项：没有重新构建或上传 `v0.4.0` 安装包，没有运行 Kubernetes，也没有新增 Docker 或远端 Git 服务；比较文件读取、Git 参数和写操作仍未在本切片中改变。
- Git：设计确认提交 `2893f914` 已推送；实现代码、回归测试和本验收记录随本切片独立提交并推送 `main`。

### B-GIT-001-AA：工作区状态刷新后保留相邻文件标签（设计确认，2026-09-30）

- 问题证据：`sync_changes_tabs_with_status_paths` 在当前 Changes 标签对应文件已恢复干净、被删除或无法继续归类时，会删除该标签；如果原活动标签目标找不到，当前实现直接选择刷新后列表的最后一个标签。标签顺序中存在 Project Files、Commit 或其它 Changes 标签时，刷新会把用户带到无关位置，丢失原来附近的工作上下文。
- 设计：清理和重定向 Changes 标签前记录活动标签的数组位置。稳定目标仍优先用于恢复同一路径或重新归类后的标签；目标不存在时，按清理前的位置在剩余标签中取相同位置，超出长度时取最后一个；没有标签时才清空主区。恢复后复用现有选择函数或 `activate_file_tab_state`，不改变状态刷新、Git 参数和异步请求代际。
- 改动范围：只调整工作区状态刷新删除活动 Changes 标签后的选中标签恢复，并补充 headless 回归；不改变文件状态分组、路径重定向、Diff 读取、文件内容缓存、Git 写操作或 SSH 集成测试。
- 验收条件：测试覆盖活动 Changes 标签位于中间且被移除、活动标签重定向到其它 Changes 分组、活动标签前方标签被移除后三种情况；确认剩余标签顺序、稳定目标、活动下标和主区选择状态保持一致；`ramag-tool-vcs` 目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不重新构建或上传 `v0.4.0` 安装包，不启动本机 Docker，不运行 Kubernetes；不代替用户执行 114 服务器终端/端口转发集成回放，不把 headless 结果扩大为真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再实现状态刷新后的原位置恢复和 headless 回归；验证通过后独立提交并推送 `main`，继续阶段 B Git 工作区边界开发。

### B-GIT-001-AA：工作区状态刷新后保留相邻文件标签（代码与 headless 验收完成，2026-09-30）

- 实现：`sync_changes_tabs_with_status_paths` 保存刷新前的活动下标和稳定目标；状态标签被删除时，先按路径与来源恢复，目标不存在时按刷新前位置选择剩余标签，位置超出范围时取最后一个；状态分组变化仍复用既有选择函数，主区选择状态与缓存按原流程同步。
- 验收结果：新增 `status_refresh_restores_file_tab_position_after_change_removal` 回归，覆盖活动 Changes 标签被移除、Changes 标签重定向到其它分组以及前方标签被移除三种情况；`cargo test --locked -p ramag-tool-vcs --all-targets -- --test-threads=1` 共 182 项，其中 177 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：headless 回归证明状态刷新后剩余标签顺序、稳定目标、活动下标和主区选择状态按预期恢复；没有把 headless 结果扩大为真实 Windows 原生窗口验收，114 服务器终端/端口转发集成回放仍由用户自行验证。
- 未完成项：没有重新构建或上传 `v0.4.0` 安装包，没有启动本机 Docker，没有运行 Kubernetes；没有改变 Git 状态分组、Diff 读取、文件缓存或写操作协议。
- Git：设计确认提交 `db68bc34` 已先行推送；实现代码、回归测试和本验收记录随本切片独立提交并推送 `main`。

### B-GIT-001-AB：仓库会话缓存恢复稳定文件标签（设计确认，2026-09-30）

- 问题证据：`save_current_session_to_cache` 复制文件标签后会移除 Compare 标签，但仍直接保存移除前的 `active_file_tab_idx`。当活动标签是 Compare，或 Compare 标签位于活动标签之前时，恢复会话使用旧下标，可能选中其它标签、越过列表末尾，或者让主区不再对应用户离开仓库时的文件。
- 设计：缓存会话前记录活动标签的 `FileTabTarget` 和原位置；移除 Compare 标签后优先按稳定目标寻找保留的非 Compare 标签，目标属于 Compare 或已不存在时按原位置选择相邻标签，没有标签时保存空活动位置。恢复流程继续使用现有 `activate_file_tab_state`，不扩展会话持久化格式。
- 改动范围：只调整仓库会话缓存移除 Compare 标签后的活动标签下标和恢复测试；不改变仓库打开、文件内容缓存清理、提交草稿、布局尺寸、Git 驱动参数或写操作。
- 验收条件：headless 测试覆盖活动 Compare 标签、Compare 标签位于活动标签前方、没有剩余标签三种情况；确认缓存中的标签顺序、活动下标、稳定目标和恢复后的主区选择状态一致；`ramag-tool-vcs` 目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不重新构建或上传 `v0.4.0` 安装包，不启动本机 Docker，不运行 Kubernetes；不代替用户执行 114 服务器终端/端口转发集成回放，不把 headless 结果扩大为真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再实现会话缓存的稳定标签恢复和 headless 回归；验证通过后独立提交并推送 `main`，继续阶段 B Git 工作区边界开发。

### B-GIT-001-AB：仓库会话缓存恢复稳定文件标签（代码与 headless 验收完成，2026-09-30）

- 实现：新增会话缓存标签整理步骤，在移除 Compare 标签前记录活动标签的稳定目标和原位置；保存时优先按稳定目标计算新的活动下标，活动标签属于 Compare 或已不存在时按原位置选择相邻标签，并继续清理缓存正文。恢复流程保持现有标签顺序和 `activate_file_tab_state` 行为，不改变会话缓存结构。
- 验收结果：新增 `cached_session_tabs_restore_non_compare_target_or_neighbor` 和 `restoring_repo_session_keeps_non_compare_file_tab_active`，覆盖活动非比较标签、活动 Compare 标签和 Compare-only 标签；`cargo test --locked -p ramag-tool-vcs --all-targets -- --test-threads=1` 共 184 项，其中 179 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：headless GPUI 回归实际调用会话保存与恢复流程，确认缓存下标、标签顺序和恢复后的主区选择状态一致；没有把 headless 结果扩大为真实 Windows 原生窗口验收，114 服务器终端/端口转发集成回放仍由用户自行验证。
- 未完成项：没有重新构建或上传 `v0.4.0` 安装包，没有启动本机 Docker，没有运行 Kubernetes；没有改变 Git 驱动参数、文件内容读取、提交草稿或仓库写操作。
- Git：设计确认提交 `5124b872` 已先行推送；实现代码、回归测试和本验收记录随本切片独立提交并推送 `main`。

### B-GIT-001-AC：提交信息复制请求按最近点击生效（设计确认，2026-09-30）

- 问题证据：`copy_commit_message` 为每次点击启动独立的 `commit_details` 请求，但没有记录请求代际。连续点击不同提交时，响应先后顺序不稳定，较早点击的迟到回包仍可能写入剪贴板并显示成功提示，最终结果不一定对应最后一次点击。
- 设计：增加提交信息复制请求代际。每次点击递增代际并捕获本次值；回包只有在仓库仍然一致且代际仍为最新时，才能写剪贴板、显示成功提示或写入错误。切换仓库和重置仓库状态时同时使旧复制回包失效，不改变提交详情读取接口和复制文本内容。
- 改动范围：只调整提交信息复制的异步回包边界和代际测试；不改变历史列表、提交详情、剪贴板格式、Git 驱动参数或其它文件标签请求。
- 验收条件：纯状态测试确认旧代际回包被拒绝、最新代际回包被接受；`ramag-tool-vcs` 目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不重新构建或上传 `v0.4.0` 安装包，不启动本机 Docker，不运行 Kubernetes；不代替用户执行 114 服务器终端/端口转发集成回放，不把 headless 结果扩大为真实 Windows 原生窗口验收。
- 实施顺序：先提交本设计确认，再实现复制请求代际和目标测试；验证通过后独立提交并推送 `main`，继续阶段 B Git 工作区边界开发。

### B-GIT-001-AC：提交信息复制请求按最近点击生效（代码与 headless 验收完成，2026-09-30）

- 实现：`VcsView` 新增提交信息复制请求代际；每次点击复制时递增代际，`commit_details` 回包只有在仓库一致且仍是最新请求时才写入剪贴板、显示成功提示或写入错误；仓库重置时同时使旧复制回包失效。
- 验收结果：新增 `commit_copy_response_only_applies_to_latest_request`，确认旧代际回包被拒绝、最新代际回包被接受；`cargo test --locked -p ramag-tool-vcs --all-targets -- --test-threads=1` 共 185 项，其中 180 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：测试证明复制回包的代际判断不会接受旧请求，代码路径保留原有完整提交信息格式；没有把 headless 结果扩大为真实 Windows 原生剪贴板交互验收，114 服务器终端/端口转发集成回放仍由用户自行验证。
- 未完成项：没有重新构建或上传 `v0.4.0` 安装包，没有启动本机 Docker，没有运行 Kubernetes；没有改变提交详情读取、历史列表、Git 驱动参数或其它剪贴板入口。
- Git：设计确认提交 `5106ad6a` 已先行推送；实现代码、回归测试和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-005：VCS 文件内容状态使用主题语义色（设计确认，2026-09-30）

- 问题证据：VCS Project Files 内容区的文件读取错误直接使用固定红色，大文件截断提示的背景直接使用固定黄色；切换浅色/深色主题后，这两个状态不会跟随当前主题的 `danger` 和 `warning` 语义色。
- 设计：Project Files 渲染从当前主题读取 `danger` 作为读取错误文字颜色，读取 `warning` 作为大文件截断提示背景，并保留弱化正文色、编辑禁用和 4 MiB 读取上限。Diff 增删颜色、Git 状态类别色和代码语法高亮不在本切片内调整。
- 改动范围：只调整 `ramag-tool-vcs` Project Files 内容区的错误/警告颜色入口，并补充颜色映射和现有渲染回归；不改变文件读取、编辑保存、截断判定、Markdown 预览或 Git 操作。
- 验收条件：VCS 生产代码不再为 Project Files 错误和截断提示直接创建固定红/黄颜色；浅色/深色主题下 headless 渲染读取当前主题语义色；`ramag-tool-vcs` 目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不启动本机 Docker，不重新构建或上传 `v0.4.0` 安装包，不运行 Kubernetes；不把 headless 主题颜色结果扩大为真实 Windows 原生窗口主题切换验收，SSH 集成测试继续由用户自行验证。
- 实施顺序：先提交本设计确认，再替换 Project Files 状态颜色来源并补回归；验证通过后独立提交并推送 `main`，继续阶段 A 质量收口。

### A-QUALITY-THEME-005：VCS 文件内容状态使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 实现：Project Files 内容区的读取错误改为使用当前主题 `danger`，大文件截断提示背景改为使用当前主题 `warning` 并保留原有透明度；文件读取、编辑禁用、Markdown 预览和 4 MiB 上限未改变。
- 测试：新增 `project_file_state_colors_follow_theme_tokens`，确认错误和截断状态颜色直接采用调用方传入的主题语义色；`ramag-tool-vcs` 全量 186 项中 181 项通过、5 项性能观察测试按设计忽略，现有 Project Files 渲染和三种窗口布局回归继续通过。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：headless 测试证明颜色入口不再固定使用红/黄值；没有把 headless 主题结果扩大为真实 Windows 原生窗口主题切换验收，114 服务器终端/端口转发集成回放仍由用户自行验证。
- 未完成项：没有启动本机 Docker，没有重新构建或上传 `v0.4.0` 安装包，没有运行 Kubernetes；Diff 增删色、Git 状态类别色和语法高亮仍保持现状。
- Git：设计确认提交 `1610ecb8` 已先行推送；实现代码、回归测试和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-006：Git 状态颜色使用主题语义色（设计确认，2026-09-30）

- 问题证据：Git 工作区的未暂存组徽标、Changes 文件标签圆点以及状态字母 `M/A/D/R/C/T/U` 仍使用固定 HSLA 色值；明暗主题变化时，这些状态颜色不会跟随应用主题，且相同的状态在不同区域可能颜色不一致。
- 设计：把修改、添加、删除、重命名/复制、类型变化和冲突分别映射到当前主题的 `warning`、`success`、`danger`、`info`、`info` 和 `danger`；变更组徽标与对应 Changes 标签圆点使用相同的组颜色：未暂存为 `warning`、冲突为 `danger`、暂存为 `accent`、未跟踪为弱化正文色。Project Files、Commit、Compare 标签的来源分类色不在本切片改动范围。
- 改动范围：统一 Git 变更组徽标、Changes 文件标签圆点及工作区/比较/提交详情/Project Files 文件状态字母的颜色映射，并补充纯函数测试；不改变文件状态分类、Git 命令、行身份、标签交互或 Diff 内容。
- 验收条件：状态颜色映射测试确认 `M/A/D/R/C/T/U` 分别读取预期主题色；headless 渲染在现有 Git 工作区、比较列表和提交详情路径通过；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不启动本机 Docker，不重新构建或上传 `v0.4.0` 安装包，不运行 Kubernetes；不修改 Diff 增删行底色、Reflog 操作类型色或 Project Files/Commit/Compare 来源分类色，不把 headless 结果扩大为真实 Windows 原生窗口主题切换验收。
- 实施顺序：先提交本设计确认，再把 Git 状态颜色映射切换到当前主题并补回归；验证通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-006：Git 状态颜色使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 实现：新增共享的 Git 状态颜色映射，从当前主题读取状态色；工作区分组徽标和 Changes 标签圆点按暂存、未暂存、冲突、未跟踪状态显示；工作区、比较、提交详情和 Project Files 中的 `M/A/D/R/C/T/U` 状态字母改用主题语义色。文件状态分类、Git 命令、标签来源分类色和 Diff 增删行底色保持不变。
- 测试：新增状态字母和变更组颜色映射测试，确认 `M/A/D/R/C/T/U` 及四种分组使用预期主题色或弱化正文色；`cargo test --locked -p ramag-tool-vcs --all-targets -- --test-threads=1` 通过，188 项中 183 项通过、5 项性能观察测试按设计忽略；现有工作区、比较列表、提交详情和 Project Files headless 渲染回归通过。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过。
- 证据边界：颜色映射单元测试和 headless 渲染回归通过；没有把 headless 结果扩大为真实 Windows 原生窗口明暗主题切换验收。
- 未完成项：没有启动本机 Docker 或构建 Release 安装包，没有运行 Kubernetes；真实窗口主题切换仍待原生窗口验收环境恢复。
- Git：设计确认提交 `e5f81437` 已先行推送；实现代码和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-007：Reflog 操作标签使用主题语义色（设计确认，2026-09-30）

- 问题证据：Reflog 中 `checkout`、`reset`、`merge` 和 `rebase` 操作标签直接使用固定 HSLA 颜色，切换主题后颜色不变；提交操作和未知操作已分别使用主题 `accent` 与 `muted_foreground`。
- 设计：提交操作继续使用 `accent`；`checkout` 与 `merge/rebase` 使用主题 `info`；`reset` 使用主题 `danger`；未知操作继续使用 `muted_foreground`。只替换颜色来源，保留操作文字、行布局和交互。
- 改动范围：仅调整 Reflog 操作标签的颜色映射并补充映射测试；不改变筛选、缓存、提交 ID、Checkout 操作或 Reflog 数据来源。
- 验收条件：映射测试覆盖提交、Checkout、Reset、Merge/Rebase 及未知操作；`ramag-tool-vcs` 全量 headless 测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不调整 Commit/Tag/HEAD/远程分支引用标签颜色，不启动本机 Docker，不构建 `v0.4.0` Release 安装包，不运行 Kubernetes；不把 headless 结果扩大为真实 Windows 原生窗口主题切换验收。
- 实施顺序：先推送本设计确认，再修改 Reflog 颜色来源并补测试；验证通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-007：Reflog 操作标签使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 实现：Reflog 提交操作继续使用当前主题 `accent`；`checkout`、`merge`、`rebase` 使用 `info`，`reset` 使用 `danger`，未知操作继续使用 `muted_foreground`。筛选、缓存、操作文字和 Checkout 行为未变。
- 测试：新增操作颜色映射测试，覆盖提交、Checkout、Reset、Merge/Rebase 和未知操作；`cargo test --locked -p ramag-tool-vcs --all-targets -- --test-threads=1` 通过，189 项中 184 项通过、5 项性能观察测试按设计忽略，现有 Reflog headless 行回归通过。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过。
- 证据边界：操作标签映射测试和现有 headless 渲染回归通过；没有把 headless 结果扩大为真实 Windows 原生窗口明暗主题切换验收。
- 未完成项：没有启动本机 Docker 或构建 Release 安装包，没有运行 Kubernetes；真实窗口主题切换仍待原生窗口验收环境恢复。
- Git：设计确认提交 `3c48e896` 已先行推送；实现代码和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-008：提交历史引用标签使用主题语义色（设计确认，2026-09-30）

- 问题证据：提交历史行中的 Tag、HEAD 和带斜线的引用标签仍在 `ref_chip` 内直接使用固定 HSLA 值；切换浅色或深色主题时，这些文字和背景颜色不会采用当前主题配色。
- 设计：保留现有引用识别和显示规则，只替换颜色来源：Tag 用主题 `warning`，HEAD 用 `success`，带斜线的引用用 `info`，其它引用和省略提示用 `accent`；背景继续从文字颜色复制并使用现有透明度。
- 改动范围：提交历史行引用标签的颜色选择和纯函数测试；不改变 Git 引用解析、名称截断、排序、提交行布局、操作菜单或任何 Git 命令。
- 验收条件：映射测试覆盖 Tag、HEAD、带斜线引用、普通引用和省略提示，并确认标签文字与半透明背景使用同一主题色；VCS headless 行渲染、`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不启动本机 Docker，不重新构建或上传 `v0.4.0` 安装包，不运行 Kubernetes；不把 headless 测试扩大为真实 Windows 原生窗口主题切换验收。
- 实施顺序：先推送设计确认，再接入当前主题颜色并补映射测试；验证通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-008：提交历史引用标签使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 实现：提交历史中的 Tag 使用当前主题 `warning`，HEAD 使用 `success`，带斜线的引用使用 `info`，普通引用和省略提示使用 `accent`；标签文字和半透明背景共用同一主题颜色。Git 引用解析、名称截断、提交行布局和操作菜单未改变。
- 测试：新增引用标签颜色映射测试，覆盖 Tag、HEAD、带斜线引用、普通引用和省略提示；`cargo test --locked -p ramag-tool-vcs --all-targets -- --test-threads=1` 通过，190 项中 185 项通过、5 项性能观察测试按设计忽略；现有提交历史 headless 行渲染回归通过。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过。
- 证据边界：引用颜色映射测试和提交历史 headless 渲染回归通过；没有把 headless 结果扩大为真实 Windows 原生窗口明暗主题切换验收。
- 未完成项：没有启动本机 Docker 或构建 Release 安装包，没有运行 Kubernetes；真实窗口主题切换仍待原生窗口验收环境恢复。
- Git：设计确认提交 `d045e0a2` 已先行推送；实现代码和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-009：Git 侧栏引用图标使用主题语义色（设计确认，2026-09-30）

- 问题证据：Git 侧栏 Tag 的圆点和远程仓库的 Globe 图标仍直接使用固定 HSLA 色值；主题切换后，引用类别图标不会跟随当前主题，和提交历史引用标签的配色规则不一致。
- 设计：Tag 图标使用主题 `warning`，远程仓库图标使用主题 `info`；仅替换图标颜色来源，保留侧栏行高、文字层级、筛选导航和操作菜单。
- 改动范围：`sidebar_tags`、`sidebar_remotes` 的颜色入口和纯函数映射测试；不改变 Tag/Remote 数据、排序、筛选、推送、重命名、删除或地址修改行为。
- 验收条件：颜色映射测试确认 Tag 和 Remote 分别读取调用方传入的 `warning`/`info`；VCS 侧栏 headless 渲染、`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不启动本机 Docker，不重新构建或上传 `v0.4.0` 安装包，不运行 Kubernetes；不把 headless 结果扩大为真实 Windows 原生窗口主题切换验收。
- 实施顺序：先推送本设计确认，再接入当前主题颜色并补测试；验证通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-009：Git 侧栏引用图标使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 实现：Tag 圆点改用当前主题 `warning`，远程仓库 Globe 图标改用当前主题 `info`；侧栏行高、文字层级、引用筛选和操作菜单未改变。
- 测试：在共享 Git 状态颜色映射中新增 Tag/Remote 图标颜色测试；`cargo test --locked -p ramag-tool-vcs --all-targets -- --test-threads=1` 通过，191 项中 186 项通过、5 项性能观察测试按设计忽略；现有历史侧栏 headless 稳定选择器回归通过。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过；Tag/Remote 生产代码不再直接创建固定 HSLA 颜色。
- 证据边界：映射测试和历史侧栏 headless 渲染回归通过；没有把 headless 结果扩大为真实 Windows 原生窗口明暗主题切换验收。
- 未完成项：没有启动本机 Docker 或构建 Release 安装包，没有运行 Kubernetes；真实窗口主题切换仍待原生窗口验收环境恢复。
- Git：设计确认提交 `8196a37b` 已先行推送；实现代码和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-010：文件标签来源颜色使用主题语义色（设计确认，2026-09-30）

- 问题证据：工作区文件标签中 Project Files、Commit 和 Compare 的来源圆点仍直接使用固定蓝色、紫色和绿色；主题切换后颜色不会随当前工作区主题变化，也没有和侧栏引用、Git 状态颜色共用语义入口。
- 设计：保留 Changes 标签按暂存、未暂存、未跟踪和冲突显示状态色；Project Files 使用主题 `info`，Commit 使用主题 `accent`，Compare 使用主题 `success`。标签文字、关闭按钮、来源识别和稳定目标 ID 不变。
- 改动范围：只调整文件标签来源圆点的颜色映射、共享颜色辅助函数和回归测试；增加一项可在具备 GPUI 图像渲染器的环境中执行的 VCS 截图测试，验证 1024×768 工作区帧能被捕获并写入 `target/ui-screenshots/`。不改变标签顺序、标签关闭、Diff 读取或 Git 操作。
- 验收条件：来源颜色测试确认 Project Files、Commit、Compare 分别读取传入主题的 `info`/`accent`/`success`；现有 `ramag-tool-vcs` headless 布局和交互测试继续通过；具备 GPUI 图像渲染器时截图测试确认画布尺寸并生成 PNG；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。Linux WSL 当前没有 GPUI headless 图像渲染器时，只记录结构化 headless 证据和截图测试未执行原因，不把它写成像素截图已通过。
- 不做事项：不重新构建或上传 `v0.4.0` 安装包，不启动本机 Docker，不运行 Kubernetes；不改变 Diff 增删行底色、Git 状态映射、引用解析、主题调色板或真实 Windows 原生窗口操作。
- 实施顺序：先推送本设计确认，再替换文件标签来源颜色并补截图测试；目标测试和质量检查通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-010：文件标签来源颜色使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 实现：文件标签中的 Project Files、Commit 和 Compare 来源圆点分别读取当前主题的 `info`、`accent` 和 `success`；Changes 标签继续按 Git 状态显示颜色。文件标签栏增加 `vcs-ftab-bar` 稳定调试选择器，方便布局和截图测试定位。
- 测试：新增 `file_tab_sources_use_theme_semantic_colors`，确认三类来源和未跟踪状态分别使用主题颜色或弱化正文色；新增 `vcs_file_tab_sources_fit_supported_window_sizes`，在 `360x640`、`1024x768` 和 `1440x900` 检查标签栏边界。`ramag-tool-vcs` 全量测试 `188` 项通过、`5` 项性能观察测试按设计忽略。
- 截图测试：新增 `captures_vcs_file_tab_screenshot`。在 macOS Metal 主线程视觉运行器中，它会捕获 `1024x768` VCS 工作区帧，检查尺寸和非空像素，并写入 `target/ui-screenshots/vcs-file-tabs-1024x768.png`。当前 WSL/Linux 的 GPUI 测试平台没有 headless 图像渲染器，因此本次只执行了三尺寸 headless 布局回归，没有生成像素 PNG；Computer Use 原生窗口运行时也未恢复，不把这两项限制写成截图已通过。
- 质量检查：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过；未重新构建或上传 `v0.4.0` 安装包，未启动 Docker，未运行 Kubernetes。
- Git：设计确认提交 `4f086e8c` 已先行推送；实现代码、回归测试和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-011：SSH 环境标签使用主题语义色（设计确认，2026-09-30）

- 问题证据：SSH 连接管理器的 `dev`、`test`、`prod` 环境标签直接创建固定绿色、橙色和红色；切换浅色/深色主题后，标签文字颜色不会跟随当前主题，和连接列表、生产标记的状态色规则不一致。
- 设计：`dev` 使用当前主题 `success`，`test` 使用 `warning`，`prod` 使用 `danger`；未识别或空白环境继续使用调用方传入的 `muted_foreground`。标签背景沿用文字颜色并保持 `0.12` 透明度，连接可用性、生产标记、平台标签和操作按钮不变。
- 改动范围：只调整 SSH 环境标签的颜色入口、主题颜色传递和回归测试；增加一项可在具备 GPUI 图像渲染器的环境中执行的 SSH 管理器截图测试，验证 `1024x768` 工作区帧能被捕获并写入 `target/ui-screenshots/`。不改变连接筛选、打开终端、远程桌面、删除连接或 SFTP/端口转发行为。
- 验收条件：颜色映射测试确认三种已知环境分别读取调用方传入的 `success`/`warning`/`danger`，未知环境回退到传入的弱化正文色；SSH 管理器 headless 布局在 `360x640`、`1024x768` 和 `1440x900` 中保持搜索栏、连接行和操作区在窗口内；具备 GPUI 图像渲染器时截图测试确认画布尺寸并生成 PNG；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。Linux WSL 当前没有 GPUI headless 图像渲染器时，只记录结构化 headless 证据和截图测试未执行原因，不把它写成像素截图已通过。
- 不做事项：不代替用户执行 `10.17.17.114` 的 SSH 终端和端口转发集成回放，不启动本机 Docker，不重新构建或上传 `v0.4.0` 安装包，不运行 Kubernetes；不把 headless 结果扩大为真实 Windows 原生窗口主题切换验收。
- 实施顺序：先推送本设计确认，再接入当前主题颜色、补充 SSH 管理器布局和截图测试；目标测试和质量检查通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-011：SSH 环境标签使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 实现：SSH 连接管理器和工作区标签统一读取当前主题的 `success`、`warning`、`danger`，分别用于 `dev`、`test`、`prod`；未知环境回退到当前主题的 `muted_foreground`，标签背景继续使用文字颜色的 `0.12` 透明度。连接筛选、终端、远程桌面、SFTP 和端口转发行为未改变。
- 测试：新增 `environment_badges_follow_the_active_theme_palette`，覆盖大小写、首尾空白、未知环境和背景透明度；既有 SSH 管理器连接行在 `360x640`、`1024x768` 和 `1440x900` 的边界回归继续通过。`cargo test --locked -p ramag-tool-ssh --lib -- --test-threads=1` 共 `85` 项全部通过。
- 截图测试：新增 `captures_ssh_manager_environment_badges_screenshot`，在 macOS Metal 主线程视觉运行器中捕获 `1024x768` SSH 管理器帧，检查尺寸和非空像素，并写入 `target/ui-screenshots/ssh-manager-environment-badges-1024x768.png`。当前 WSL/Linux 没有 GPUI headless 图像渲染器，本次截图筛选命令实际为 `0` 项，未生成 PNG；只保留代码级截图测试和 headless 布局证据，不把它写成像素截图已通过。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过；未启动本机 Docker，未重新构建或上传 `v0.4.0` 安装包，未运行 Kubernetes。
- 证据边界：测试证明 SSH 环境标签和工作区标签会读取调用方传入的主题颜色，并在三种窗口尺寸中保持连接行边界；没有提供真实 Windows 原生窗口主题切换的鼠标/键盘证据，也没有代替用户执行 `10.17.17.114` 的终端和端口转发集成回放。
- Git：设计确认提交 `4963ca4c` 已先行推送；实现代码、测试和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-012：SSH 工作区加载状态使用主题警示色（设计确认，2026-09-30）

- 问题证据：SSH 工作区标签在终端、SFTP 或文件预览加载时直接创建固定黄色圆点；切换主题后，加载状态不会跟随当前主题的 `warning`，而连接异常和生产状态已经使用主题 `danger`。
- 设计：把工作区标签圆点的优先级集中到纯颜色映射：加载中使用当前主题 `warning`，SFTP 错误或生产连接使用当前主题 `danger`，其它情况继续按 `dev`/`test`/`prod` 环境标签读取 `success`/`warning`/`danger`，未知环境使用 `muted_foreground`。不改变标签文本、关闭操作、终端/SFTP 请求或工作区切换。
- 改动范围：只调整 SSH 工作区标签圆点的颜色入口并补充纯函数回归；不改变连接生命周期、加载状态设置、错误提示、端口转发或远程服务行为。
- 验收条件：颜色映射测试确认加载状态优先于错误和生产状态，错误/生产状态使用主题危险色，环境状态继续读取主题颜色；SSH 工作区现有生命周期和三种窗口 headless 回归继续通过；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。当前 WSL/Linux 没有 GPUI 图像渲染器，不把该切片写成像素截图已通过。
- 不做事项：不代替用户执行 `10.17.17.114` 的 SSH 终端和端口转发集成回放，不启动本机 Docker，不重新构建或上传 `v0.4.0` 安装包，不运行 Kubernetes；不把 headless 结果扩大为真实 Windows 原生窗口主题切换验收。
- 实施顺序：先推送本设计确认，再替换 SSH 工作区加载圆点颜色并补测试；目标测试和质量检查通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-012：SSH 工作区加载状态使用主题警示色（代码与 headless 验收完成，2026-09-30）

- 实现：提取 SSH 工作区标签圆点颜色映射；终端、SFTP 或文件预览加载时使用当前主题 `warning`，SFTP 错误或生产连接使用当前主题 `danger`，其它连接继续按环境主题色或弱化正文色显示。加载状态优先于错误和生产状态，标签文本、关闭操作和工作区切换未改变。
- 测试：新增 `workspace_loading_dot_uses_theme_warning_before_other_states`，覆盖加载优先级、错误状态、生产状态、`test` 环境和未知环境回退；`cargo test --locked -p ramag-tool-ssh --lib -- --test-threads=1` 共 `86` 项全部通过。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过；未启动本机 Docker，未重新构建或上传 `v0.4.0` 安装包，未运行 Kubernetes。
- 证据边界：测试证明工作区标签圆点从调用方主题读取加载、错误、生产和环境状态颜色；没有提供真实 Windows 原生窗口主题切换的鼠标/键盘或像素截图证据，也没有代替用户执行 `10.17.17.114` 的终端和端口转发集成回放。
- Git：设计确认提交 `61f5a254` 已先行推送；实现代码、测试和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-013：对象存储会话加载状态使用主题警示色（设计确认，2026-09-30）

- 问题证据：对象存储工作区标签在账号会话加载时直接创建固定黄色圆点；已配置会话使用主题 `success`、未验证会话使用主题 `warning`，三种状态的颜色入口不一致，切换主题时加载状态不会更新。
- 设计：抽出账号会话状态到圆点颜色的纯映射：`Loading` 使用当前主题 `warning`，`Configured` 使用 `success`，`Unverified` 使用 `warning`，没有状态的会话继续使用 `muted_foreground`。不改变账号标签文本、关闭账号、会话加载、验证或对象列表行为。
- 改动范围：只调整对象存储工作区标签圆点的颜色入口和回归测试；不改变云厂商品牌色、只读标记、账号配置、Bucket 列表或对象读写行为。
- 验收条件：颜色映射测试确认四种状态读取调用方主题颜色；对象存储现有账号行和账号表单 headless 回归继续通过；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。当前 WSL/Linux 没有 GPUI 图像渲染器，不把该切片写成像素截图已通过。
- 不做事项：不启动本机 Docker，不重新构建或上传 `v0.4.0` 安装包，不运行 Kubernetes；不把 headless 结果扩大为真实 Windows 原生窗口主题切换验收。
- 实施顺序：先推送本设计确认，再替换对象存储会话加载圆点颜色并补测试；目标测试和质量检查通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-013：对象存储会话加载状态使用主题警示色（代码与 headless 验收完成，2026-09-30）

- 实现：对象存储工作区标签的账号会话圆点改为读取调用方主题：`Loading` 和 `Unverified` 使用 `warning`，`Configured` 使用 `success`，没有状态的会话使用 `muted_foreground`；账号标签、关闭操作、会话加载和对象列表行为未改变。
- 测试：新增 `account_session_dots_follow_theme_status_colors`，覆盖加载、已配置、未验证和无状态四种映射；对象存储 crate 全量 `26` 项测试通过，包含账号行三种窗口宽度和账号表单紧凑窗口回归。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过；未启动本机 Docker，未重新构建或上传 `v0.4.0` 安装包，未运行 Kubernetes。
- 证据边界：测试证明会话状态圆点读取当前主题状态色，且既有账号行和表单布局保持边界；没有提供真实 Windows 原生窗口主题切换的鼠标/键盘或像素截图证据。
- Git：设计确认提交 `1399d523` 已先行推送；实现代码、测试和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-THEME-014：数据库会话连接中状态使用主题警示色（设计确认，2026-09-30）

- 问题证据：数据库客户端会话标签在首次连接或恢复标签重新建连时，`连接中` 状态直接创建固定黄色圆点和文字颜色；其它状态已经读取当前主题的 `warning`、`danger`、`success` 或弱化正文色。
- 设计：把会话标签状态颜色抽成纯映射；过期会话使用 `warning`，未连接使用 `muted_foreground`，连接中使用当前主题 `warning`，连接失败使用 `danger`，已连接使用 `success`。保留状态文案、生产标记、连接实体懒加载和点击切换逻辑。
- 改动范围：只调整数据库客户端会话标签的连接中颜色入口和回归测试；不改变连接请求、恢复标签物化、失败处理、生产只读保护、驱动品牌色或数据库操作。
- 验收条件：颜色映射测试确认五种会话状态读取调用方主题颜色；数据库客户端现有会话恢复、连接列表和三种窗口 headless 回归继续通过；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。当前 WSL/Linux 没有 GPUI 图像渲染器，不把该切片写成像素截图已通过。
- 不做事项：不启动本机 Docker 数据库，不重新构建或上传 `v0.4.0` 安装包，不运行 Kubernetes；不把 headless 结果扩大为真实 Windows 原生窗口主题切换验收。
- 实施顺序：先推送本设计确认，再替换数据库会话连接中颜色并补测试；目标测试和质量检查通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-014：数据库会话连接中状态使用主题警示色（代码与 headless 验收完成，2026-09-30）

- 实现：数据库客户端会话标签的状态颜色抽成纯映射；过期会话、连接中使用当前主题 `warning`，未连接使用 `muted_foreground`，连接失败使用 `danger`，已连接使用 `success`。状态文案、生产标记、连接实体懒加载和点击切换逻辑未改变。
- 测试：新增 `session_tab_statuses_follow_theme_colors`，覆盖需重连、未连接、连接中、连接失败和已连接五种状态；`cargo test --locked -p ramag-tool-dbclient --lib -- --test-threads=1` 共 `363` 项全部通过。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过；未启动本机 Docker 数据库，未重新构建或上传 `v0.4.0` 安装包，未运行 Kubernetes。
- 证据边界：测试证明数据库会话标签从当前主题读取连接状态颜色，并保留现有恢复、连接列表和三种窗口布局行为；没有提供真实 Windows 原生窗口主题切换的鼠标/键盘或像素截图证据。
- Git：设计确认提交 `5469315d` 已先行推送；实现代码、测试和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-UI-03：数据库客户端 Windows 窗口截图与列筛选验收（替代证据完成，2026-09-30）

- 运行环境：以 WSL workspace 当前 `main` 提交 `1735dff1` 为基准，通过 Windows MSVC 调试构建生成最新 `ramag.exe`；没有重新构建 Release 安装包。
- 截图验收：启动最新调试程序后，系统窗口标题为 `Ramag — 数据库客户端`，窗口内可见 `10.17.17.114` 的 MySQL 会话、绿色 `已连接` 状态、对象导航器、查询工具栏、数据结果标签、结果网格和底部分页状态。系统截图保存为 `target/ui-fallback/dbclient-window-20260930.png`，另保存了 `1024x768` 基线截图 `target/ui-fallback/dbclient-1024x768-cleared-20260930.png`。
- 功能验收：通过 Windows UI Automation 找到“过滤列（逗号分隔多列名）”输入框，写入只读筛选值 `devid`；截图 `target/ui-fallback/dbclient-1024x768-filter-devid-20260930.png` 显示结果网格只保留 `devid varchar` 列，底部状态显示 `命中 1 / 12 列`。清空输入后再次读取控件值为空，筛选状态恢复；本次没有执行写入、事务、DDL 或远程修改。
- 视觉检查：深色主题下连接状态使用绿色，查询工具栏、结果标签、对象树、横向滚动条和分页状态均在窗口内；`1024x768` 截图未发现控件重叠或结果区域被底部状态遮挡。
- 证据边界：本次使用真实 Windows 原生窗口的系统截图和 UI Automation 作为替代证据，确认窗口启动、连接状态展示和列筛选操作可用；Computer Use 当前仍不能接管该窗口，因此没有把本次结果描述为 Computer Use 鼠标/键盘流程，也没有覆盖明暗主题切换、完整多尺寸原生窗口矩阵或 SSH 终端/端口转发回放。
- Git：本记录随本次 UI 验收独立提交并推送 `main`；不改变业务代码、数据库数据或远程服务状态。

### A-QUALITY-VCS-001：历史提交去重索引复用（设计确认，2026-09-30）

- 问题证据：`B-GIT-001-X` 已按完整提交 ID 保护重复回包，但每次分页追加仍从全部已保留提交重新复制 ID 到 `HashSet`。历史缓存达到 100,000 条时，每翻一页都会重复扫描并分配已有 ID，功能正确但不符合大结果集质量目标。
- 设计：在 `VcsView` 中维护与历史提交缓存同步的完整提交 ID 集合；首次替换时一次构建，分页追加时只检查当前页并把真正新增的 ID 加入集合。保留现有 `Rc<Commit>` 和条数/内存上限，切仓或清空查询时同时清空索引；索引只用于本地重复判断，不作为 Git 参数或持久化数据。
- 验收条件：补充带既有 ID 索引的替换/追加测试，确认重复项不增加提交图行、新提交保留首次对象且旧 `Rc` 继续复用；`ramag-tool-vcs` 全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。验收记录说明该切片消除分页追加时对完整历史 ID 的重复构建，不把单元测试耗时当作真实窗口首帧指标。
- 范围：只调整历史缓存的本地 ID 索引生命周期和测试，不改变分页位置、搜索语法、引用/路径过滤、提交详情或 Git 写操作。
- 不做事项：不重新构建 `v0.4.0` 安装包，不运行 Kubernetes，不引入新的 Git 驱动接口；真实 Windows 窗口和 SSH 集成测试仍按既有边界处理。
- 实施顺序：先提交本设计确认，再实现索引复用和目标测试；通过后独立提交并推送，继续按阶段 A 的性能与 headless 质量要求推进。

### A-QUALITY-VCS-001：历史提交去重索引复用（代码与 headless 验收完成，2026-09-30）

- 实现：`VcsView` 维护与历史提交缓存同步的完整提交 ID 集合；首次替换时构建一次，分页追加只检查当前页并加入新增 ID，不再每页复制全部已保留提交 ID。切仓、清空历史和搜索刷新继续通过替换路径同步清空索引；提交图仍复用原有 `Rc<Commit>`。
- 验收结果：新增带既有 ID 索引的追加测试，确认重复提交跳过、旧 `Rc` 保留、新提交只加入一次；`ramag-tool-vcs` 全量 179 项中 174 项通过、5 项性能观察测试按设计忽略；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 质量边界：本切片消除了分页追加时对完整历史 ID 的重复构建，证明范围是缓存处理路径的复杂度和对象复用；没有把单元测试耗时扩大为真实窗口首帧、跨平台发布或远程服务性能结论。
- Git：设计确认提交和代码提交均独立推送 `main`；`v0.4.0` 已发布，本切片没有重新构建或上传安装包。

### A-QUALITY-DBCLIENT-001：恢复连接标签的首帧延迟物化（代码与 headless 验收完成，2026-09-30）

- 问题证据：`DbClientView` 已经只为上次激活的连接创建会话实体，但恢复标签的筛选、去重、激活位置和占位状态都直接写在渲染函数中，缺少独立回归测试；后续调整渲染顺序时容易再次把全部恢复标签提前建连，导致首帧同时读取元数据。
- 设计：把恢复配置转换为会话占位槽的纯状态步骤抽成小函数。该步骤只按保存顺序去重、遵守 `MAX_CONNECTION_SESSIONS`、保留已有标签并计算激活索引；所有新槽的 `entity` 保持为空。渲染函数随后只对激活槽调用 `materialize_slot`，其它标签首次点击时再创建会话。
- 验收条件：测试确认恢复多个连接时只生成一个激活索引、所有恢复槽初始不含会话实体、重复配置不增加标签、超过上限时保留前 `MAX_CONNECTION_SESSIONS` 个并正确回退激活项；数据库客户端全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 范围：只调整恢复占位状态的组织方式和回归测试，不改变连接配置格式、偏好键、会话建立流程、元数据查询、用户点击行为或 `MAX_CONNECTION_SESSIONS` 的上限。
- 不做事项：不重新构建 `v0.4.0` 安装包，不启动 Kubernetes，不新增 Docker 数据库服务，不把 headless 测试耗时写成真实窗口首帧指标；真实窗口流程仍受 Computer Use 环境限制。
- 实施：新增恢复占位槽状态函数，渲染层先完成去重、数量限制和激活位置选择，再只对激活槽调用 `materialize_slot`；新增重复配置、已有标签、上限截断、激活项回退和所有槽未物化测试。
- 验收结果：`cargo test --locked -p ramag-tool-dbclient --lib -- --test-threads=1` 通过，`358` 项全部通过；`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：测试证明恢复状态不会提前创建非激活连接实体，且保持已有标签和 `MAX_CONNECTION_SESSIONS` 限制；没有把 headless 测试结果扩大为真实窗口首帧时间，Computer Use 原生窗口证据仍待环境恢复。
- Git：设计确认提交 `2c0adae2` 已推送；实现提交已通过测试并独立推送 `main`，本切片不重复构建或上传 `v0.4.0` 安装包。

### A-QUALITY-DBCLIENT-002：宽结果列索引按可见范围保留（代码与 headless 验收完成，2026-09-30）

- 问题证据：结果视图最多渲染 `MAX_COLUMNS_DISPLAY` 列，但当前派生视图会把所有命中列的下标先复制到 `matching_col_indices`；宽结果或列筛选命中大量列时，未显示的列仍占用索引内存。行筛选还需要扫描所有命中列，不能简单地把筛选范围一起截断。
- 设计：构建列索引时同时统计命中总数和可见前缀。没有行筛选时只保留前 `MAX_COLUMNS_DISPLAY` 个下标供表头和单元格渲染；启用行筛选时保留全部命中下标，仅用于检查行内容，再把可见列截到同一上限。状态栏的命中总数、列截断提示、列筛选和行筛选语义保持不变。
- 验收条件：宽结果测试确认未启用行筛选时只保留可见列前缀但仍报告完整命中数量；行筛选测试确认位于可见前缀之外的命中列仍能筛中对应行；取消检查、结果缓存、`MAX_ROWS_DISPLAY` 行上限和三种窗口布局回归不受影响。数据库客户端全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 范围：只调整结果视图派生列索引的临时内存和测试，不改变结果数据、列筛选匹配规则、行筛选范围、列渲染上限、排序、分页、编辑和导出行为。
- 不做事项：不改变数据库查询或服务端分页，不新增 Docker 数据库回放，不重新构建 `v0.4.0` 安装包，不运行 Kubernetes；真实窗口首帧和原生拖拽证据仍按既有边界记录。
- 实施：新增列索引收集步骤，未启用行筛选时只保留前 `MAX_COLUMNS_DISPLAY` 个下标；启用行筛选时保留全部命中下标，确保可见列之外的内容仍能参与行筛选；保留完整命中数量和截断状态。
- 验收结果：`ramag-tool-dbclient` 全量 `359` 项测试通过；新增宽结果列索引边界测试，并保留可见列之外的行筛选回归；`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 证据边界：本切片证明无行筛选的宽结果不会为未渲染列保留完整下标，同时不缩小行筛选范围；没有把单元测试或本地性能推断扩大为真实窗口首帧、数据库服务或原生拖拽结论。
- Git：设计确认提交 `f0d241d7` 已推送；实现提交已通过测试并独立推送 `main`，本切片不重复构建或上传 `v0.4.0` 安装包。

### A-QUALITY-TREE-001：最近访问筛选的对象树缓存刷新（代码与 headless 验收完成，2026-09-30）

- 问题证据：对象树行视图按 `tree_revision` 缓存；打开或定位表时会记录最近访问项，但 `record_recent_table` 只保存偏好，没有使行缓存失效或通知界面。在“最近访问”筛选下，首次构建过空结果后打开表，侧栏可能继续显示旧列表，直到其他操作碰巧触发行视图重建。
- 设计：`record_recent_table` 完成去重、置顶和数量限制后，沿用收藏状态更新的处理顺序，清除对象树行缓存并通知当前窗口。只刷新本地行视图，不重复读取数据库元数据，不改变最近访问偏好格式、筛选规则或表打开流程。
- 验收条件：新增 headless 回归，先在“最近访问”筛选下缓存无表结果，再记录当前连接中的表，确认树版本递增且下一次读取立即出现该表；同时保留现有最近访问去重、连接隔离和表树布局测试。数据库客户端全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 范围：只调整最近访问状态变更后的对象树缓存失效与界面通知，并补充对应测试；不改变数据库查询、最近访问持久化、收藏上限、搜索、表列加载或真实窗口交互。
- 不做事项：不重新构建或上传 `v0.4.0` 安装包，不启动 Kubernetes，不新增 Docker 数据库回放；Computer Use 原生窗口证据继续按阶段 A 的既有边界记录。
- 实施：`record_recent_table` 在保存最近访问状态后清除对象树行缓存并通知窗口；新增 `recent_filter_refreshes_after_recording_table` headless 回归，覆盖先缓存空结果、再记录表并立即显示的路径。
- 验收结果：`ramag-tool-dbclient` 全量 `360` 项通过；目标回归、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。未重复构建或上传 `v0.4.0` 安装包。
- 证据边界：测试证明最近访问筛选在状态变化后立即重建本地对象树行视图；没有把 headless 结果扩大为真实窗口鼠标流程、数据库服务或发布构建证据。
- Git：设计确认提交 `43bd100f` 和实现提交 `e7a24430` 已独立推送 `main`，继续阶段 A 对象树质量收口。

### A-QUALITY-THEME-001：对象树错误状态使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 问题证据：对象树加载失败、Schema 加载错误和表大小读取失败仍直接使用 `gpui_kit::red()`；该颜色不随浅色/深色主题的 `Theme::danger` 变化，和应用中其它错误提示的颜色规则不一致。
- 设计：对象树渲染层统一读取当前主题的 `danger`，用于全屏 Schema 错误、Schema 占位错误和表大小失败状态；保留 `warning`、正文色和选中态文字的现有语义。把表大小状态颜色选择抽成纯函数，明确选中态优先使用选中文字颜色，未选中失败态使用传入的主题危险色。
- 验收条件：纯函数测试确认失败、过期、已知、加载中和未知状态分别使用传入的主题语义色，选中态不被状态色覆盖；对象树 headless 布局测试和数据库客户端全量测试通过，workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 范围：只调整 `table_tree` 的错误状态颜色来源和回归测试，不改变对象树状态、数据库请求、错误文案、主题调色板或其它工具的品牌色。
- 不做事项：不重新构建或上传 `v0.4.0` 安装包，不启动 Kubernetes，不启动本机 Docker；Computer Use 原生窗口证据继续按阶段 A 的既有边界记录。
- 实施：对象树全屏错误、Schema 错误占位和表大小失败状态改为读取当前主题的 `danger`；表大小状态颜色映射集中到小函数，选中行仍优先使用选中文字颜色。
- 验收结果：新增主题语义色回归；`ramag-tool-dbclient` 全量 `361` 项通过，workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。未启动本机 Docker，未重复构建或上传 `v0.4.0` 安装包。
- 证据边界：测试证明对象树错误状态使用调用方传入的主题色，且选中态不被失败色覆盖；没有把 headless 结果扩大为真实窗口主题切换或发布构建证据。
- Git：设计确认提交 `ca218dbc` 和实现提交 `ef7c4892` 已独立推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-002：连接表单和连接列表使用主题状态色（代码与 headless 验收完成，2026-09-30）

- 问题证据：连接表单的生产模式开关、连接测试成功/失败提示，以及连接列表的生产标记和 `prod` 环境标记仍使用固定红色或绿色；浅色和深色主题的状态色因此与其它错误、成功和危险操作提示不一致。
- 设计：连接表单读取当前主题的 `danger` 和 `success`，分别用于生产模式和连接测试结果；连接列表的 `prod` 使用 `danger`，`dev` 使用 `success`，`test` 使用 `warning`，数据库类型品牌色保持不变。环境标记继续使用低透明度背景，避免改变现有信息层级。
- 验收条件：状态色纯函数测试确认 `dev`、`test`、`prod` 使用调用方传入的主题语义色，未知环境继续回退到正文弱化色；连接表单和连接列表目标测试、数据库客户端全量测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 范围：只调整连接表单和连接列表的状态颜色来源，并补充颜色映射测试；不改变连接保存、生产模式写操作保护、数据库品牌色、环境字段格式或连接列表交互。
- 不做事项：不重新构建或上传 `v0.4.0` 安装包，不启动 Kubernetes，不启动本机 Docker；Computer Use 原生窗口证据继续按阶段 A 的既有边界记录。
- 实施：连接表单的生产开关和连接测试结果改为读取当前主题的 `danger`/`success`；连接列表的 `dev`、`test`、`prod` 环境标记改为读取 `success`、`warning`、`danger`，数据库驱动品牌色保持不变。
- 验收结果：新增环境标记颜色映射回归；`ramag-tool-dbclient` 全量 `362` 项通过，workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。未启动本机 Docker，未重复构建或上传 `v0.4.0` 安装包。
- 证据边界：测试证明连接状态色来自调用方主题语义色，未知环境仍回退到弱化正文色；没有把 headless 结果扩大为真实窗口主题切换或发布构建证据。
- Git：设计确认提交 `6612a25f` 和实现提交 `6008c283` 已独立推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-003：Redis 工作区状态错误使用主题语义色（设计确认，2026-09-30）

- 问题证据：Redis 工作区的 Key 详情加载失败、TTL/内存估算重试、值编辑和 TTL 编辑失败、通用表单错误、命令行错误输出及生产只读提示仍直接使用 `gpui_kit::red()`；切换浅色/深色主题时，这些状态不会跟随当前主题的危险色。
- 设计：所有 Redis 错误状态和生产只读提示读取当前 `Theme::danger`；Key 详情把危险色传给头部和正文，表单底部从当前 `Context` 读取，命令行转录行把 `LineTone::Error` 映射到调用方传入的危险色。Redis 数据类型标签的固定品牌色、正文色、弱化文字色和操作语义保持不变。
- 改动范围：只调整 Redis 视图的错误/只读颜色来源，补充命令行状态颜色纯函数测试和现有 Key 详情、命令行窄窗口回归；不改变 Redis 请求、错误文案、生产保护、提交状态或数据类型标签。
- 验收条件：生产代码中不再出现 Redis 视图直接调用 `gpui_kit::red()`；错误转录行使用调用方传入的主题危险色，其他转录状态仍使用原有颜色；`ramag-tool-redis` 全量测试、`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不启动本机 Docker，不构建或上传 `v0.4.0` release，不运行 Kubernetes；不把 headless 结果扩大为真实窗口主题切换或原生鼠标/键盘验收，Computer Use 证据继续按阶段 A 的既有边界记录。
- 实施顺序：先提交本设计确认，再替换 Redis 视图颜色来源并补测试；目标测试和质量检查通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-003：Redis 工作区状态错误使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 实现：Key 详情正文、TTL/内存估算重试、值编辑、TTL 编辑和通用表单底部均改为使用当前 `Theme::danger`；命令行生产只读提示和 `LineTone::Error` 也通过当前渲染上下文传入危险色。Redis 数据类型标签颜色保持不变。
- 测试：`ramag-tool-redis` 全量 `113` 项测试通过；新增 `transcript_error_uses_the_current_theme_danger_color`，确认错误转录行读取调用方危险色，普通、弱化和强调行仍保留各自颜色；现有 Key 详情三种窗口布局和命令行窄窗口工具栏回归继续通过。`rg` 检查确认 Redis 生产代码不再直接调用 `gpui_kit::red()`。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过。
- Docker 与发布边界：本切片未启动本机 Docker，未构建或上传 `v0.4.0` release，未运行 Kubernetes；没有把单元测试或 headless 结果扩大为真实窗口主题切换证据。
- Git：设计确认提交 `56fbaea6` 和实现提交 `5520fa95` 已独立推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-004：公共提示和剪贴板设置错误使用主题语义色（设计确认，2026-09-30）

- 问题证据：共享输入对话框的校验错误，以及剪贴板设置页的设置读取异常和全局热键注册失败提示仍直接使用 `gpui_kit::red()`；这两类公共状态在浅色/深色主题下不会跟随当前主题的危险色。
- 设计：增加共享的错误文字颜色入口，统一返回当前 `Theme::danger`；普通/掩码输入对话框、剪贴板设置页的异常提示全部使用该入口。输入校验、设置读取、热键注册和保存逻辑不变。
- 改动范围：只调整 `ramag-ui` 公共提示与剪贴板设置的错误颜色来源，补充主题语义色回归和现有对话框/设置布局测试；不改变错误文案、输入提交、剪贴板采集、热键状态或设置持久化。
- 验收条件：`ramag-ui` 生产代码不再直接调用 `gpui_kit::red()`；公共错误颜色入口始终返回调用方主题的 `danger`，浅色/深色主题切换后仍使用当前值；目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 不做事项：不启动本机 Docker，不构建或上传 `v0.4.0` release，不运行 Kubernetes；不把 headless 对话框或设置测试扩大为真实窗口主题切换证据。
- 实施顺序：先提交本设计确认，再调整公共颜色入口和调用方并补测试；验证通过后独立提交并推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-THEME-004：公共提示和剪贴板设置错误使用主题语义色（代码与 headless 验收完成，2026-09-30）

- 实现：新增 `ramag-ui::error_text_color` 共享入口返回当前主题 `danger`；普通/掩码输入对话框的校验错误，以及剪贴板设置页的设置读取异常和全局热键失败提示均改用该入口。输入、设置保存和热键状态逻辑未改变。
- 测试：`ramag-ui` 全量 `109` 项测试通过；新增 `shared_error_text_color_follows_the_active_theme`，覆盖浅色→深色→浅色切换并确认错误颜色始终跟随当前主题；现有对话框、设置页三种窗口尺寸回归继续通过。`rg` 检查确认 `ramag-ui` 生产代码不再直接调用 `gpui_kit::red()`。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-source-size.sh` 和 `git diff --check` 通过。
- Docker 与发布边界：本切片未启动本机 Docker，未构建或上传 `v0.4.0` release，未运行 Kubernetes；没有把 headless 对话框或设置测试扩大为真实窗口主题切换证据。
- Git：设计确认提交 `6fa162f0` 和实现提交 `5ae9734a` 已独立推送 `main`，继续阶段 A 主题一致性收口。

### A-QUALITY-ICON-001：结果分页图标资源完整性（2026-09-27）

- 问题证据：数据库结果页使用上游 `IconName::SkipBack` 和 `IconName::SkipForward`，运行时加载 `icons/skip-back.svg`、`icons/skip-forward.svg` 时资源不存在，日志持续出现 `could not find asset at path`，但窗口仍能启动。
- 修复：在 `ramag-ui/assets/icons` 内嵌两个分页 SVG；`RamagAssets` 优先加载本地资源，因此不改动上游图标枚举和分页事件语义。
- 验收结果：资源回归测试通过；`ramag-tool-dbclient` 全量 351 项测试通过；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过；最新 x64 MSVC 可执行程序启动后日志未再出现 `skip-back`、`skip-forward` 或 `could not find asset`。
- 不做事项：不改变第一页/上一页/下一页/最后一页的分页逻辑，不替换其它上游图标，不把启动成功扩大为完整真实窗口交互验收。

### A-DB-RED-02-B：对象树默认焦点（2026-09-27，代码与 Windows 窗口替代验证完成）

- 问题证据：连接初始化会把 `Server Objects` 根节点设置为展开，导致 collations/users 等服务器级对象占据默认视口，Schema/数据库列表被推到后面。
- 设计：连接切换或重置时将 `ServerObjectsState::is_expanded` 设为 `false`，清除旧的当前 Schema，并在元数据首次返回后按驱动规则打开一个默认数据库/Schema，同时按需加载其 tables/views；刷新已有连接时保留用户的展开状态。用户点击 `Server Objects` 后仍可展开，刷新和筛选语义不变。
- 验收条件：对象树状态测试确认连接重置后的根节点收起，默认数据库/Schema 行在新连接中展开；`table_tree`/`ramag-tool-dbclient` 回归、三种窗口布局、fmt、Clippy、源码尺寸和 `git diff --check` 通过；最新 Windows 窗口截图中数据库列表位于 Server Objects 之前。
- 不做事项：不删除 Server Objects、不改变 collations/users 查询、不改变用户已经手动展开后的交互，也不把真实窗口替代截图写成 Computer Use 证据。
- 验收结果：新增默认数据库/Schema 展开与保留用户状态测试；`views::table_tree` 44 项通过；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过；最新 x64 MSVC 可执行程序成功启动。系统窗口截图 `target/ui-fallback/default-schema-open.png` 显示 `ramag_ui_test` 的 tables/views 已展开且 `Server Objects` 默认收起；Win32 鼠标输入将其展开后再收起，分别保存 `server-objects-expanded-latest.png` 和 `server-objects-collapsed-latest.png`。Windows UI Automation 发现 42 个可访问控件并调用“数据结果”页签成功。
- 证据边界：Computer Use 运行时仍只返回浏览器且 `apps: []`，因此本次使用真实 Windows 窗口的系统截图、Win32 输入和 UI Automation 作为替代证据；这不宣称 Computer Use 完整鼠标/键盘流程已经恢复。

### SHELL-001：共享工作区令牌与双区框架（2026-09-26）

- 设计：新增 `ramag-ui::workbench` 共享几何令牌，统一导航器宽度、720px 紧凑断点、工具栏/标签/密集行/状态栏高度；深色主题从 VSCode Dark+ 调整为中性深灰与高亮蓝的 JetBrains 工作区层次。
- 改动范围：主壳层顶部工具栏、数据库会话左侧对象导航器/中央查询区的共享宽度与调试选择器；没有改动数据库查询、连接或事务语义。
- Headless UI：`cargo test --locked -p ramag-ui --lib -- --nocapture`（95 项通过），覆盖 `360x640`、`1024x768`、`1440x900` 的壳层顶部区域；新增 `workbench_shell_header_stays_inside_supported_window_sizes`。
- 数据库回归：`cargo test --locked -p ramag-tool-dbclient --lib -- --nocapture`（324 项通过），包含会话紧凑断点和既有对象树/查询/结果网格边界测试。
- 质量检查：`cargo fmt --all`、`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 在提交前通过。
- Docker：本切片不改变数据库协议或数据行为，未启动 Docker 服务。
- 真实窗口：Computer Use 当前无法发现可操作的 DataGrip/Ramag 原生窗口，因此没有把进程启动或 headless 结果描述为真实窗口验收；真实窗口截图和鼠标/键盘流程仍待环境恢复后补充。
- Git：使用单一功能提交并推送 `main`；下一步进入 `DB-UX-001`。

### DB-RED-01：对象树连接上下文行（2026-09-26）

- 设计：在数据库对象树顶部显示当前连接名称、数据库类型、主机/端口和基于元数据生命周期的连接状态；加载中、已连接、未加载和失败状态共用同一行结构，长名称在窄侧栏中省略。
- 改动范围：`TableTreePanel` 顶部上下文行和 `table_tree` headless 边界测试；没有引入虚假 Server Objects/Virtual views 数据，没有修改查询、事务或驱动协议。
- 验收：`cargo test --locked -p ramag-tool-dbclient --lib table_tree -- --nocapture`（37 项通过）；`table_tree_toolbar_wraps_inside_sidebar_widths` 在 180/280/360/1024/1440px 中检查连接上下文、名称、状态和工具栏边界。
- Docker：本切片只渲染已有连接上下文，不读取新的数据库对象，未启动 Docker 服务。
- 真实窗口：Computer Use 当前无法发现可操作的 Ramag 原生窗口；本切片只提供 headless 证据，真实窗口连接状态和点击流程仍待补充。
- 范围状态：`DB-RED-01` 完成；`DB-RED-02` 已有表树基础但尚未按新矩阵重新验收；`DB-RED-03`/`DB-RED-04` 等待真实驱动对象接口，不以静态分组完成。

### DB-RED-03：Server Objects 真实元数据树（2026-09-26，设计确认）

- 设计：对象树在 schema/table 行之后追加 `Server Objects` 根节点；根节点下显示驱动返回的 `collations` 与 `users` 分组及数量，分组可独立展开/收起，子项显示可复制的真实名称和简短属性。首次连接和刷新自动读取，旧连接的异步结果不得写回新连接。
- 驱动边界：MySQL 读取 `information_schema.COLLATIONS` 与 `mysql.user`；PostgreSQL 读取 `pg_catalog.pg_collation` 与 `pg_catalog.pg_roles`；SQLite、Redis、MongoDB 返回明确的 `NotImplemented`，UI 显示可读失败行，不生成静态假数据。
- 安全边界：查询使用驱动元数据 API 和固定 SQL，不拼接用户输入；沿用元数据条数与内存上限；`mysql.user` 权限不足时保留失败信息，不能把空列表误报为“没有用户”。
- 改动范围：领域元数据实体、Driver/SQL shared 转发、MySQL/PostgreSQL 元数据实现、ConnectionService、对象树状态与行渲染；不修改查询编辑器、事务和用户 SQL 执行语义。
- 验收条件：
  - 单元测试覆盖 MySQL/PostgreSQL 元数据分组映射、资源上限和 Driver 默认不支持行为；
  - headless 对象树覆盖根节点、分组数量、展开/收起、加载中和错误行，且 180/280/360/1024/1440px 不越界；
  - 本机 Docker 使用 PostgreSQL 17+ 与 MySQL 8.4+ 验证真实 collations/users 结果，记录启动、端口、健康和清理状态；
  - Computer Use 可用时完成真实窗口连接、刷新、展开和复制流程；不可用时明确记录替代证据及未覆盖行为。
- 不做事项：本切片不实现用户/角色编辑、权限授予、collation 管理，也不提前实现 `Virtual views`；这些归入后续切片。

### DB-RED-03：验收记录（2026-09-26）

- 实现：新增 `ServerObject`/`ServerObjectGroup` 领域实体和 SQL shared Driver 转发；MySQL 使用 `information_schema.COLLATIONS` 与 `mysql.user`（权限不足时回退当前账号），PostgreSQL 使用 `pg_catalog.pg_collation` 与 `pg_catalog.pg_roles`；SQLite、Redis、MongoDB 保持 `NotImplemented`。
- UI：对象树追加可展开的 `Server Objects`、`collations`、`users` 行；支持分组数量、名称详情、过滤、收起/展开、加载中、刷新失败和二次点击复制；请求代际同时校验连接 ID，旧连接结果不会写回。
- 测试：`cargo test --locked -p ramag-tool-dbclient --lib table_tree -- --nocapture`（39 项通过）；`cargo test --locked -p ramag-domain --lib -- --nocapture`（220 项通过）；`cargo test --locked -p ramag-infra-sql-shared --lib -- --nocapture`（37 项通过）；workspace Clippy、fmt、源码尺寸和 `git diff --check` 通过。
- Docker：本机临时 `mysql:8.4`（`127.0.0.1:13316`，`ramag-db-red03-mysql84`）和 `postgres:17-alpine`（`127.0.0.1:15442`，`ramag-db-red03-postgres17`）启动后健康检查通过；MySQL 独立 `server_objects` 集成测试 1 项通过，PostgreSQL `integration` 过滤测试 1 项通过；测试后执行 `docker rm -f`，临时容器不存在。此前旧 `ramag-db-test` 测试栈和 `mysql:8.0` 已按用户要求清理，不作为本次证据。
- 真实窗口：Computer Use 当前无法发现可操作的 Ramag 原生窗口；本切片只有 headless/UI 行为和真实 Docker 元数据证据，未宣称真实窗口点击完成。
- Git：验证通过后使用单一功能提交并推送 `main`；下一项进入 `DB-RED-04 Virtual views/sessions`。

### DB-RED-04：Virtual views / sessions（2026-09-26，设计确认）

- 设计：对象树在 `Server Objects` 之后显示 `Virtual views 1` 根节点和 `sessions` 子项；虚拟节点使用不同于物理表的图标和只读标识，沿用对象树刷新动作独立重新加载。展开/收起只改变虚拟节点，不改变真实 schema/table 的展开状态。
- 真实行为：MySQL 的 `sessions` 使用 `information_schema.PROCESSLIST`，PostgreSQL 使用 `pg_catalog.pg_stat_activity`；打开子项创建新的只读查询标签并执行对应会话快照查询，不绑定表目标，因此不显示危险编辑入口。SQLite、Redis、MongoDB 返回明确的不支持状态。
- 状态边界：虚拟视图列表和会话查询各自使用连接 ID、元数据代际和请求代际校验；刷新失败保留旧虚拟节点并显示错误；刷新不触发表树重载；空结果显示“无会话”而不是成功数据缺失。
- 改动范围：`VirtualView` 领域实体、Driver/SQL shared 虚拟视图与查询能力、MySQL/PostgreSQL 驱动实现、ConnectionService、对象树状态/行渲染、虚拟视图打开事件和测试；不实现会话终止、用户编辑、物理 View DDL 或权限修改。
- 验收条件：
  - headless 覆盖根节点、只读图标/标识、对象树刷新、展开/收起、错误/空状态和宽度边界；点击 `sessions` 后打开独立查询标签，真实表选择和展开状态保持不变；
  - MySQL 8.4 与 PostgreSQL 17 本机 Docker 真实执行 sessions 查询，验证至少一条当前会话或明确的空结果；不使用 mock 代替数据库证据；
  - `cargo fmt --all -- --check`、workspace Clippy、源码尺寸、目标 crate 测试和 `git diff --check` 全部通过；Computer Use 不可用时如实记录未覆盖的真实窗口行为。
- 不做事项：本切片不终止会话、不修改会话参数、不把 sessions 结果写回数据库、不实现其他 DataGrip Virtual views 类别；后续按新验收区域单独排期。

### DB-RED-04：验收记录（2026-09-26）

- 实现：新增 `VirtualView` 领域实体、Driver/SQL shared 转发和 MySQL/PostgreSQL `sessions` 能力；MySQL 使用 `information_schema.PROCESSLIST`，PostgreSQL 使用 `pg_catalog.pg_stat_activity`，SQLite/Redis/MongoDB 保持明确不支持。
- UI：对象树显示 `Virtual views 1 → sessions`，使用 Network/Eye 图标和“只读会话快照”详情；根节点独立展开/收起，刷新沿用全局对象树刷新；打开 `sessions` 通过独立 `TreeEvent` 创建查询标签，不绑定表目标，因此不进入单元格编辑或 DDL 菜单。
- 状态与安全：连接 ID、元数据代际和请求代际阻止迟到结果写回；刷新失败保留旧节点并显示错误；成功空结果显示“（无会话）”；虚拟视图名称只映射驱动固定 SQL，不拼接树文本。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib table_tree -- --nocapture`（41 项通过），覆盖 Virtual views 根/子项、过滤、错误与空状态，并与既有对象树渲染边界回归一起通过；workspace 全量 `cargo test --locked --workspace` 通过。
- Docker：本机 `mysql:8.4` 容器 `ramag-db-red04-mysql84` 使用 `127.0.0.1:13317 -> 3306`，本机 `postgres:17-alpine` 容器 `ramag-db-red04-postgres17` 使用 `127.0.0.1:15443 -> 5432`；两服务健康检查通过，MySQL/PostgreSQL `virtual_views` 集成测试各 1 项通过，实际返回当前会话列；测试后执行 `docker rm -f`，两个临时容器均不存在。
- 质量：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- 真实窗口：按 Computer Use 初始化检查返回 `apps: []`，没有可操作的 Ramag/DataGrip 原生窗口；因此本切片只记录 headless 和真实 Docker 证据，未宣称真实窗口点击、刷新和查询标签流程完成。
- Git：本记录对应一个独立功能提交并推送 `main`；下一项进入 `DB-RED-05A`，继续按截图中的对象树与查询工作区标准推进。

### DB-RED-05A：Query Console 上下文条（2026-09-26，设计确认）

- 设计：在查询标签栏上方增加稳定的连接/驱动/端点上下文条；Schema 使用已有元数据缓存提供下拉切换，切换沿用现有未提交状态确认和所有标签同步机制。上下文条在窄窗口保持单行省略，不挤压标签滚动区。
- 改动范围：`QueryPanel` 查询上下文渲染、Schema 下拉入口、`Tx: Auto` 文案统一和对应 headless 边界测试；不在本切片实现结果网格分页/排序、DDL 操作或事务语义改造。
- 安全边界：连接名称、主机和 Schema 只作为已验证配置/缓存文本展示；Schema 选择不拼接 SQL；没有连接或没有缓存 Schema 时显示明确占位并禁用下拉。
- 验收条件：
  - headless 在 `360x640`、`1024x768`、`1440x900` 验证上下文条、连接名称、Schema 入口、标签栏和右侧工具动作均在父容器内；长连接名和 Schema 不遮挡按钮；
  - 通过 Schema 下拉选择后，所有 QueryTab 的活动 Schema 同步，原有未提交草稿/结果确认规则保持不变；
  - `cargo fmt --all -- --check`、workspace Clippy、源码尺寸、目标测试和 `git diff --check` 全部通过；本切片不需要新增 Docker 服务；Computer Use 不可用时如实记录真实窗口限制。
- 不做事项：不新增驱动协议、不执行数据库请求、不实现 `WHERE`/`ORDER BY` 网格筛选、不加入 DDL 执行按钮；这些分别归入后续 `DB-RED-05B`、`DB-RED-06`、`DB-RED-07`。

### DB-RED-05A：验收记录（2026-09-26）

- 实现：Query Console 在标签栏上方显示连接名称、驱动、端点和当前 Schema；Schema 下拉复用 `SchemaCache::all_schemas`，选择后调用既有 QueryPanel 上下文切换流程，所有 QueryTab 保持同一活动 Schema。
- UI：上下文条固定 34px 高度，连接文本和端点在窄窗口省略，标签区继续独立水平滚动；事务状态统一显示 `Tx: Auto`、`Tx: Manual`、`Tx: Processing`、`Tx: Error`，与参考图控制条语义一致。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib -- --nocapture`（329 项通过）；新增 `query_context_bar_keeps_connection_and_schema_inside_supported_widths` 覆盖 `360x640`、`1024x768`、`1440x900`，并复跑 QueryPanel 11 项与 QueryTab 56 项专项测试。
- Docker：本切片只展示已缓存连接和 Schema，不新增或访问数据库服务。
- 质量：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 真实窗口：Computer Use 初始化检查返回 `apps: []`，没有可操作的 Ramag/DataGrip 原生窗口；本切片只记录 headless 证据，未宣称真实窗口下拉选择和标签滚动完成。
- Git：验证通过后使用单一功能提交并推送 `main`；下一项为 `DB-RED-05B`（结果网格排序入口）。

### DB-RED-05B：结果网格排序入口（2026-09-26，设计确认）

- 设计：在现有 `WHERE` 条件输入旁增加 `ORDER BY` 下拉入口；菜单从当前结果列生成 ASC/DESC 选项，当前排序与表头箭头保持同一状态，选择后沿用现有服务端分页/本地排序和迟到回包隔离。
- 改动范围：ResultPanel 排序状态设置 API、QueryTab 结果工具栏 `ORDER BY` 菜单、结果工具栏 headless 边界和排序回归测试；不改列头点击规则、分页总数计算和 DDL/事务语义。
- 安全边界：排序只传递结果列序号和固定方向枚举，不能把列名或菜单文本拼接到 SQL；有未提交编辑或 DML 正在执行时沿用现有禁用与提示。
- 验收条件：
  - headless 在 `360x640`、`1024x768`、`1440x900` 验证 `WHERE`、`ORDER BY`、事务组和运行按钮不越界；
  - 选择 ASC/DESC 后表头箭头、服务端排序请求和当前页结果保持一致；选择同一排序项可明确切换方向；
  - `cargo fmt --all -- --check`、workspace Clippy、源码尺寸、目标测试和 `git diff --check` 全部通过；本切片不新增 Docker 服务，Computer Use 限制继续单独记录。
- 不做事项：不在本切片实现多列排序、复杂表达式排序、结果网格分页控件重做或 `WHERE` 语法扩展。

### DB-RED-05B：验收记录（2026-09-26）

- 实现：结果工具栏在 `WHERE` 输入旁增加 `ORDER BY` 下拉入口；从当前结果列生成 ASC/DESC 菜单，当前项与表头排序箭头共享 `ResultPanel` 状态。
- 状态与安全：菜单仅提交列序号和 `SortDir`，不把列名或菜单文本拼接到 SQL；排序沿用服务端分页/本地派生视图、横向滚动位置保存、未提交编辑和 DML 忙碌保护；重复选择当前项不重复发出排序事件。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib --quiet`（329 项通过）；结果工具栏三尺寸边界、服务端排序横向位置回归和显式 DESC 状态断言通过。
- Docker：本切片只复用已有结果列和排序状态，不新增或访问数据库服务。
- 质量：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 真实窗口：Computer Use 初始化检查返回 `apps: []`，没有可操作的 Ramag/DataGrip 原生窗口；本切片只记录 headless 证据，未宣称真实窗口下拉菜单和表头联动完成。
- Git：验证通过后使用单一功能提交并推送 `main`；下一项为 `DB-RED-05C`（结果分页与底部状态栏）。

### DB-RED-05C：结果分页与底部状态栏（2026-09-26，设计确认）

- 设计：对齐参考图底部的范围/总数/翻页条，统一页码、页大小、总行数未知和加载/失败状态；分页控件必须与当前连接、Schema、排序和筛选结果绑定。
- 改动范围：结果网格底部状态栏与既有分页事件、页大小菜单、查询取消/迟到回包状态；不在本切片重做单元格编辑、导出或多列排序。
- 验收条件：`360x640`、`1024x768`、`1440x900` 下状态栏不覆盖最后一行和水平滚动条；翻页、页大小、排序/筛选后范围和总数状态可解释；分页请求失败保留当前页并提供重试；通过目标测试、fmt、Clippy、源码尺寸和 `git diff --check`。

### DB-RED-05C：验收记录（2026-09-26）

- 实现：结果底部状态栏新增紧凑分页组，显示页大小、首/前/后/末页、跳页、当前范围和总数；总数未知且存在下一页时显示 `1-500 of 501+`，总数计算中显示省略状态，不伪造最终总数。
- 状态与安全：所有按钮继续发出既有 `PageRequested`/`PageSizeChanged` 事件；未提交编辑或 DML 执行中禁用分页，首末页只在已知页数范围内可用；失败仍由现有结果错误通知和重试路径处理。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib --quiet`（332 项通过）；结果表专项覆盖 360px、1024px 和 1440px 相关布局边界，并验证范围文本、状态栏、水平/垂直滚动条和分页控制组不越界。
- Docker：本切片只复用已有分页状态，不新增或访问数据库服务。
- 质量：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 真实窗口：Computer Use 初始化检查返回 `apps: []`，没有可操作的 Ramag/DataGrip 原生窗口；本切片只记录 headless 证据，未宣称真实窗口分页点击完成。
- Git：验证通过后使用单一功能提交并推送 `main`；下一项为 `DB-RED-05D`（结果单元格查看与复制）。

### DB-RED-05D：结果单元格查看与复制（2026-09-26，设计确认）

- 设计：对齐截图中长文本/JSON 单元格的截断展示，补充单元格详情查看、当前值复制和 NULL/空字符串/二进制状态区分；详情只读取当前单元格，不复制整个结果集。
- 改动范围：结果单元格上下文菜单、值查看器和复制反馈；保留现有虚拟行、列宽、排序、筛选、分页与编辑保护，不在本切片扩展导出或多列排序。
- 验收条件：长文本和 JSON 在网格中不撑破列宽，详情窗口在三种窗口尺寸内可滚动；复制只针对选中单元格并显示成功/失败反馈；敏感值不写入日志；通过目标 UI 测试、fmt、Clippy、源码尺寸和 diff 检查。

### DB-RED-05D：验收记录（2026-09-26）

- 实现：结果单元格继续使用有界预览和独立值查看器；查看器显示列名、数据库类型、值状态和大小，提供文本/CSV/JSON/SQL 四种复制格式。网格对 NULL/空字符串使用弱化色，对 JSON/二进制使用强调色，保持列宽和省略规则不变。
- 状态与安全：复制和查看在没有选中单元格或单元格已失效时显示可解释警告；有效复制只读取当前行列，不遍历结果集；查看器对大字段保留字节上限，敏感值不写日志。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib --quiet`（332 项通过）；结果表专项 19 项通过，值查看器覆盖 `360x620`、`1024x620`、`1440x620`，验证内容区、元数据、复制工具栏、横向滚动和关闭动作边界。
- Docker：本切片只复用内存中的结果状态，不新增或访问数据库服务。
- 质量：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 真实窗口：Computer Use 初始化检查返回 `apps: []`，没有可操作的 Ramag/DataGrip 原生窗口；本切片只记录 headless 证据，未宣称真实窗口复制和查看流程完成。
- Git：验证通过后使用单一功能提交并推送 `main`；下一项为 `DB-RED-06`（`Tx: Auto` 事务控件）。

### DB-RED-06：`Tx: Auto` 事务控件（2026-09-26，设计确认）

- 设计：对齐截图工具栏中的 `Tx: Auto` 下拉入口，明确自动提交/手动事务/处理中/错误状态，并把开始、提交、回滚和未提交修改反馈收束到同一会话上下文。
- 改动范围：事务模式菜单、状态文案和按钮可用性；复用现有驱动能力声明、事务事件和保存点，不在本切片改写 SQL 执行或 DML 生成。
- 验收条件：无连接、驱动不支持、查询执行中和存在未提交编辑时控件状态可解释；MySQL 8.4 与 PostgreSQL 17 Docker 验证自动提交、手动开始、提交和回滚；三种窗口尺寸下控件不越界，并通过目标 UI、fmt、Clippy、源码尺寸和 diff 检查。

### DB-RED-06：验收记录（2026-09-26）

- 实现：将截图中的 `Tx: Auto` 文案升级为可发现的事务模式下拉入口，统一显示自动提交、手动事务、处理中和错误状态；菜单提供手动开始、提交和回滚，并沿用现有事务事件、保存点和未提交编辑互斥规则。
- 状态与安全：查询执行、DML 执行、事务控制请求或存在未提交单元格修改时，相关菜单项保持禁用并说明当前状态；自动提交项不在打开手动事务时隐式提交；驱动不支持事务时禁用手动入口。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib --quiet`（332 项通过）；`query_tab` 专项 56 项通过；活动和非活动事务工具栏均在 `360x620`、`1024x620`、`1440x620` 下验证事务模式入口、事务控制区和保存点控件不越界。
- Docker：使用 Docker Desktop `desktop-linux` 上的 `mysql:8.4`（127.0.0.1:13306）和 `postgres:17-alpine`（127.0.0.1:15432）专用服务完成提交/回滚探针，回滚计数为 0、提交计数为 1；同时启动的 `redis:7-alpine`（16379）和 `mongo:8.2`（27018）健康检查通过。验证后通过 Compose `down --volumes --remove-orphans` 删除专用容器、网络和数据卷。
- 质量：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 真实窗口：Computer Use 初始化检查返回 `apps: []`，没有可操作的 Ramag/DataGrip 原生窗口；本切片只记录 headless 证据，未宣称真实窗口下拉点击完成。
- Git：验证通过后使用单一功能提交并推送 `main`；下一项为 `DB-RED-07`（`DDL` 查看/复制入口）。

### DB-RED-07：`DDL` 查看控件（2026-09-26，设计确认）

- 设计：在截图对应的查询结果工具栏加入 `DDL` 入口；仅对对象树打开的单表或视图启用，打开现有只读 DDL 预览，复制和刷新继续由预览对话框提供。
- 改动范围：QueryTab 工具栏、查询标签到连接会话的 DDL 事件链和已有表属性预览复用；不增加 DDL 执行、迁移或自动修改结构的动作。
- 验收条件：无目标表、执行计划、查询/写操作进行中或存在未提交编辑时入口保持禁用并说明原因；有效入口打开目标对象的只读定义，定义内容可滚动、复制和刷新；三种窗口尺寸不越界，并通过目标 UI、fmt、Clippy、源码尺寸和 diff 检查。

### DB-RED-07：验收记录（2026-09-26）

- 实现：查询结果工具栏新增 `sql-ddl` 入口；事件由 QueryTab 转发至 QueryPanel，再由 ConnectionSession 打开现有 `TablePropertiesDialog`。预览明确显示只读定义，保留复制 DDL、刷新 DDL、垂直/水平滚动和错误重试边界；没有执行 DDL 的按钮。
- 状态与安全：没有 pinned 表目标、执行计划可见、查询或 DML 忙、或存在未提交单元格修改时入口禁用；事件只携带 Schema、对象名和视图标志，不把 SQL 文本拼接到工具栏动作中。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib query_tab -- --nocapture`（56 项通过）；`cargo test --locked -p ramag-tool-dbclient --lib table_properties -- --nocapture`（7 项通过）；覆盖 `360x480`、`1024x480`、`1440x480` 的工具栏入口边界，以及 `360x240`、`420x320`、`1024x720` 的 DDL 预览边界。
- Docker：使用 Docker Desktop `desktop-linux` 的 `mysql:8.4`（127.0.0.1:13306）执行 `SHOW CREATE TABLE`，使用 `postgres:17-alpine`（127.0.0.1:15432）执行 `pg_get_viewdef`；同时启动的 `redis:7-alpine`（16379）和 `mongo:8.2`（27018）健康检查通过。探针对象、容器、网络和数据卷已通过 Compose `down --volumes --remove-orphans` 清理。
- 质量：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 真实窗口：Computer Use 初始化检查返回 `apps: []`，没有可操作的 Ramag/DataGrip 原生窗口；本切片只记录 headless 和 Docker 证据，未宣称真实窗口 DDL 点击完成。
- Git：验证通过后使用单一功能提交并推送 `main`；下一项进入 `DB-UX-003`（结果数据网格的稳定表头、双轴滚动和导出边界）。

### DB-UX-003A：结果导出格式和作用域控件（2026-09-26，设计确认）

- 设计：对齐截图结果工具栏中的 `CSV` 下拉入口，支持 `CSV` 与 `JSONL` 两种明确格式；继续只导出用户选中的有效行，避免在大结果集上误导出全部数据。
- 改动范围：结果导出编码器、工具栏格式菜单、文件扩展名和成功/失败反馈；不改分页、排序、筛选、单元格编辑和数据库查询。
- 验收条件：CSV 正确转义逗号、引号、换行、Unicode 和 NULL；格式菜单在 360/1024/1440 宽度下不越界；没有结果或没有有效选中行时入口显示可解释禁用/警告；通过导出专项、dbclient 全量测试、fmt、Clippy、源码尺寸和 diff 检查。

### DB-UX-003A：验收记录（2026-09-26）

- 实现：结果工具栏新增 `CSV` 格式选择入口，菜单同时提供 `JSONL`；格式与文件扩展名一致，导出继续限定为有效选中行，空结果/无选择/重复导出请求均保留解释性通知。
- 编码：CSV 输出表头，NULL 输出为空字段；逗号、双引号、换行和 Unicode 按 CSV 规则转义；写入仍使用同目录临时文件、完整刷新和原子替换，失败不覆盖已有目标文件。
- Headless：`cargo test --locked -p ramag-app --lib --quiet`（227 项通过）；导出专项 10 项通过；`cargo test --locked -p ramag-tool-dbclient --lib --quiet`（332 项通过），覆盖 `360x480`、`1024x480`、`1440x480` 结果工具栏入口和事务控件换行边界。
- Docker：本切片只处理本地结果集编码，不访问数据库服务；未新增 Docker 集成范围。
- 质量：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 真实窗口：Computer Use 未启动原生窗口；本切片只记录 headless 和纯函数导出证据，未宣称真实文件对话框流程完成。
- Git：验证通过后使用单一功能提交并推送 `main`；下一项为 `DB-UX-003B`（稳定表头与双轴滚动加固）。

### DB-UX-003B：稳定表头与双轴滚动加固（2026-09-26，设计确认）

- 设计：为结果网格建立独立表头验收锚点，确保表头随列横向移动但不随结果行纵向滚动；继续使用轴锁定滚轮、独立水平/垂直滚动条和底部状态栏布局。
- 改动范围：结果表头稳定渲染选择器和纵向滚动回归；不改分页、排序、过滤、导出和单元格编辑语义。
- 验收条件：横向滚轮不移动行，纵向滚轮不移动表头；滚动条仍不覆盖状态栏；通过结果表专项、dbclient 全量测试、fmt、Clippy、源码尺寸和 diff 检查。

### DB-UX-003B：验收记录（2026-09-26）

- 实现：结果表头新增独立 `result-header` 渲染锚点；纵向滚动回归现在明确比较滚动前后表头位置，同时保留横向轴锁定和滚动条设置开关验证。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib result_table::render_test::result_scroll_horizontal_gesture_does_not_move_rows_vertically -- --nocapture` 与 `cargo test --locked -p ramag-tool-dbclient --lib result_table::header_test::result_header_stays_fixed_during_vertical_scroll -- --nocapture`（各 1 项通过）；`cargo test --locked -p ramag-tool-dbclient --lib --quiet`（333 项通过）。
- Docker：本切片只验证内存结果网格和 GPUI 滚动，不访问数据库服务。
- 质量：`cargo fmt --all -- --check`、workspace Clippy、源码尺寸检查和 `git diff --check` 通过。
- 真实窗口：Computer Use 未启动原生窗口；本切片只记录 headless 滚动证据，未宣称真实窗口拖动滚动条完成。
- Git：验证通过后使用单一功能提交并推送 `main`；下一项为 `DB-UX-003C`（大结果集列布局和滚动边界）。

### DB-UX-003C-1：结果列宽边界和拖拽起点（2026-09-26，设计确认）

- 设计：列宽以结果网格当前实际渲染宽度作为拖拽起点；估算宽度、手动宽度和拖拽结果共用 `60–800px` 范围，避免首拖跳变或异常宽度挤压工作区，同时保留超宽列横向浏览。
- 改动范围：只约束结果列宽状态与表头 resize handle，并增加可检查的列锚点；不改变查询行数预算、服务端分页、筛选/排序语义或结果内存预算。
- 验收条件：真实 GPUI 拖拽按当前列宽增量调整；小于/大于范围的设置分别限制到 `60px`/`800px`；渲染列宽和横向滚动范围一致；已有分页与滚动条窄窗测试继续通过，并通过 dbclient 全量测试、fmt、Clippy、源码尺寸和 diff 检查。

### DB-UX-003C-1：验收记录（2026-09-26）

- 实现：调整手柄只处理自己所属列的拖拽；列宽从当前渲染宽度继续计算；列宽写入状态前统一限制到 `60–800px`，表头锚点可直接检查渲染宽度。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib result_table::header_test::result_column_width_is_clamped_before_layout -- --nocapture`、`cargo test --locked -p ramag-tool-dbclient --lib result_table::render_test::server_sort_keeps_horizontal_scroll_position_across_result_reload -- --nocapture` 各 1 项通过；`cargo test --locked -p ramag-tool-dbclient --lib --quiet`（334 项通过）。测试覆盖拖拽增量、最小/最大宽度、渲染宽度和横向滚动范围。
- Docker：本机 `mysql:8.4` 容器 `ramag-visual-test-mysql84` 绑定 `127.0.0.1:13318 -> 3306`，数据库 `ramag_ui_test`；复用 `scripts/db-test/seed/mysql.sql` 初始化，`bulk_records` 为 100,000 行。数据卷 `ramag-visual-test-mysql84-data` 保留，容器按用户要求持续运行；本切片 UI 状态测试不依赖数据库，已通过本机连接执行 SQL 复核记录数，没有删除容器或卷。
- 质量：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、PowerShell 源码尺寸检查和 `git diff --check` 通过；`cargo build --locked -p ramag-bin` 通过。
- 真实窗口：Ramag 原生窗口曾成功启动和截图，但 Computer Use 输入后连接管理页出现删除确认，随后连接列表显示为空；不能确认旧 `ramag-docker-mysql` 配置及其历史/草稿仍可用，因此停止后续应用设置操作。重置 Computer Use 后再次启动目标程序，`get_window_state` 报告 `foreground window did not report a process id`，刷新后的 `list_windows` 和 `list_apps` 均未返回 Ramag 窗口。没有完成原生窗口的连接配置或列宽拖拽验收；本记录只认定 headless GPUI 交互测试通过，不声称真实窗口通过。
- 未完成项：根切片 `DB-UX-003C` 仍在进行；下一子切片为 `DB-UX-003C-2`，验证长表头、大结果集虚拟行末端、分页/滚动条与状态栏边界，并在 Computer Use 可可靠定位窗口后补原生流程证据。

### DB-UX-003C-2：虚拟行末端和分页边界（2026-09-26，设计确认）

- 设计：以结果面板支持的最大 10,000 行构造第二页结果，直接滚动到虚拟列表末行；长列标题和宽列同时存在，检查末行、垂直滚动条、水平滚动条、分页范围和状态栏各自保持在所属可视区域。
- 改动范围：只增加 GPUI 结果网格边界回归，不改变服务端分页、结果内存预算、排序/过滤或查询协议。
- 验收条件：跳到第 10,000 行后该行仍渲染在结果视口内；第 10,001–20,000 行范围和分页按钮留在状态栏内；滚动条不越过状态栏；通过定向测试、dbclient 全量测试、fmt、Clippy、源码尺寸和 diff 检查。

### DB-UX-003C-2：验收记录（2026-09-26）

- 实现：增加虚拟列表末行定位选择器和结果网格回归；覆盖 10,000 行页的第二页末行、长标题、800px 列宽、横纵滚动条与分页状态栏边界，并用混合类型十列结果复现 MySQL `bulk_records` 行形状。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib --quiet`（336 项通过），包含 `final_virtual_row_and_pagination_stay_inside_result_regions` 与 `mixed_bulk_table_rows_render_in_the_result_grid`。
- 真实窗口：Computer Use 使用普通 Cargo 构建的 Windows 程序打开本机 MySQL `bulk_records`；每页 10,000 行时进入第二页，结果行和 `10001-20000 of 100000` 分页范围可见。4 MiB 主线程栈的原生结果页滚动至第二页末端时，末行 `19982-20000` 仍在结果视口，水平滚动条与分页状态栏保持可见，窗口未闪退。
- 本机 Docker：原生窗口读取本机服务 `ramag-visual-test-mysql84`，镜像 `mysql:8.4`，端口 `127.0.0.1:13318 -> 3306/tcp`，数据库 `ramag_ui_test`；验收时容器处于运行状态并按既有要求保留。Headless 测试只构造内存结果，不依赖 Docker。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`scripts/windows/check-source-size.ps1`、`git diff --check` 和 `cargo build --locked -p ramag-bin` 通过。
- 未覆盖：Computer Use 未逐项拖动滚动条手柄；边界几何通过 headless GPUI 断言，真实窗口覆盖点击查询、设置分页、翻到第二页和滚动至末端。

### DB-RED-03-UI-01：Server Objects 名称与说明行错位（2026-09-28，代码与 headless 验收完成）

- 问题证据：Collations 每项把名称和字符集说明堆叠显示，但对象树使用固定高度的 uniform list，说明文本溢出 28px 行并覆盖下一项名称。
- 设计：保留 28px 统一行高，把可选说明改为主名称右侧的次要文字；名称优先占据剩余宽度并省略，说明保持单行并受行容器裁切。Server Objects 与 Virtual views 共用该规则，保留复制和点击语义。
- 改动范围：只调整元数据树的名称/说明行布局、增加几何调试锚点和 GPUI 回归；不改数据库元数据内容、加载流程或复制行为。
- 验收条件：headless GPUI 在 180/280/360px 窗口验证名称和说明横向分离、始终留在本行且不与下一行重叠；Computer Use 在本机 MySQL 8.4 的 Collations 中检查多项名称与字符集说明；通过定向测试、dbclient 全量测试、fmt、Clippy、源码尺寸和 diff 检查。
- 验收结果：`server_object_details_stay_inline_without_overlapping_following_rows` 覆盖 180/280/360px，并同时检查 Server Objects 与 Virtual views 的名称、说明和相邻行边界；`ramag-tool-dbclient` 全量 356 项通过，`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。Computer Use 当前仍无法发现可操作的原生窗口，因此本记录只确认代码与 headless 证据，未宣称真实窗口流程通过。

### DB-RED-03-UI-02：结果网格垂直滚动条随横向内容偏移（2026-09-28，代码与 headless 验收完成）

- 问题证据：本机 MySQL `bulk_records` 结果集的垂直滚动条要等横向滚动到最右端才进入可视区域，横向位置在起点时右侧没有滑块。
- 原因：垂直 `Scrollbar` 默认使用宽结果列表的 `UniformListScrollHandle` 视口边界定位滑块；列表随横向内容变宽，滚动条横坐标也被定位到完整内容的最右端。
- 设计：垂直滚动条使用固定在结果视口右侧的布局边界作为绘制视口，并从 34px 表头下方开始；保持滚动内容尺寸仍来自虚拟列表句柄，不改变横向滚动、分页或滚轮分轴行为。
- 验收条件：headless GPUI 确认固定滚动条视口与数据行视口左右对齐、起始位置和横向末端不变，长结果页保留正向垂直滚动范围；系统截图在 Computer Use 不可用时验证本机 MySQL 8.4 多行结果的右侧滑块在横向起点可见；通过 dbclient 定向测试、fmt、Clippy、源码尺寸和 diff 检查。
- 验收结果：`final_virtual_row_and_pagination_stay_inside_result_regions` 覆盖 10,000 行第二页末端、长表头、800px 列宽、横向起点与末端，并验证垂直滚动条固定在结果视口右侧且可响应点击；`ramag-tool-dbclient` 全量 356 项通过，`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。Computer Use 当前仍不可用，本记录不把 headless 或系统截图描述为真实窗口验收。

### DB-UX-004A：撤销选中的未提交单元格修改（2026-09-26，已完成）

- 问题证据：结果状态栏当前只能撤销全部未提交修改，无法保留其他单元格草稿并单独放弃当前选中项；这缺少 DataGrip 数据编辑器的 `Revert Selected` 核心操作。
- 设计：增加选中草稿判断和单项移除操作；按钮只修改本地 `pending_cell_edits`，不访问数据库、不改变已加载结果或事务状态。没有选中草稿时按钮禁用，撤销全部和提交修改保持原语义。
- 改动范围：ResultPanel 草稿状态 API、结果状态栏操作区、headless GPUI 交互/响应式回归；不改 DML 生成、行定位、提交、回滚或驱动协议。
- 验收条件：在 280/360/1024px 窗口操作区和按钮不越界；两个草稿中选中一个并撤销后仅剩另一个；没有选中草稿时单项撤销入口不可用；通过 dbclient 定向测试、全量测试、fmt、Clippy、源码尺寸和 diff 检查。
- 验收结果：定向测试和 dbclient 全量 338 项测试通过；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`scripts/windows/check-source-size.ps1`、`git diff --check` 均通过。未宣称真实窗口验收，下一项为 `DB-UX-004B` 的 Docker DML 与事务反馈。

### DB-UX-004B-1：提交前 DML 确认（2026-09-26，已完成）

- 问题证据：结果状态栏“提交修改”当前直接启动异步 DML，用户在 DataGrip 风格工作区中看不到本次将提交的草稿数量、影响行范围和事务模式，也缺少取消确认的统一入口。
- 设计：提交按钮先调用本地摘要 API并打开确认框；确认回调再次调用既有 `commit_pending_cell_edits_async`，由当前状态重新校验连接、定位键、表上下文和 SQL 大小，不保存或复用确认前生成的 SQL。
- 安全规则：取消、Esc、关闭确认框不清理草稿、不改变结果和事务；自动提交模式明确提示执行后落库，手动事务明确提示仍需提交事务；没有草稿时不打开确认框。
- 改动范围：ResultPanel 摘要 API、结果状态栏确认入口、确认层响应式 headless 测试；不改 DML 生成、驱动协议和事务接口。
- 验收条件：确认框覆盖 `280/360/1024px`；取消后草稿数量和内容不变；确认期间变更草稿后仍以最新状态进入安全提交路径；通过 dbclient 定向/全量测试、fmt、Clippy、源码尺寸和 diff 检查。
- 验收结果：定向确认框测试和 dbclient 全量 338 项通过；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`scripts/windows/check-source-size.ps1`、`git diff --check` 均通过。真实 Docker DML 和原生窗口证据未在本切片宣称完成，下一项为 `DB-UX-004B-2`。

### DB-UX-004B-2：结果编辑器 DML 与事务真实回读（2026-09-26，设计确认）

- 问题证据：现有 Docker 事务测试主要覆盖驱动级 INSERT/事务接口，尚未证明结果编辑器按主键生成的 UPDATE 在 MySQL/PostgreSQL 两种方言中影响行数正确，并能由独立连接观察提交或回滚结果。
- 设计：新增两个后端集成测试文件，分别创建临时主键表并执行自动提交 UPDATE、手动事务 UPDATE、事务内读取、回滚后外部读取和提交后外部读取；MySQL 保留 `LIMIT 1`，PostgreSQL 使用方言兼容 UPDATE。
- 测试环境：只使用本机 Docker `mysql:8.4`（`127.0.0.1:13306`）和 `postgres:17-alpine`（`127.0.0.1:15432`），测试账号来自 `.ramag/db-test.env`；不使用远程集群或 mock。
- 改动范围：`crates/ramag-infra-mysql/tests/result_editor_dml.rs`、`crates/ramag-infra-postgres/tests/result_editor_dml.rs` 及验收记录；不改 UI、驱动接口或共享种子数据。
- 验收条件：两个 Docker 测试均通过，临时表清理完成；目标 crate 测试、fmt、Clippy、源码尺寸和 diff 检查通过。
- 验收结果：MySQL 8.4 与 PostgreSQL 17-alpine 的 `result_editor_update_round_trip_uses_*_dml_and_transactions` 均通过；`cargo test --locked -p ramag-infra-mysql --all-targets --quiet` 与 PostgreSQL 对应命令全通过；workspace Clippy、fmt、源码尺寸和 `git diff --check` 通过。Docker 容器和专用卷保持运行，临时测试表已清理；未宣称连接失效或原生窗口验收，下一项为 `DB-UX-004B-3`。

### DB-UX-004B-3A：DML 连接失败状态与重试可见性（2026-09-26，已完成）

- 问题证据：DML 失败目前主要通过全局 Toast 告知用户；结果表仍保留草稿，但状态栏没有稳定的失败锚点，窄窗口下也无法快速确认是否可以重试。
- 设计：ResultPanel 保存当前 DML 失败摘要；结果状态栏显示有界错误文本，保留既有“提交修改”入口作为重试动作。开始新提交、提交成功、撤销全部草稿或载入新结果时清理摘要。
- 安全规则：错误摘要只显示驱动返回的有界提示，不把密码或完整连接配置写入 UI；失败路径不清理本地草稿，不伪造提交成功。
- 改动范围：ResultPanel 状态字段、结果表状态栏锚点和 `280/360/1024px` headless 回归；不改驱动协议、DML 生成和事务生命周期。
- 验收条件：模拟 DML 失败后错误锚点不越界、草稿和重试入口保留；成功/撤销/新结果清除错误；目标测试、fmt、Clippy、源码尺寸和 diff 检查通过。Computer Use 原生窗口当前不可操作，必须单独记录限制。
- 验收结果：`selected_pending_edit_reverts_without_touching_other_drafts` 扩展回归和 dbclient 全量 338 项通过；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`scripts/windows/check-source-size.ps1`、`git diff --check` 均通过。Computer Use `cua.getState()` 未发现原生应用，虽检测到 `Ramag — 数据库客户端` 进程窗口，也未执行真实交互；下一项为 `DB-UX-004B-3B`。

### DB-UX-004B-3B：所有结果编辑 DML 的失效连接反馈（2026-09-26，已完成代码与 headless 验证）

- 设计：单元格 UPDATE、行内 DELETE、批量 DELETE 和 INSERT 的数据库失败统一写入结果面板的持久错误状态；状态栏继续显示有界摘要，当前操作入口保持可用，用户恢复连接后可从同一入口重试。
- 验收结果：四类 DML 失败保留错误和草稿；MySQL 8.4（`127.0.0.1:13306`）与 PostgreSQL 17（`127.0.0.1:15432`）失效连接测试通过；`ramag-tool-dbclient` 全量测试、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。Computer Use 当前没有可操作的原生窗口，未宣称真实窗口验收。
- 未完成项：原生窗口鼠标/键盘流程待 Computer Use 恢复后补验；下一项进入 `DB-UX-005` 迁移差异切片。

### DB-UX-005A：EXPLAIN ANALYZE 执行风险确认（2026-09-26，已完成代码与 headless 验证）

- 问题证据：当前 `EXPLAIN ANALYZE` 只有在目标语句本身包含 DELETE、UPDATE、DROP 或 TRUNCATE 时才命中高危检测；普通 SELECT 的 `EXPLAIN ANALYZE` 仍可直接执行，未向用户说明它会运行目标查询。
- 设计：SQL 风险检测识别 MySQL/PostgreSQL 的 `EXPLAIN ANALYZE` 选项，无论目标语句是否写入都返回“会执行目标查询”的确认提示；继续沿用现有连接、Schema、编辑器和执行代次校验，用户确认后才提交计划请求。普通 `EXPLAIN` 保持只读直通，结构化/原始结果视图不变。
- 改动范围：`query_tab` 风险检测纯函数、执行前确认分支及 headless 回归；不改数据库驱动协议、计划解析器、结果网格或 EXPLAIN SQL 生成规则。
- 验收条件：MySQL/PostgreSQL 的 `EXPLAIN ANALYZE SELECT` 均返回有界风险摘要；带危险 DML 的现有风险仍保留；普通 `EXPLAIN SELECT` 不弹确认；检测跳过字符串、注释、子查询中的同名文本；目标 crate 测试、fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 验收结果：`ramag-tool-dbclient` 定向 SQL 风险测试 32 项、全量测试 340 项通过；`EXPLAIN ANALYZE SELECT` 和 PostgreSQL 选项形式均要求确认，普通 `EXPLAIN` 和查询正文中的 `ANALYZE` 不误报；`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。该切片没有新增布局或真实窗口流程，沿用现有 headless 证据，Computer Use 限制不影响本次纯风险检测验收。

### DB-UX-005B：迁移执行后的目标结构回读校验（2026-09-26，已完成代码与 headless 验证）

- 问题证据：迁移执行成功后当前流程只重新加载源表和目标表元数据，没有根据回读结果明确告诉用户目标结构是否已经一致；MySQL DDL 部分执行或元数据加载失败时，用户只能重新查看差异判断结果。
- 设计：迁移执行成功后标记一次回读任务；刷新源/目标列、索引和外键元数据后，使用现有结构差异计算判断结果。无警告且没有新增/删除差异时显示“回读一致”；任一侧元数据不完整时显示回读不完整和原因；仍有差异时显示差异数量并保留结构差异面板，禁止宣称迁移完成。
- 改动范围：`SchemaDiffDialog` 回读状态、迁移成功后的刷新回调和有界通知；不改变迁移 SQL 生成、执行审批、驱动协议或差异算法。
- 验收条件：成功回读、回读警告和回读后仍有差异分别有测试；迁移执行失败不显示一致；回读使用最新请求代次，旧异步结果不能覆盖新连接或新表上下文；目标 crate 测试、headless 对话框测试、fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 验收结果：回读一致、回读不完整和仍有差异三种状态测试通过；`schema_diff_dialog` 定向测试 12 项、`ramag-tool-dbclient` 全量测试 343 项通过；`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。该切片未改变数据库执行协议，真实窗口证据仍按现有 Computer Use 限制记录。

### DB-UX-005C：无稳定键的结果差异安全边界（2026-09-26，已完成代码与 headless 验证）

- 问题证据：两侧结果没有共同主键或非空唯一键时，旧实现会按共有列内容匹配相同的行；内容相同不能证明两行来自同一条数据库记录。
- 设计：只有两侧都提供且值可用的主键或非空唯一键时才生成行级匹配和单元格差异；没有共同稳定键时，所有已加载源行按整行删除、目标行按整行新增展示，不按内容或位置猜测对应关系。
- 改动范围：结果差异匹配模式、统计和提示；不改变稳定键来源、数据库查询、结果快照、分页和导出上限。
- 验收结果：无稳定键即使存在相同内容也不产生未变化或单元格差异，稳定键路径继续保留单元格定位；`result_diff` 定向测试 8 项、`ramag-tool-dbclient` 全量测试 343 项通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口证据沿用 Computer Use 当前不可用的限制。

### DB-UX-005D：迁移脚本指纹复核（2026-09-26，已完成代码与 headless 验证）

- 问题证据：审批历史保存了脚本 SHA-256，但当前迁移预览和确认说明不显示本次指纹，复制或保存后的脚本缺少直接复核线索。
- 设计：当前完整迁移脚本生成稳定 SHA-256；预览显示完整指纹，复制文本和执行确认使用同一指纹。指纹只作为复核标识，不保存 SQL 正文，也不改变执行语义。
- 验收条件：指纹稳定且对脚本变化敏感；预览、复制文本和确认上下文一致；长指纹在支持窗口内不溢出；目标 crate 测试、fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 验收结果：预览显示完整 SHA-256，复制文本和执行确认复用同一指纹函数；`schema_diff_dialog` 定向测试 13 项、`ramag-tool-dbclient` 全量测试 344 项通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口证据沿用当前 Computer Use 限制。

### DB-UX-005E：唯一重命名候选迁移（2026-09-26，已完成代码与 headless 验证）

- 问题证据：结构差异能标记唯一列重命名候选，但迁移生成器当前仍生成删除旧列和新增新列，可能丢失原列数据。
- 设计：仅对未按名称匹配且两侧唯一、完整列定义一致（包含主键属性和双方已知且相同的列序号）的候选生成重命名；位置缺失、候选歧义或定义变化继续保守处理。MySQL 使用 `CHANGE COLUMN`，PostgreSQL/SQLite 使用 `RENAME COLUMN`。
- 验收条件：唯一候选不生成 DROP/ADD；歧义和定义变化不自动重命名；方言单元测试、全量测试及质量检查通过。
- 验收结果：MySQL/PostgreSQL/SQLite 唯一候选均生成对应方言的重命名 SQL；歧义、定义变化继续生成删除/新增；迁移定向测试 19 项、dbclient 全量测试 349 项通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口证据沿用 Computer Use 当前限制。

### DB-UX-005F：迁移阶段逐段复核（2026-09-26，已完成代码与 headless 验证）

- 问题证据：迁移预览显示阶段计数，但复制脚本和执行确认只显示总语句数，无法快速核对每个阶段的删除或修改风险。
- 设计：把现有阶段顺序和破坏性计数格式化为有界摘要，附加到复制文本和执行确认；不引入逐阶段执行或新的数据库协议。
- 验收条件：预览、复制文本和确认说明中的阶段顺序与计数一致；空阶段不输出；目标 crate 测试、fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 验收结果：复制文本和执行确认复用同一阶段摘要，阶段顺序、语句数和破坏性计数一致；`schema_diff_dialog` 定向测试 14 项、`ramag-tool-dbclient` 全量测试 351 项通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口证据沿用 Computer Use 当前限制。

### PLAT-004-A：多入口插件清单与原生工具注册底座（已完成代码与 headless 验证）

- 目标：扩展静态插件清单，使一个插件可以声明多个稳定入口；每个入口声明名称、说明、输入/输出数据类型和有界载荷上限。现有 `entry_id` 保留为单入口兼容字段，并且必须对应清单中的一个入口。
- 注册规则：`StaticPlugin` 提供多个 `Tool` 时，`ToolRegistry` 一次性校验并注册整组入口；入口数量、ID、工具元数据必须一一对应，任一项失败都不得修改现有注册表。按插件 ID 的生命周期、设置和权限仍只维护一份。
- 边界：本切片只实现领域描述、清单校验和原子注册，不创建 GPUI 视图、不启动后台任务、不增加动态插件加载，也不引入 WebView。统一 GPUI 入口渲染、按需激活和任务预算分别归入 `PLAT-004-B` 与 `PLAT-005`。
- 验收：单入口旧插件行为不变；双入口插件可同时列出、按插件 ID 一起卸载；重复入口、入口数量不匹配、非法输入/输出规格和部分注册失败均有测试；通过 domain/app 定向测试、workspace Clippy、格式、源码大小和 `git diff --check`。
- 验收结果：`ramag-domain` 全量 222 项、`ramag-app` 全量 259 项、插件诊断 headless 1 项和 `ramag-bin` 启动前置测试 1 项通过；单入口兼容、多入口原子注册、按插件整体卸载、数据规格上限和诊断入口数量均有覆盖。workspace Clippy、格式、源码大小和 `git diff --check` 通过；未创建视图、后台任务或 WebView。

### PLAT-004-B：统一 GPUI 入口渲染（已完成代码与 headless 验证）

- 设计：宿主为每个已注册入口预置标准原生 GPUI 元数据面板；专用工作台通过 `register_tool_view` 覆盖标准面板，未覆盖入口仍可直接导航。面板显示入口名称、插件/入口 ID、输入输出数据类型和有界载荷上限。
- 边界：标准面板只展示清单边界并明确“执行适配尚未注册”，不伪造执行结果，不创建 WebView，不启动后台任务；输入提交、取消和结果预算进入 `PLAT-005`。
- 验收结果：标准入口在 `360x520`、`1024x640`、`1440x900` headless 窗口内保持边界；`ramag-ui` 全量 97 项、`ramag-bin` 启动前置测试 1 项、workspace Clippy、格式、源码大小和 `git diff --check` 通过。专用工作台覆盖标准面板的兼容路径继续保留。

### PLAT-005-A：标准入口按需激活（已完成代码与 headless 验证）

- 问题证据：如果宿主启动时为全部插件入口创建 GPUI 实体，未打开的入口也会占用视图对象和布局资源，违背本机优先的按需激活边界。
- 设计：`Shell` 启动时只保存入口描述；用户导航到入口时才创建标准 `StandardPluginEntryView`。专用工作台视图仍由 `register_tool_view` 在窗口装配阶段覆盖，不改变现有导航和恢复逻辑。
- 验收结果：未激活入口没有标准视图调试节点，激活后才创建并显示；`ramag-ui` 全量 98 项（含 `360/1024/1440` 入口面板和延迟激活测试）、`ramag-bin` 启动前置测试 1 项、workspace Clippy、格式、源码大小和 `git diff --check` 通过。后台任务取消和结果预算仍归入后续切片。

### PLAT-005-B：入口任务生命周期绑定与取消令牌（已完成代码与测试验证）

- 设计：`PluginContext::start_task` 要求显式 `task.scoped` 能力，为每个任务返回带取消标记的句柄；单插件最多保留 8 个活动句柄，任务名称有界且禁止控制字符。句柄丢弃释放名额。
- 生命周期：宿主在初始化失败、关闭回调前和卸载时停止接受新任务并标记全部句柄取消；插件只能观察取消状态，不能在关闭阶段重新创建任务。
- 验收结果：任务数量、名称、能力拒绝、上下文停止和宿主关闭取消均有测试；`ramag-app` 全量 265 项、workspace Clippy、格式、源码大小和 `git diff --check` 通过。实际异步执行器、超时和结果字节预算仍待后续切片。

### PLAT-005-C：任务执行超时与结果预算（已完成代码与测试验证）

- 设计：`PluginTaskHandle::run` 接收一个异步操作和 `PluginTaskBudget`，并行观察任务结果、生命周期取消标记和超时定时器；取消优先结束等待，超过默认允许的 60 秒则返回超时错误。
- 预算：超时时间必须大于 0 且不超过 60 秒，结果上限必须在 1 字节到 1 MiB 之间；任务返回结果超过上限时拒绝结果，插件错误文本截断到 512 字节并保持 UTF-8 边界。
- 验收结果：新增预算校验、成功、取消、超时、结果超限和错误有界测试；具体插件执行器的调度和并发策略仍留给后续接入切片。

### PLAT-005-D：静态插件入口执行器（已完成代码与测试验证）

- 设计：`StaticPlugin::execute` 提供可选的静态入口执行适配；宿主先校验插件状态、入口 ID 和输入字节上限，再创建任务句柄并返回可等待的 `PluginTaskExecution`。任务对象持有句柄直到完成，保证关闭或卸载时仍能收到取消信号。
- 边界：输出预算取调用方预算和入口清单上限的较小值；未实现执行适配的静态插件返回有界诊断。该切片不加载动态代码、不执行外部进程、不引入 WebView。
- 验收结果：覆盖成功执行、未知插件/入口、输入超限、未授权任务、输出超限和宿主关闭取消；下一项进入 `TOOL-MIG-001`，将一个真实纯计算核心接入该入口执行器。

### TOOL-MIG-001-A：JSON Path 纯计算核心（已完成代码与测试验证）

- 设计：从 IT Tools 的 JSON Path 提取器迁移点号、方括号、通配符、负数组索引、JSON5 输入和缩进 JSON 输出；核心放在 `ramag-domain`，不依赖 GPUI，可供桌面入口和未来 Web/WASM 适配复用。
- 边界：路径、输入、匹配数和输出均有上限；缺少匹配返回空文本，非法 JSON/路径和超限输入返回结构化错误。对象通配符保留 JSON 插入顺序，桌面端不加载 WebView。
- 验收结果：覆盖路径分词、JSON5、嵌套查询、通配符、负索引、缺失匹配、插入顺序和超限错误；下一项为 `TOOL-MIG-001-B` 原生 GPUI 入口。

### TOOL-MIG-001-B：JSON Path 原生 GPUI 入口（已完成代码与 headless 验证）

- 设计：新增 `ramag-tool-json-path` 静态插件包，使用原生 GPUI 输入编辑器、路径输入、执行按钮和结果面板；请求通过 `StaticPluginHost::execute_entry` 进入 `ramag-domain` 核心，桌面端不嵌入 WebView。
- 生命周期：插件声明 `task.scoped`，由组合根明确授予；入口任务继承输入、输出、超时和取消边界，宿主关闭时取消正在等待的执行。
- 验收结果：插件注册、宿主执行、JSON Path 结果、输入/输出限制、360/1024/1440 宽度 headless 布局测试均通过；下一项进入 `DUAL-CORE-001` 的 Web/WASM 适配评估。

### DUAL-CORE-001-A：JSON Path Web/WASM 适配（已完成代码与跨目标验证）

- 设计：新增 `ramag-tool-json-path-wasm`，仅暴露 JSON 编码请求/响应的 `wasm_bindgen` 函数；它复用 `ramag-domain::json_path`，不引入 GPUI、凭据、网络或文件系统能力。
- 兼容性：WASM 与桌面使用相同的 JSON5、路径和结果格式；结构化错误上限为 512 字节。桌面组合根不依赖该适配，继续直接调用原生 Rust/GPUI 插件。
- 验收结果：WASM 适配单元测试 2 项通过；`cargo build --locked -p ramag-tool-json-path-wasm --target wasm32-unknown-unknown --release` 通过，产物约 581 KiB。

### CATALOG-001：第一方工具目录（已完成代码与 headless 验证）

- 设计：静态宿主在注册阶段生成第一方目录快照，记录插件 ID、入口 ID、API 版本、桌面/Web 支持、能力、数据处理边界和审核状态；目录登记失败时原子回滚工具注册，不执行任何未信任代码。
- 实现：`ramag-app::PluginCatalog` 提供有界、去重和按插件卸载的目录模型；插件设置页新增“第一方工具目录”区域。静态插件默认标记桌面原生，JSON Path 入口同时标记 Web/WASM；目录与运行时工具列表分离，桌面端继续使用原生 GPUI，不加载 WebView。
- 验收结果：目录原子登记/重复拒绝、宿主多入口登记与卸载、JSON Path 双端平台标记通过；`ramag-ui` 插件诊断在 360/1024/1440 headless 窗口显示目录并保持边界；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。下一项为 `COLLAB-001`。

### COLLAB-001-A：本机优先共享包边界（已完成代码与测试）

- 目标：把“用户明确选择的文档或结果”建模为可验证的共享包，并先落到本机加密存储；本切片不启动网络同步、不扫描远程目录、不自动上传任何内容。
- 数据边界：共享包只接受 `Document` 和 `QueryResultPreview` 两类入口；凭据、连接配置和 JWT/Token 等秘密类型直接拒绝。标记为敏感或原始业务数据的内容可以留在本机加密草稿中，但不能进入手动导出包，除非后续切片增加单独的用户确认策略。
- 状态与冲突：每个包带单调递增 revision、`LocalDraft`/`Shared`/`Revoked` 状态和有界审计事件；更新必须带期望 revision，过期更新只记录冲突审计，不覆盖当前内容。
- 加密与撤销：`Storage` 使用主密钥加密共享包后再落盘；撤销只改变本机包状态并写审计，不删除历史证据，不代表远端已撤回。
- 验收条件：领域校验拒绝越界和敏感导出；本机 redb 重启后可恢复且明文不出现在表值；错误 revision 不覆盖数据并产生冲突记录；撤销后不能再次导出；workspace 测试、fmt、Clippy、源码尺寸和 `git diff --check` 通过。Docker 和真实窗口不属于本切片。
- 验收结果：`ramag-domain`、`ramag-infra-storage` 和 `ramag-app` 的专项测试通过；workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。GPUI 入口、网络传输、自动同步和真实窗口验收留给 `COLLAB-001-B`。

### A-QUALITY-UI-04：数据库会话标签长名称布局（设计确认，2026-09-30）

- 问题证据：数据库会话标签直接渲染完整连接名称，没有标题宽度上限或全文提示；长连接名会挤压数据库类型、连接状态、生产标识和关闭按钮，违反共享 UI 对长文本收缩和操作可见性的要求。
- 设计：按窗口宽度为会话标签标题提供有界最大宽度，标题使用单行省略并通过 tooltip 保留完整连接名称；状态圆点、数据库类型、连接状态、生产标识和关闭按钮继续保持独立布局，不改变标签选择、关闭和横向滚动语义。
- 验收条件：headless GPUI 在 `360x640`、`1024x768` 和 `1440x900` 下验证长名称标题不越出标签标题区域，状态和关闭入口仍参与布局；多标签仍可横向滚动，点击标签和关闭按钮继续定位原会话；目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 范围：只调整 `DbClientView` 会话标签标题布局并补充宽度规则测试，不改变连接配置、会话状态、持久化恢复或数据库请求。
- 不做事项：不重新构建或上传 `v0.4.0` 安装包，不启动本机 Docker，不把 headless 结果扩大为 Computer Use 原生窗口证据。
- 实施顺序：先提交本设计确认，再实现标签标题的响应式宽度和 tooltip；验证通过后使用独立功能提交并推送 `main`，继续阶段 A 的 UI 对齐收口。

### A-QUALITY-UI-04：数据库会话标签长名称布局（代码与 headless 验收完成，2026-09-30）

- 实现：`DbClientView` 会话标签标题按窗口宽度使用 `140/180/240px` 最大宽度，超长名称单行省略；标题保留完整名称 tooltip，状态圆点、驱动类型、连接状态、生产标识和关闭按钮继续独立布局，标签横向滚动和会话定位语义不变。
- 测试：新增实际 headless GPUI 标题渲染测试，覆盖 `360x240`、`1024x240` 和 `1440x240` 的标题边界与响应式宽度规则；`ramag-tool-dbclient` 全量 `365` 项测试通过。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- Docker 与发布边界：本切片只验证标签布局，不访问数据库服务，未启动本机 Docker，未构建或上传 `v0.4.0` 安装包。
- 原生窗口证据：Computer Use `getState()` 只返回空应用清单和浏览器；尝试启动调试程序后进程窗口存在，但当前运行时没有可用的 Windows 应用/窗口绑定 API，无法执行原生鼠标/键盘流程。因此本切片只认 headless GPUI 证据，没有宣称 Computer Use 或系统截图验收通过。
- Git：设计确认提交 `de11aaca` 已推送；实现、测试和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-DBCLIENT-003：会话标签使用稳定连接标识（设计确认，2026-09-30）

- 问题证据：会话标签的选择、关闭和“配置已更新”面板回调捕获渲染时的数组下标；另一个标签先被关闭或异步刷新改变顺序后，旧回调可能把操作应用到相邻连接。边界检查只能避免越界，不能保证仍定位同一个连接。
- 设计：每个标签回调捕获稳定的 `ConnectionId`，执行时重新从当前会话列表解析位置；标签、标题和关闭按钮的调试标识也使用连接 ID，避免列表重排后复用旧节点身份。保留数组下标用于同一同步调用链内部的状态更新，不改变连接恢复、会话关闭或资源清理语义。
- 验收条件：headless 回归验证三种窗口下标签标题、状态和关闭按钮仍在布局内；单元测试验证会话列表重排后按连接 ID 仍定位原会话；目标测试、workspace fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 范围：只调整 `DbClientView` 标签与 stale 面板的回调目标解析、稳定调试标识和对应测试，不改变连接配置、查询、事务或数据库请求。
- 不做事项：不启动本机 Docker，不重新构建或上传 `v0.4.0` 安装包，不把 headless 结果扩大为 Computer Use 原生窗口证据。
- 实施顺序：先提交本设计确认，再替换 UI 回调的索引捕获并补回归；验证通过后使用独立功能提交并推送 `main`。

### A-QUALITY-DBCLIENT-003：会话标签使用稳定连接标识（代码与 headless 验收完成，2026-09-30）

- 实现：会话标签选择、关闭、stale 面板重连和关闭回调改为捕获 `ConnectionId`，执行时解析当前会话位置；标签、标题和关闭按钮的调试标识改用连接 ID，列表重排后不会复用旧下标指向相邻连接。同步内部状态更新仍使用当前索引。
- 测试：新增列表重排后按连接 ID 解析原会话的回归；`ramag-tool-dbclient` 全量 `366` 项通过，包含 `360x240`、`1024x240`、`1440x240` 标签标题 headless 边界测试。
- 质量检查：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、源码尺寸检查和 `git diff --check` 通过。
- Docker 与发布边界：本切片只验证会话标签目标解析和布局，不访问数据库服务，未启动本机 Docker，未构建或上传 `v0.4.0` 安装包。
- 原生窗口证据：按 `computer-use` 技能重新初始化 `@oai/sky` 时返回 `Trusted RPC service is not configured: sky`；当前无法获取 Windows 原生窗口或执行鼠标/键盘流程。因此本切片只认 headless GPUI 和单元测试证据，没有宣称 Computer Use 原生窗口验收通过。
- Git：设计确认提交 `d0f04f23` 已推送；实现、测试和本验收记录随本切片独立提交并推送 `main`。

### A-QUALITY-UI-05：数据库结果工具栏动作组对齐（设计确认，2026-09-30）

- 问题证据：真实系统截图的 `1024x768` 数据库工作区中，结果工具栏因整体 `flex_wrap` 把导出和运行按钮拆到第二行，运行入口与筛选、排序和事务上下文分离，操作发现性和视觉对齐不符合共享工具栏基线。
- 设计：将新增行、删除、导入、导出、取消/运行等尾部操作包进稳定的结果动作组，组内使用紧凑间距并保持按钮顺序；响应式空间不足时整组换行，禁止组内单个按钮被外层工具栏拆开。筛选、排序、事务和 DDL 的业务状态与禁用规则保持不变。
- 验收条件：headless GPUI 在 `360x480`、`1024x480` 和 `1440x480` 检查动作组及其按钮都在结果工具栏内；`1024` 宽度下运行按钮与导出动作同属一组，`360` 宽度下组可整体换行；真实系统截图复核 `1024x768` 和 `1440x900` 的工具栏层次与边界。
- 范围：只调整 QueryTab 结果工具栏布局和对应调试锚点/测试，不改变查询执行、导出、DML、事务或数据库请求。
- 不做事项：不启动本机 Docker，不重新构建或上传 `v0.4.0` 安装包；Computer Use 不可用时，系统截图只作为回退证据，不写成 Computer Use 原生窗口证据。
- 实施顺序：先提交本设计确认，再实现动作组布局与回归测试；验证通过后使用独立功能提交并推送 `main`。

## 5. 分支和清理

默认在最新 `main` 上开发和推送。只有用户明确要求功能分支时才创建分支；分支必须基于最新 `main`，验证后合并回 `main`，重新验证并推送，再确认源分支无未合并/未推送提交和关联 worktree 后清理。不得删除 `main` 或未明确纳入本次合并的分支。
