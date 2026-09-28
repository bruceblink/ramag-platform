# Ramag Platform 最新主线：单机桌面收口与原生工作区迁移

> 状态：现行主线；阶段 A 单机桌面收口与阶段 B 原生工作区迁移并行执行，阶段 C（原生协同画布）后置。现有数据库、插件、IT Tools、本机协作和 Relay B3 切片保留其已验证状态；Relay B4、生产 Relay、动态插件市场和画布实现不进入当前开发队列。Linux 构建依赖与插件注册表回归修复已在 `490e4e18`、`97a108f4` 推送。
> 更新日期：2026-09-27
> 适用范围：所有 GPUI 工具和共享 UI
> 共同验收标准：[`07-ui-acceptance-standard.md`](07-ui-acceptance-standard.md)
> 历史设计、待办与完成记录：[`archive/2026-09-25-pre-datagrip-rebaseline/`](archive/2026-09-25-pre-datagrip-rebaseline/)

## 术语表与命名约定

| 规范中文名 | English / Acronym | 本主线中的职责边界 | 不代表什么 |
|---|---|---|---|
| 主线 | Mainline | 规定所有工具采用统一工作区基线的顺序和依赖 | 不替代领域协议或驱动实现 |
| 设计基线 | Design Baseline | 共享 UI 结构、令牌、交互和证据要求 | 不代表某个工具已经完成全部功能 |
| 平台壳层 | Platform Shell | 窗口分区、标签、工具窗口、主题、命令和持久化布局 | 不包含数据库、Kafka 或 API 业务状态 |
| 领域工作区 | Domain Workbench | 在平台壳层中实现某个工具的对象导航和核心操作 | 不表示所有工具共享相同的数据模型 |
| 验收切片 | Acceptance Slice | 可以独立实现、验证、提交、推送和回滚的最小功能 | 不表示可以把多个产品线混在一次提交中 |
| 历史基线 | Historical Baseline | 归档文档中已经完成或曾经排期的实现与证据 | 不表示自动满足新的视觉验收标准 |
| 原生工具插件 | Native Tool Plugin | 使用 Rust 逻辑和 GPUI 视图接入桌面宿主的插件 | 不代表允许使用 WebView 或外部窗口 |
| 计算核心 | Compute Core | 不依赖 GPUI 的解析、转换、校验或计算模块 | 不代表可以直接访问凭据、数据库或任意文件 |
| 第一方工具目录 | First-party Catalog | 由项目维护、审查并随版本发布的工具清单 | 不代表第三方市场或不受信任代码已经开放 |

## 1. 目标和边界

Ramag 的目标是以 GPUI 的原生性能承载 JetBrains 风格的高密度工作区：用户可以在稳定的左侧对象导航、中央标签工作区和辅助工具窗口之间连续完成“定位对象、编辑内容、执行操作、检查结果、处理错误”的流程。

“复刻 DataGrip”在本项目中的可执行含义是：复刻参考图所体现的核心工作区结构、信息层级、操作发现性、网格工作流和安全反馈；不承诺复制 DataGrip 的全部数据库方言、插件生态或内部实现。

本主线新增以下产品边界：

- 桌面端所有插件界面使用原生 GPUI；WebView、嵌入式浏览器和把 Vue 页面直接装入桌面的方案不进入实现或验收范围。
- IT Tools 只作为功能来源，按用户价值、迁移成本和数据风险逐项迁移；不以一次性重写全部网页工具为目标。
- 轻量工具优先迁移为“标准工具入口 + 计算核心”；数据库、API、Git、SSH 和容器等复杂能力继续使用专用原生工作台。
- Web 端与桌面端共享计算核心和测试样例即可；Web 端可选用 WASM，桌面端直接调用 Rust，不要求每个入口同时支持两个平台。
- 本机优先保存和计算；远程协作只同步用户明确选择的非敏感文档或结果，凭据、连接配置和原始业务数据不自动同步。

## 2. 当前基线（历史，不作为新验收结论）

- 数据库、API、Kafka、SSH/终端、容器和 Git 均已有可运行的工具入口及不同程度的领域功能。
- 数据库已有 SQL/MongoDB/Redis 查询、对象树、结果分页、编辑、事务、执行计划、差异和迁移相关能力；详细实现与测试记录已归档。
- 共享 UI 已有 GPUI Kit、响应式弹窗、工具栏和多种 headless 边界测试，但真实窗口证据并不覆盖全部工具。
- MySQL 8.4+、PostgreSQL 17+ 是当前数据库集成基线；旧版本记录仅作为历史事实保留。

这些内容只能作为实现起点。新功能必须同时满足 [`07-ui-acceptance-standard.md`](07-ui-acceptance-standard.md)，不能因为旧测试通过就跳过截图对应的布局、状态和操作验收。

## 3. 阶段主线

### 阶段 A：单机桌面功能收口（当前主线，与阶段 B 并行推进）

阶段 A 只处理已经存在的桌面功能、验收证据和资源边界，不新增画布或远程 Relay 产品面。

1. `A-DB-005`：已完成 MySQL 8.4 与 PostgreSQL 17 的迁移脚本回放和 PostgreSQL 失败回滚验证；保留脚本指纹、阶段复核、人工确认、回读和破坏性变更保护。
2. `A-DB-RED-02`：已完成 MySQL 8.4/PostgreSQL 17 Docker 元数据和 `table_tree` headless 复验；真实 Windows 流程仍待补。
3. `A-UI-REAL`：真实窗口探测受 Computer Use 环境阻塞，状态保持未完成；不把替代证据写成真实窗口通过。
4. `A-P0C`：Windows 主线已完成系统凭据库、主密钥和插件秘密上下文的真实环境验收；Linux Secret Service、macOS Keychain 和发布环境仍待分别验收，验证失败时保留安全拒绝状态。
5. `A-PLAT-005`：JSON Path 真实静态入口已完成 headless 运行指标、任务回收和 Windows 系统 UI Automation/截图替代验收；当前调试构建已记录进程内存样本，Linux/macOS、发布构建和 Computer Use 证据仍待补。
6. `A-QUALITY`：完成大结果集、对象树、首帧、主题一致性和发布检查，形成可复现测量记录。

阶段 A 的每个子项独立设计、验证、提交和推送；同一时间只有一个子项处于开发中。

### 阶段 B：复杂工具原生工作区迁移（当前主线，与阶段 A 收口并行）

阶段 B 按用户频率和领域风险逐个迁移复杂工具，不共享不适用的数据模型：

1. `B-API-001`：API 工作区的请求编辑、环境、历史、响应和真实 Docker HTTP/gRPC 验收；`B-API-001-A/B/C` 已完成代码、headless 和本机 Docker 验收，`B-API-001-UI` 已完成 Windows 系统 UI Automation 与截图替代证据，Computer Use 原生窗口证据待运行时恢复；下一项进入 `B-KAFKA-001` 设计确认。
2. `B-KAFKA-001`：Kafka 连接、Topic、消息、Schema Registry 和 Broker 状态工作区；`B-KAFKA-001-A/B` 已完成代码、headless、Windows 系统 UI Automation 取消流程替代证据和本机 Docker Kafka 成功连接/Topic/消息回读，Computer Use 原生窗口证据待运行时恢复。
3. `B-SSH-001`：SSH/终端、SFTP、端口转发和连接生命周期工作区；`B-SSH-001-A` 已完成代码、headless 和 `10.17.17.114` 真实 OpenSSH/SFTP 回放，`B-SSH-001-B` 的 114 回放由用户自行验证；当前开发转入 `B-CONTAINER-001`。
4. `B-CONTAINER-001`：容器、镜像、日志、执行和资源状态工作区。
5. `B-GIT-001`：Git 仓库、分支、差异、提交和推送工作区。

每个工具保留自己的连接、取消、错误、权限和数据模型；每个切片完成真实服务或本机 Docker 验收后再进入下一项。

### 阶段 C：原生协同画布（后置）

阶段 C 保留为产品方向，不进入当前队列。计划参考 [Excalidraw](https://github.com/excalidraw/excalidraw) 的无限画布、场景元素、撤销重做、导出和协同体验，在 Rust/GPUI 中重新实现，不使用 WebView。

阶段 C 未来按 `CANVAS-001` 至 `CANVAS-006` 拆分：场景模型、自有 JSON、原生编辑器、本机加密保存、PNG/SVG 导出、本机共享、Web/WASM 核心适配，最后再评估实时 Relay。第一版不承诺 `.excalidraw` 双向兼容，不提前扩展账号、权限、生产持久化和动态插件市场。

## 4. 当前交付队列

同一时间只允许一个切片处于“开发中”。状态以代码、测试和证据为准，不以计划文字推断完成。

| ID | 内容 | 状态 | 依赖 | 必要证据 |
|---|---|---|---|---|
| `SHELL-001` | 共享 JetBrains 工作区壳层和设计令牌 | 已完成（headless；真实窗口待补） | 阶段 A | Headless 三尺寸、可用时 Computer Use、fmt/Clippy |
| `DB-UX-001` | 数据库对象导航器 | 阶段 A 代码、headless 与 Docker 复验完成；真实窗口待补 | `SHELL-001` | 对象树交互、MySQL/PostgreSQL Docker、窗口证据 |
| `DB-UX-002` | 查询控制台和连接上下文 | 功能切片完成（`DB-RED-05A` 至 `DB-RED-07`；真实窗口待补） | `DB-UX-001` | SQL 执行/取消/标签回归、Docker、窗口证据 |
| `DB-UX-003` | 结果数据网格 | 功能切片完成（`DB-UX-003A`、`DB-UX-003B`、`DB-UX-003C-1`、`DB-UX-003C-2`；原生拖拽证据待补） | `DB-UX-002` | 大数据量、双轴滚动、分页/编辑交互、窗口证据 |
| `DB-UX-004` | 安全编辑与事务反馈 | 代码、Docker 与 headless 验证完成（`DB-UX-004B-3B` 原生窗口证据待补） | `DB-UX-003` | 成功/失败/取消/回滚、MySQL/PostgreSQL Docker |
| `DB-UX-005` | 分析、差异和迁移工作流 | 阶段 A 代码与 Docker 回放已完成；真实窗口和残余 UI 证据待补 | `DB-UX-004` | 原始回退、人工确认、回读和窗口证据 |
| `P0-C` | 插件设置与权限检查 | Windows 主线代码与真实 Credential Manager/主密钥/秘密上下文验收完成；Linux Secret Service 与 macOS Keychain 待验收 | `DB-UX-005` | 命名空间隔离、类型/大小校验、迁移恢复、每次调用授权和真实系统凭据链路 |
| `PLAT-004` | 多入口原生插件与标准工具入口 | 已完成代码与 headless 验证（`PLAT-004-A`、`PLAT-004-B`；真实窗口待补） | `DB-UX-005`、`P0-C` | GPUI 标准渲染、入口冲突、长输入和错误边界 |
| `PLAT-005` | 按需激活与插件资源预算 | 阶段 A 进行中（JSON Path 真实入口 headless 指标和任务回收已完成；进程内存与真实窗口证据待补） | `PLAT-004` | 首次打开、取消、内存/结果上限和生命周期回收 |
| `TOOL-MIG-001` | JSON Path 计算核心与原生入口迁移 | 已完成（`TOOL-MIG-001-A`、`TOOL-MIG-001-B`） | `PLAT-004`、`PLAT-005` | Rust 核心、GPUI 入口、共享样例和回归 |
| `DUAL-CORE-001` | 已迁移核心的 Web/WASM 双端适配评估 | 已完成（`DUAL-CORE-001-A`） | `TOOL-MIG-001` | Web 结果一致性、构建体积和桌面无 WebView 证据 |
| `CROSS-UX-001` | API/Kafka/SSH/容器/Git 原生工作区迁移 | 阶段 B 进行中；API、Kafka 和 SSH/SFTP 代码/headless/真实服务证据已完成，容器 Docker 日志历史读取、停止、持续 tail、暂停展示、当前窗口复制、follow 不重复历史日志和当前窗口导出已完成代码、headless、本机 Docker 回放；当前处理容器详情状态与健康信息刷新，Kafka 与 SSH 原生窗口证据待 Computer Use 恢复，SSH 终端/端口转发回放由用户自行验证 | `SHELL-001`、`DB-UX-005` | 各工具专项交互与真实服务证据 |
| `CATALOG-001` | 第一方工具目录和能力说明 | 已完成（代码与 headless；真实窗口待补） | `PLAT-004`、`TOOL-MIG-001`、`DUAL-CORE-001` | 清单校验、平台标识、权限和版本记录 |
| `COLLAB-001-A` | 本机加密共享包、版本冲突、撤销和审计边界 | 已完成代码与专项测试 | `CATALOG-001` | 敏感数据阻断、加密落盘、冲突和撤销 |
| `COLLAB-001-B` | 原生 GPUI 选择、导出/导入和远程协作入口 | 当前范围完成（B1、B2、B3 已完成；B4 到期回收及生产 Relay 后置） | `COLLAB-001-A` | 用户确认、选择性同步、冲突和审计 |
| `CANVAS-001..006` | Excalidraw 风格原生协同画布 | 阶段 C 后置，尚未开始 | 阶段 A、B | 场景模型、GPUI 编辑器、本机共享和后续实时协同 |
| `A-UI-REAL` | 阶段 A 真实 Windows 窗口证据收口 | 受 Computer Use 环境阻塞；替代证据已记录但不能替代完整窗口流程 | `DB-UX-001`、`DB-UX-002`、`DB-UX-003`、`DB-UX-004`、`PLAT-004`、`TOOL-MIG-001` | 启动、鼠标/键盘流程、截图和限制记录 |
| `QUALITY-UX-001` | 性能、主题和发布证据收口 | 持续 | 各切片 | 测量、全量质量检查和发布记录 |

## 5. 交付规则

1. 先写设计、范围、不做事项和验收条件，再实现代码。
2. 每个切片只包含一个可验收功能及其必要测试、文档或配置；测试通过后使用一个英文 Conventional Commit，并立即推送当前 `main`。
3. 默认直接在 `main` 开发。用户明确指定功能分支时，基于最新 `main` 创建，验证后合并回 `main`，重新验证并推送，再确认源分支无未合并提交、未推送提交和关联 worktree 后清理。
4. UI 先通过 headless GPUI 交互和边界测试；Computer Use 可用时补真实窗口流程。替代证据必须明确记录限制。
5. 外部服务只使用本机 Docker；记录镜像、端口、健康状态、启动方式和清理方式。PostgreSQL 不低于 17，MySQL 不低于 8.4。
6. 所有 Rust 项目提交前从 workspace 根目录依次通过 `cargo fmt --all -- --check` 和 `cargo clippy --workspace --all-targets -- -D warnings`；之后再次修改必须重新验证。

## 6. 明确不做

- 不把参考图中的连接名、表名、数据值或尺寸当作固定业务数据。
- 不把 DataGrip 全部功能、方言和插件生态列为本项目一次性目标。
- 不以静态截图、仅编译成功或旧版本测试替代真实窗口、交互或本机 Docker 证据。
- 不为统一外观而把 MongoDB、Redis、Kafka 或 SSH 强行套用 SQL 表格和事务语义。
- 不在一个长期分支或一次提交中塞入多个工具的功能。
- 不使用 WebView、嵌入式浏览器或网页页面作为桌面插件 UI。
- 不把第三方动态插件、远程市场和自动同步敏感数据列入原生插件首个交付阶段。
