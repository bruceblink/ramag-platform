# Ramag Platform 执行计划与验收记录入口

> 状态：现行执行规则
> 更新日期：2026-09-26
> 主线：[`development-roadmap.md`](development-roadmap.md)
> 统一 UI 标准：[`ui-acceptance-standard.md`](ui-acceptance-standard.md)
> 历史执行记录：[`archive/2026-09-25-pre-datagrip-rebaseline/development-plan.md`](archive/2026-09-25-pre-datagrip-rebaseline/development-plan.md)

## 术语表与命名约定

| 规范中文名 | English / Acronym | 当前文件中的职责边界 | 不代表什么 |
|---|---|---|---|
| 设计确认 | Design Confirmation | 在实现前确认切片范围、界面规则和验收条件 | 不代表代码已完成 |
| 验收记录 | Acceptance Record | 记录命令、环境、结果、证据边界和未完成项 | 不代表所有产品线都通过 |
| 本机 Docker 集成 | Local Docker Integration | 使用本机容器验证真实数据库或协议服务 | 不代表远程集群或生产验证 |
| 真实窗口 | Native Window | 通过 Computer Use 在 Windows 窗口中完成的操作 | 不代表 headless 渲染或进程启动 |
| 虚拟视图 | Virtual View | 由数据库工具提供的只读对象树节点，通过系统目录或会话查询生成结果 | 不代表数据库中存在同名物理表或可写 View |
| 会话快照 | Session Snapshot | 用户展开 `sessions` 时读取的当前连接会话结果集 | 不代表持久化审计记录或可编辑数据表 |

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

## 4. 当前切片

`SHELL-001` 已完成并推送；`DB-UX-001` 已完成 `DB-RED-01`、`DB-RED-03`、`DB-RED-04`，`DB-UX-002` 已完成 `DB-RED-05A`、`DB-RED-05B`，当前进入 `DB-RED-05C`（结果分页与底部状态栏）。未完成的旧 UI-001、M1-M4、R 系列或工具专项事项必须先映射到新的切片 ID，并重新满足统一 UI 标准后才能恢复；不能仅修改状态文字宣称完成。

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

## 5. 分支和清理

默认在最新 `main` 上开发和推送。只有用户明确要求功能分支时才创建分支；分支必须基于最新 `main`，验证后合并回 `main`，重新验证并推送，再确认源分支无未合并/未推送提交和关联 worktree 后清理。不得删除 `main` 或未明确纳入本次合并的分支。
