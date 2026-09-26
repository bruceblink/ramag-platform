# Ramag Platform 执行计划与验收记录入口

> 状态：现行执行规则
> 更新日期：2026-09-26
> 主线：[`02-development-roadmap.md`](02-development-roadmap.md)
> 统一 UI 标准：[`ui-acceptance-standard.md`](ui-acceptance-standard.md)
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

## 4. 当前切片与后续平台队列

`SHELL-001` 已完成并推送；`DB-UX-001` 已完成 `DB-RED-01`、`DB-RED-03`、`DB-RED-04`，`DB-UX-002` 已完成 `DB-RED-05A`、`DB-RED-05B`、`DB-RED-05C`、`DB-RED-05D`、`DB-RED-06`、`DB-RED-07` 的 headless 功能切片，`DB-UX-003A`、`DB-UX-003B`、`DB-UX-003C-1`、`DB-UX-003C-2`、`DB-UX-004B-3B` 和 `DB-UX-005A` 至 `DB-UX-005F` 的代码与 headless 验证已完成；涉及真实数据库的切片另有 MySQL 8.4/PostgreSQL 17 Docker 证据。当前等待 DB-UX-005 剩余迁移差异收口。`DB-UX-004B-3B` 的 Computer Use 原生窗口证据仍待环境恢复后补验。未完成的旧 UI-001、M1-M4、R 系列或工具专项事项必须先映射到新的切片 ID，并重新满足统一 UI 标准后才能恢复；不能仅修改状态文字宣称完成。

数据库分析主线完成后，后续开发按以下顺序推进：

1. `P0-C`：完成插件设置命名空间、能力授权和运行时权限检查（`P0-C-1` 能力授权检查已完成，设置命名空间仍待实现）。
2. `PLAT-004`：多入口原生 GPUI 插件和标准工具入口。
3. `PLAT-005`：按需创建视图、任务取消以及输入/内存/结果资源预算。
4. `TOOL-MIG-001`：以 JSON Path 为样例迁移 IT Tools 的纯计算核心和原生入口。
5. `DUAL-CORE-001`：为已迁移核心评估 Web/WASM 适配；桌面端不使用 WebView。
6. `CATALOG-001`：建立第一方工具目录，记录工具 ID、版本、平台、权限、数据处理和验收状态。
7. `COLLAB-001`：建立本机优先的文档/结果共享边界，敏感数据默认不自动同步。

这些项目在设计确认前不改变当前 `DB-UX-005` 后续差异和迁移切片的实现范围，也不代表动态插件、第三方市场或远程协作已经实现。

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

### DB-RED-03-UI-01：Server Objects 名称与说明行错位（2026-09-26，设计确认）

- 问题证据：Collations 每项把名称和字符集说明堆叠显示，但对象树使用固定高度的 uniform list，说明文本溢出 28px 行并覆盖下一项名称。
- 设计：保留 28px 统一行高，把可选说明改为主名称右侧的次要文字；名称优先占据剩余宽度并省略，说明保持单行并受行容器裁切。Server Objects 与 Virtual views 共用该规则，保留复制和点击语义。
- 改动范围：只调整元数据树的名称/说明行布局、增加几何调试锚点和 GPUI 回归；不改数据库元数据内容、加载流程或复制行为。
- 验收条件：headless GPUI 在 180/280/360px 窗口验证名称和说明横向分离、始终留在本行且不与下一行重叠；Computer Use 在本机 MySQL 8.4 的 Collations 中检查多项名称与字符集说明；通过定向测试、dbclient 全量测试、fmt、Clippy、源码尺寸和 diff 检查。

### DB-RED-03-UI-02：结果网格垂直滚动条随横向内容偏移（2026-09-26，设计确认）

- 问题证据：本机 MySQL `bulk_records` 结果集的垂直滚动条要等横向滚动到最右端才进入可视区域，横向位置在起点时右侧没有滑块。
- 原因：垂直 `Scrollbar` 默认使用宽结果列表的 `UniformListScrollHandle` 视口边界定位滑块；列表随横向内容变宽，滚动条横坐标也被定位到完整内容的最右端。
- 设计：垂直滚动条使用固定在结果视口右侧的布局边界作为绘制视口，并从 34px 表头下方开始；保持滚动内容尺寸仍来自虚拟列表句柄，不改变横向滚动、分页或滚轮分轴行为。
- 验收条件：headless GPUI 确认固定滚动条视口与数据行视口左右对齐、起始位置和横向末端不变，长结果页保留正向垂直滚动范围；系统截图在 Computer Use 不可用时验证本机 MySQL 8.4 多行结果的右侧滑块在横向起点可见；通过 dbclient 定向测试、fmt、Clippy、源码尺寸和 diff 检查。

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

## 5. 分支和清理

默认在最新 `main` 上开发和推送。只有用户明确要求功能分支时才创建分支；分支必须基于最新 `main`，验证后合并回 `main`，重新验证并推送，再确认源分支无未合并/未推送提交和关联 worktree 后清理。不得删除 `main` 或未明确纳入本次合并的分支。
