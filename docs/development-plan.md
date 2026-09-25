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

`SHELL-001` 已完成并推送；当前正在推进 `DB-UX-001`，其中 `DB-RED-01`、`DB-RED-03` 已完成，下一项是 `DB-RED-04`。未完成的旧 UI-001、M1-M4、R 系列或工具专项事项必须先映射到新的切片 ID，并重新满足统一 UI 标准后才能恢复；不能仅修改状态文字宣称完成。

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

## 5. 分支和清理

默认在最新 `main` 上开发和推送。只有用户明确要求功能分支时才创建分支；分支必须基于最新 `main`，验证后合并回 `main`，重新验证并推送，再确认源分支无未合并/未推送提交和关联 worktree 后清理。不得删除 `main` 或未明确纳入本次合并的分支。
