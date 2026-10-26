# 数据库工作区主线：DataGrip 风格核心闭环

> 状态：现行专项路线图；`DB-UX-005F`、`A-DB-005` 和 `A-DB-RED-02` 的代码、headless 与 Docker 验证已完成；`A-UI-REAL` 已完成 MySQL/PostgreSQL 连接、触发器子树、连接恢复、行编辑读回及单元格 UPDATE 连接失败的部分真实窗口验收；PostgreSQL 结果网格列宽拖动、100,000 行分页、10,000 行只读查询、1 MiB 以上字段预览、1,000 张表的对象树和长表头也已验收，完整数据库操作矩阵仍未完成
> 更新日期：2026-10-27
> 共同视觉与交互基线：[`07-ui-acceptance-standard.md`](07-ui-acceptance-standard.md)
> 跨工具顺序：[`02-development-roadmap.md`](02-development-roadmap.md)
> 历史实现、待办和验收记录：[`archive/2026-09-25-pre-datagrip-rebaseline/03-database-client-datagrip-roadmap.md`](archive/2026-09-25-pre-datagrip-rebaseline/03-database-client-datagrip-roadmap.md)

## 术语表与命名约定

| 规范中文名 | English / Acronym | 本路线图中的职责边界 | 不代表什么 |
|---|---|---|---|
| 数据库工作区 | Database Workbench | 连接、对象导航、查询、结果和 Schema 操作的完整 UI | 不包含 API、Kafka 或 Git 工作流 |
| 数据库对象导航器 | Database Object Explorer | 展示连接、数据库/Schema、表和元数据子节点 | 不代表驱动缓存或数据库本身 |
| 查询控制台 | Query Console | 绑定连接与 Schema 的 SQL/文档查询标签页 | 不代表后台连接池 |
| 结果数据网格 | Result Data Grid | 展示当前页结果并支持滚动、筛选、编辑和复制 | 不代表已读取全部结果 |
| 安全编辑 | Safe Editing | 有身份列、影响行数、事务边界和失败恢复的写入流程 | 不代表任何结果都可以直接写回 |

## 1. 产品目标

把参考图体现的数据库闭环做成 Ramag 的默认工作方式：

```text
连接/Schema
  -> 对象导航器定位表或集合
  -> 打开查询控制台/数据工作区
  -> 执行并在结果网格中浏览
  -> 过滤、排序、查看、复制或编辑
  -> 在事务边界内确认写入
  -> 查看 DDL、执行计划、差异或迁移结果
```

目标是复刻核心操作和界面质感，不是复制 DataGrip 的全部产品表面。所有实现先遵循共享 UI 规范，再补数据库驱动差异。

## 2. 红框验收矩阵（数据库硬性标准）

参考图中的每个红框都是一个独立的可操作验收区域。以下项目全部通过后，数据库工作区才可以标记为达到本轮视觉和功能标准；仅完成颜色、静态布局或 headless 启动不算通过。

| 编号 | 参考图区域 | 必须实现的核心功能 | 最低验收动作 |
|---|---|---|---|
| `DB-RED-01` | 左侧连接行 | 显示连接名称、数据库类型/图标、连接状态、环境标识和同类连接计数；支持选中、展开/折叠、刷新、重连和右键连接动作 | 选择连接后树与中央工作区同步；断开时保留名称并显示失败/重试；切换连接不串写旧结果 |
| `DB-RED-02` | `tables` 对象树 | 按 Schema/表分组，显示数量；表支持展开到 columns、keys、indexes、triggers；支持搜索、收藏、最近访问、刷新和双击打开数据 | 展开/折叠真实表树，双击表打开结果网格；刷新期间旧节点可见，完成后状态和数量正确 |
| `DB-RED-03` | `Server Objects` 分组 | 展示数据库服务级对象（例如 collations、users、系统对象），按需加载并区分权限错误、空状态和加载失败 | 展开分组并读取至少一个真实对象类别；无权限时显示可解释错误和重试，不把空列表当成功加载 |
| `DB-RED-04` | `Virtual views` 分组 | 展示本地/虚拟对象（例如 sessions），提供与真实表不同的图标、只读标识和刷新入口 | 展开并打开虚拟对象；只读对象不出现危险编辑按钮，刷新后不影响真实表树 |
| `DB-RED-05` | 中央 Query Console / Result Workbench | 标签式查询控制台、查询/结果标签、连接与 Schema 上下文、工具栏、`WHERE`/`ORDER BY` 条件条、虚拟化结果网格、分页和底部状态栏 | 从 `DB-RED-02` 双击表进入；执行查询后显示列、行号、排序/过滤图标、双轴滚动、页码范围；切换标签和取消查询不串结果 |
| `DB-RED-06` | `Tx: Auto` 控件 | 明确显示当前事务模式；可在驱动支持范围内切换自动提交/手动事务，并提供开始、提交、回滚和未提交状态 | 切换模式后状态可见；手动事务中的编辑显示未提交，提交/回滚后结果和状态按驱动回读；不支持时说明原因 |
| `DB-RED-07` | `DDL` 控件 | 提供当前对象的只读 DDL 查看/复制/刷新入口，并明确执行/迁移动作不在本切片内 | 从表或查询上下文打开 DDL；DDL 内容可滚动、复制和刷新；执行/迁移风险确认归入后续 `DB-UX-005` |

红框验收的视觉条件：区域边界、分隔线、行高、图标间距、选中色、滚动槽和状态栏符合统一 UI 标准；红框标注线本身不出现在产品中。`DB-RED-01` 至 `DB-RED-07` 任一项未通过时，只能报告为部分完成。

## 3. DataGrip 官方能力对比

下表只采用 JetBrains 官方 DataGrip 文档作为功能参考；网页中的快捷键、版本号和平台细节不直接复制到 Ramag，Ramag 只吸收与红框工作区相关的可观察行为。

| DataGrip 官方能力 | 参考页面 | Ramag 对标范围 | 对应验收 |
|---|---|---|---|
| Database Explorer：数据源、Schema、表和对象树；Speed Search、对象过滤、显示更多 Schema、分组、从编辑器定位、右键动作、DDL、结构对比、Diagram、导入导出和强制刷新 | [Database Explorer](https://www.jetbrains.com/help/datagrip/database-explorer.html) | `DB-RED-01..04` 的导航器；按需加载、过滤、收藏、最近访问、SQL 定位、DDL/Diagram/比较入口 | 真实 MySQL/PostgreSQL 元数据、空/失败/过期状态、树展开和上下文保持 |
| Query Consoles：数据源绑定的多个控制台、独立连接 session、Files 中的 Database Consoles、默认 Schema、Services 中的 Output/Result 标签 | [Query consoles](https://www.jetbrains.com/help/datagrip/query-consoles.html)、[Sessions](https://www.jetbrains.com/help/datagrip/managing-connection-sessions.html)、[Services](https://www.jetbrains.com/help/datagrip/services-tool-window.html) | `DB-RED-05` 的多标签控制台、连接上下文、结果/输出标签和查询草稿；Files/Services 作为可停靠工具窗口 | 多标签并行执行、切换 Schema、取消和迟到回包隔离；标签和结果状态不串线 |
| Data Editor：Table/Tree/Text 查看模式、过滤框、`WHERE`/`ORDER BY`、服务端 `ORDER BY` 与当前页客户端排序、列隐藏/重置、Value Editor、Aggregate View、数据提取器 | [Data editor and viewer](https://www.jetbrains.com/help/datagrip/data-editor-and-viewer.html)、[Database data views](https://www.jetbrains.com/help/datagrip/settings-tools-database-data-views.html) | `DB-RED-05` 的默认 Table 结果网格；先交付截图中的表格视图，再评估 Tree/Text 和提取器 | 大结果分页、过滤历史、排序语义、列宽/隐藏、长字段查看和滚动状态 |
| Data Editor 修改：直接编辑、Add/Delete/Clone Row、`NULL`/DEFAULT、Preview Pending Changes、Submit、Revert Selected、取消正在执行的语句、LOB 读写 | [Submit changes to a database](https://www.jetbrains.com/help/datagrip/submitting-and-reverting-changes.html) | `DB-RED-05..06` 的安全编辑和失败恢复；仅在驱动能力声明允许时显示写入动作 | 未提交状态、DML 预览、提交/回滚、部分失败明细、敏感大字段有界读取 |
| Tx 模式与会话：自动提交/手动提交、session 保存连接和事务控制状态，数据编辑器通过 `Tx` 下拉切换提交模式 | [Sessions](https://www.jetbrains.com/help/datagrip/managing-connection-sessions.html)、[Submit changes](https://www.jetbrains.com/help/datagrip/submitting-and-reverting-changes.html) | `DB-RED-06` 的 `Tx: Auto` 控件；自动/手动模式、开始/提交/回滚和失效恢复 | MySQL/PostgreSQL Docker 真实提交和回滚；不支持的驱动显示原因而不是隐藏失败 |
| Query Plan：Explain Plan/Analyse、专用 Query Plan 标签、Operations Tree、Raw、Diagram、Flame Graph、复制数据库原生计划 | [Query execution plan](https://www.jetbrains.com/help/datagrip/query-execution-plan.html) | `DB-RED-05` 的结果标签和 `DB-UX-005`；先交付原始/结构化视图，再评估图形和火焰图 | Explain 失败保留原文；Analyse 风险提示；原生计划复制不丢列名或格式 |
| Schema Comparison：同类型对象 Origin/Target、Migration 脚本、DDL/Object Properties Diff、变更颜色、Diff Viewer 和可修改脚本 | [Schema comparison and migration](https://www.jetbrains.com/help/datagrip/schema-comparison-and-migration.html) | `DB-RED-07` 的 DDL/迁移入口和 `DB-UX-005` 的结构协作 | 源/目标、脚本指纹、人工确认、破坏性变更分组、执行后元数据回读 |
| 数据库全文搜索：不知道数据所在表时按数据内容搜索 | [Full-text search in databases](https://www.jetbrains.com/help/datagrip/full-text-search-for-databases.html) | `DB-UX-001`/`DB-UX-006` 的跨对象搜索；不把搜索结果伪装成普通表树节点 | 搜索范围、取消、结果上下文和敏感字段展示边界明确 |

官方文档显示的更多 DataGrip 功能（脚本扩展、原生 `mysqldump`/`pg_dump`、插件生态、方言专用工具等）不自动进入本轮红框验收；只有加入新的用户验收区域后，才建立对应的 Ramag 切片。

## 4. 当前能力基线

以下能力已经在旧路线图或代码中存在，现阶段只作为可复用基础，不自动视为满足新视觉验收：

- SQL、MongoDB 和 Redis 的连接、查询取消、对象树和结果面板。
- SQL 结果服务端分页、排序、过滤、精确页码、双轴滚动、单元格查看、复制和编辑。
- 查询标签、历史、最近关闭草稿、事务提交/回滚/保存点和执行状态隔离。
- EXPLAIN 结果、Schema Diagram、表结构/结果差异、迁移预览、显式确认、执行后回读和审批记录。
- MySQL 8.4+、PostgreSQL 17+ 本机 Docker 测试基线，以及 SQLite/MongoDB/Redis 的独立驱动测试。

旧功能的提交、测试、未完成项和真实窗口限制保留在归档文件；新切片必须重新检查截图要求中的对象树、工具栏、连接状态、网格密度和状态反馈。

## 5. 实施队列

### DB-UX-001：对象导航器（覆盖 `DB-RED-01` 至 `DB-RED-04`）

**范围**：连接行显示名称、类型、状态和环境；顶部提供新增、刷新、过滤、系统对象和设置；树按数据库/Schema、表、列、键、索引、触发器等分组；支持按需展开、收藏、最近访问、右键动作和 SQL 表定位。

**验收**：

1. 连接失败或刷新失败时保留原连接名和旧树内容，错误区提供重试；成功恢复后只更新受影响节点。
2. 长表名、Schema 名和统计值在 `240..520px` 侧栏内不互相覆盖；过滤/清空搜索不丢失连接和 Schema 上下文。
3. MySQL 8.4 与 PostgreSQL 17 Docker 实际元数据可展开；MongoDB/Redis 显示各自对象模型，不伪造表/列层级。

#### A-DB-RED-02：对象树真实元数据复验（2026-09-27，Docker 与 headless 验证完成）

- Docker：复用本机健康的 `ramag-db-test-mysql`（`mysql:8.4`，`127.0.0.1:13306`）和 `ramag-db-test-postgres`（`postgres:17-alpine`，`127.0.0.1:15432`）；MySQL `list_` 集成测试 4 项、PostgreSQL `list_` 集成测试 4 项、两端临时触发器测试各 1 项通过。临时对象由测试清理，未停止用户复用的容器、网络或数据卷。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib table_tree -- --nocapture` 通过 42 项，覆盖 Schema 刷新保留旧树、表/列/键/索引/触发器分组、搜索、过滤、连接隔离和窄窗口布局。
- 结果：真实驱动元数据、表树代码和 headless 交互达到当前矩阵要求；真实 Windows 鼠标/键盘流程由 `A-UI-REAL` 分批验收，当前已完成的范围见下文。

#### A-UI-REAL：真实数据库工作区窗口验收（2026-10-25 至 26，部分完成）

- 环境：使用隔离 Debug 数据目录启动 `target/debug/ramag.exe`。MySQL 使用本机 `ramag-db-test-mysql`（`mysql:8.4`，实际版本 `8.4.9`，`127.0.0.1:13306`）；PostgreSQL 使用 `ramag-db-test-postgres`（`postgres:17-alpine`，实际版本 `17.10`，`127.0.0.1:15432`）。两者复用现有专用数据卷，没有重建或清空卷。
- MySQL 窗口：连接到 `ramag_test` 后，数据库对象树显示 `bulk_records`、其他表和视图；打开 `bulk_records` 后结果网格显示 `1–100 / 100000`。随后在 SQL 编辑器执行 `SELECT id, group_id, amount FROM ramag_test.bulk_records LIMIT 10;`，结果显示 10 行、3 列，耗时约 35 ms。
- PostgreSQL 窗口：连接到 `ramag_test` 后，`public` Schema 显示 3 个表和 1 个视图；打开 `bulk_records` 后结果网格显示 `1–100 / 100000`，列包含 `int8`、`int4`、`numeric`、`text` 和 `jsonb` 等驱动类型。
- 窄侧栏布局修复（2026-10-25）：修复 `bulk_records` 展开后的列名与类型挤压问题。约 300px 宽的真实窗口中，列名不再变成空白块，长名称和类型在各自区域截断，不再互相遮挡；悬停可查看完整值，原有主修饰键双击复制保留。两条历史测试数据源在关闭并重启 Debug 程序后仍可用。
- 结果网格底部空白修复（2026-10-25）：水平滚动条使用独立轨道的布局范围绘制并接收输入，避免滑块画在表格视口内、其下方仍保留一整行空白。轨道高度从 16px 缩到 10px，滑块下缘紧贴状态栏。使用新构建的 `target/x86_64-pc-windows-msvc/debug/ramag.exe`，在 PostgreSQL `bulk_records` 真实窗口中确认：左右拖动可查看末列 `updated_at` 并返回首列；纵向滚到当前页第 100 行时末行完整可见；翻到 `101–200 / 100000` 后分页栏与滚动条仍相邻，没有原有空白带。headless 回归另行覆盖底部轨道点击、窄窗口分页布局和 10000 行虚拟列表末端。
- Virtual views：PostgreSQL `sessions` 只读会话快照可打开，结果网格显示 9 行，范围为 `1–9 / 9`；没有执行写入。
- 查询取消尝试（2026-10-25）：在 PostgreSQL 执行 `SELECT pg_sleep(10);` 和 `SELECT pg_sleep(5);`，并尝试点击停止控件。两条查询仍分别在约 10006ms 和 5009ms 后完成。此两次尝试未观察到取消结果，单独不能确认取消行为；后续成功复验见下方。
- 查询取消复验（2026-10-25）：在更新后的 Debug 窗口执行只读 `SELECT pg_sleep(30);` 并立即点击取消。UI 显示“服务器已确认取消查询”，停止按钮恢复为运行按钮；随后从另一条 PostgreSQL 会话读取 `pg_stat_activity`，目标 PID 状态为 `idle`，确认长查询没有继续执行。`ramag-infra-postgres` 的 Docker 集成用例 `cancellable_query_can_be_stopped_by_backend_id` 也通过（1 项）。本次没有执行 DML/DDL；保留先前未成功的尝试记录，查询取消现由本次复验确认通过。
- 长列标题原生显示复验（2026-10-25）：在 PostgreSQL 结果网格执行带有超长别名的 5 行只读查询。表头按列宽截断，`int8` 类型和相邻 `title_value` 列仍可见，5 行数据正常显示，没有标题遮挡相邻列。列宽拖拽与大结果集的完整原生窗口矩阵仍待独立验收。
- 触发器子树（2026-10-26）：复用已健康运行的 MySQL 8.4.9（`127.0.0.1:13306`）和 PostgreSQL 17.10（`127.0.0.1:15432`）。在两端测试库的 `bulk_records` 表上各建一个无数据变更的临时 `BEFORE UPDATE` 触发器。真实窗口展开 MySQL 和 PostgreSQL 表节点后，均显示 `触发器 (1)` 和对应触发器行。没有执行会触发该逻辑的 DML。
- 连接失败与恢复（2026-10-26）：将隔离 MySQL 数据源端口临时改为未监听的 `13307`。真实窗口保留数据源名称，显示“连接失败”、超时原因和“重试”入口。恢复 `13306` 后使用“重连”，状态回到“已连接”，`ramag_test` 对象树重新加载。配置变更期间旧标签先进入暂停状态；这项窗口操作没有证明连接失败时旧树内容仍可见。旧树保留由下方 headless 回归覆盖。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib table_tree_refresh_keeps_existing_rows_during_reload_and_failure -- --nocapture` 通过。测试覆盖刷新期间和失败后保留已有表行，并保留重试入口。本机 F 盘空间为 0；测试使用 C 盘独立 Cargo 目录完成。
- 空事务控制（2026-10-26）：MySQL 8.4.9 与 PostgreSQL 17.10 的真实窗口均从 `Tx: Auto` 切到 `Tx: Manual`，并显示待提交状态说明。随后分别开始空事务并提交、回滚；界面提示事务已完成对应动作，事务控件恢复为 `Tx: Auto`。没有编辑表行或执行 DML，因此本次不证明待提交行变化或提交后的数据回读。
- 行编辑与事务读回（2026-10-26）：在 MySQL 8.4.9 与 PostgreSQL 17.10 的真实窗口中，打开单行临时表 `ramag_ui_txn_acceptance`，将 `marker` 从 `before` 改为 `rollback_probe`。按 Enter 后，界面显示“未提交 1 项修改”。确认“提交修改”后，界面报告影响 1 行，事务状态为“未提交”；独立连接仍读到 `before`。点击事务“回滚”后，界面提示回滚成功，旧结果失效；重试查询后网格显示 `before`，独立连接也读到 `before`。再次把 `marker` 改为 `commit_probe` 并确认提交修改；事务提交前，独立连接仍读到 `before`。点击事务“提交”后，界面提示事务已提交，独立连接读到 `commit_probe`。MySQL 临时表在全库同名检查确认不存在后创建；两端均只改这条测试行，没有修改共享测试数据。验收后删除两张临时表，`information_schema` 计数均为 0。
- Docker 清理状态（2026-10-26）：MySQL `ramag-db-test-mysql`（`mysql:8.4`，`127.0.0.1:13306`）和 PostgreSQL `ramag-db-test-postgres`（`postgres:17-alpine`，`127.0.0.1:15432`）均保持运行且健康；没有停止容器、重建容器或修改数据卷。
- Headless：`cargo test --locked -p ramag-tool-dbclient --lib transaction -- --nocapture` 通过 4 项，覆盖事务错误状态和活动事务控件布局。
- 构建边界：真实窗口使用已存在的 `0.7.0` Debug 程序，产品版本为 `0.7.0`。本轮新的 `cargo build --locked -p ramag-bin` 未完成，MSVC 链接以 `LNK1140` 退出；`v0.7.0` 标签之后的仓库变更只有文档，因此运行程序的 Rust 源码与当前源码一致。
- 清理：验收后删除 MySQL 和 PostgreSQL 临时触发器，并删除 PostgreSQL 辅助函数。两端 `information_schema` 查询均确认触发器数量为 0，PostgreSQL 函数数量为 0；随后窗口刷新并重新读取表详情，触发器分组不再出现。两个 Docker 容器保持健康；没有停止容器、重建容器或更改数据卷。没有修改测试数据行。
- 系统提示：Computer Use 期间出现与本次验收无关的 GPG 私钥口令提示。已取消该提示，没有输入口令。
- 状态：本轮新增确认 MySQL/PostgreSQL 触发器子树真实窗口显示、MySQL 连接失败后的恢复，两端事务行编辑提交/回滚读回，以及两端单元格 UPDATE 的 DML 连接失败反馈。旧树保留由 headless 测试确认。查询取消的旧失败尝试和后续成功复验仍按原记录保留。另在 PostgreSQL 真实窗口验收了 `bulk_records` 的列宽拖动、100,000 行分页、页内末行、末列横向滚动和长表头显示。`DB-UX-006A` 的 10,000 行只读结果加载与内存抽样、`DB-UX-006B` 的 1 MiB 字段预览，以及 `DB-UX-006C` 的 1,000 张表树加载与滚动也已完成。完整数据库操作矩阵仍未完成；并行查询、单项取消和页面切换测量继续按专项验收条件拆分。本条保持部分完成。

### DB-UX-002：查询控制台（覆盖 `DB-RED-05` 至 `DB-RED-07` 的界面入口）

**范围**：标签标题、连接/Schema 上下文、SQL 或文档编辑器、运行/停止、历史、格式化、事务模式、DDL、搜索、过滤、刷新、复制、导入/导出和设置入口。

**验收**：

1. 并行标签的结果、COUNT、取消提示和错误不会跨连接或跨执行代次覆盖。
2. 360px 宽度时工具栏换行或滚动，1024/1440px 时保持截图中的紧凑排列；当前标签和连接状态始终可见。
3. 取消、连接失效和 Schema 切换均显示可解释状态；失败保留编辑内容和重试入口。

#### DB-RED-05A：Query Console 上下文条（已完成）

先交付截图顶部工作区的连接/驱动/端点上下文和 Schema 下拉切换，再进入结果网格分页、排序和 `WHERE`/`ORDER BY` 条件条。连接上下文必须与对象树使用同一连接配置；Schema 切换复用现有确认和多标签同步逻辑。验收只认 headless 三尺寸边界、活动 Schema 同步和 `Tx: Auto` 明确显示，不把静态标签视为完成。

#### DB-RED-05B：结果网格排序入口（已完成）

在现有 `WHERE` 条件输入旁提供 `ORDER BY` 下拉入口，从当前结果列生成 ASC/DESC 选项，并与列头箭头、服务端分页排序保持同一状态。排序菜单只传递列序号和固定方向，不把列名拼接进 SQL；未提交编辑或写操作进行中时沿用现有禁用提示。

#### DB-RED-05C：结果分页与底部状态栏（已完成）

对齐参考图底部的范围/总数/翻页条，统一页码、页大小、总行数未知和加载/失败状态；分页控件必须与当前连接、Schema、排序和筛选结果绑定。先复用已有分页事件和页大小菜单，再补齐三种窗口尺寸下的底部不遮挡验收。

#### DB-RED-05D：结果单元格查看与复制（已完成）

对齐截图中长文本/JSON 单元格的截断展示，补充单元格详情查看、当前值复制和 NULL/空字符串/二进制状态区分；详情只读取当前单元格，不复制整个结果集。保留现有虚拟行、列宽、排序、筛选、分页与编辑保护。

#### DB-RED-06：`Tx: Auto` 事务控件（已完成）

对齐截图工具栏中的 `Tx: Auto` 下拉入口，明确自动提交、手动事务、处理中和错误状态，并把开始、提交、回滚和未提交修改反馈收束到同一会话上下文。复用现有驱动能力声明、事务事件和保存点，不改写 SQL 执行或 DML 生成。

#### DB-RED-07：`DDL` 控件（已完成）

提供当前对象的只读 DDL 查看、复制和刷新入口，明确本切片不执行 DDL；执行/迁移的目标连接、风险确认和失败回读归入后续 `DB-UX-005`。

### DB-UX-003：结果数据网格（当前阶段）

**范围**：虚拟化行、稳定表头、行号、列类型图标、列宽、排序、列过滤、`WHERE`/`ORDER BY` 条件条、双轴滚动、分页、单元格详情、复制和导出。

**验收**：

1. 大于当前页的数据只读取并显示有界页，底部明确显示当前范围/总数状态；精确总数不可用时不伪造页码。
2. 列总宽超过视口时可以滚到最后一列；行数超过视口时滚动条、最后一行和分页状态不重叠。
3. NULL、空字符串、二进制、长文本和截断值有不同状态；打开详情只读取当前单元格，不复制整个结果集。
4. 排序/筛选重新加载后保留或明确重置页码、横向位置、连接和 Schema 上下文。

#### DB-UX-003A：结果导出格式和作用域控件（已完成）

对齐截图中的 `CSV` 下拉入口，提供 CSV/JSONL 格式选择；导出范围继续限定为有效选中行，格式编码和文件扩展名必须一致。

#### DB-UX-003B：稳定表头和双轴滚动（已完成）

继续加固截图中的结果网格：表头在纵向滚动时保持稳定，横向滚动只移动列内容，垂直/水平滚动条和底部状态栏不互相覆盖；保留现有分页、排序和单元格查看行为。

#### DB-UX-003C：大结果集列布局和滚动边界（窗口验收完成）

验证超宽列、超长表头、大结果集和分页状态下的列宽上限、虚拟行边界、滚动范围和状态栏可见性；不降低结果内存预算和异步取消要求。

首个子切片 `DB-UX-003C-1` 已把手动列宽限制为 `60–800px`，并补齐首次拖拽增量、列宽端点和横向滚动的 headless GPUI 回归。`DB-UX-003C-2` 的真实窗口验收已完成：

- 环境：使用现有 Ramag `0.7.0` Debug 窗口和 PostgreSQL 17.10 测试容器 `ramag-db-test-postgres`（`postgres:17-alpine`，`127.0.0.1:15432`）。容器检查结果为运行且健康。
- 列宽与横向滚动：打开 `ramag_test.public.bulk_records`，把 `id` 列边界向右拖动约 50px；`id` 与相邻 `group_id` 仍清楚分隔。拖动水平滑块到最右端后，`created_at` 和 `updated_at` 列可见，横向滚动条与底部状态栏保持分离。
- 大结果与分页：表页显示 `1–100 / 100000`。滚动到页末时第 100 行完整显示在分页状态栏上方。翻到第二页后显示 `101–200 / 100000`；滚动到页末时第 200 行也完整显示，页码范围和滚动条未互相遮挡。
- 长表头：只读查询 `SELECT id AS "very_long_column_header_that_must_not_overlap_adjacent_columns_0123456789", group_id, amount FROM public.bulk_records ORDER BY id LIMIT 10;` 返回 10 行。长标题按列宽截断，`int8` 类型和相邻列标题仍可见。
- 数据边界：本切片只读取现有种子数据，没有创建对象或执行 DML。Docker 服务未停止、重建或改动数据卷。Rust 源码未变；自动化测试不适用，本条只记录真实窗口验收。

水平滚动条与状态栏之间的空白已修复，横向滑块左右拖动、当前页末行、翻页和超长列标题的截断显示均有真实窗口证据。高规模 SQL 结果集、宽字段和并行查询仍归入 `DB-UX-006`。

### DB-UX-004：安全编辑与事务（覆盖 `DB-RED-06`）

**范围**：单元格/新增行编辑、未提交标记、批量提交/撤销、影响行数、错误明细、事务开始/提交/回滚/保存点和刷新。

**验收**：

1. 没有可靠定位键、驱动不支持安全写入或目标为只读连接时，写入入口禁用并说明原因。
2. 写入失败保留用户输入和数据库错误，不把部分成功显示成全失败；提交、回滚和切换标签的状态互斥。
3. MySQL/PostgreSQL 在本机 Docker 中验证提交、回滚、取消、连接失效和属性回读；MongoDB/Redis 单独验证文档或 Key-Value 语义。

#### DB-UX-004A：撤销选中的未提交单元格修改（已完成）

- 设计：结果状态栏在已有“撤销全部修改”旁提供“撤销选中”；只有当前选中单元格存在本地未提交草稿时可用。
- 改动范围：ResultPanel 的本地草稿选择/移除 API、结果状态栏按钮和 `280/360/1024px` headless 布局与交互回归；不执行数据库请求，不改变提交、事务或行定位 SQL。
- 验收条件：选中一个待提交单元格后撤销只移除该单元格草稿，其他行/单元格草稿保留；没有选中草稿时按钮禁用；按钮和状态栏在支持宽度内不越界。
- 不做事项：不在本切片完成 MySQL/PostgreSQL DML 真实提交、回滚、取消或连接失效测试；这些属于后续 `DB-UX-004B`。
- 验收结果：`selected_pending_edit_reverts_without_touching_other_drafts` 通过，覆盖 `280/360/1024px`；`cargo test --locked -p ramag-tool-dbclient --lib --quiet` 通过（338 项）；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口证据和 Docker DML 验收留给后续切片。

#### DB-UX-004B-1：提交前 DML 确认（已完成）

- 设计：点击结果状态栏“提交修改”先打开确认框，展示待提交单元格数量、涉及行数和自动提交/手动事务模式；确认回调重新从当前草稿生成定位 SQL，再进入既有 DML 执行路径。
- 取消语义：取消、Esc 或关闭确认框只关闭确认层，保留全部本地草稿，不改变结果集和事务状态，也不发出数据库请求。
- 改动范围：ResultPanel 提交摘要 API、结果状态栏确认入口和 headless GPUI 确认框回归；不改驱动协议、SQL 方言生成器或事务接口。
- 验收条件：确认框在 `280/360/1024px` 内；取消后所有草稿仍存在；确认入口显示当前模式；确认期间修改草稿时，确认回调不会复用旧 SQL。
- 不做事项：不在本切片运行真实 MySQL/PostgreSQL DML；后续 `DB-UX-004B-2` 使用本机 Docker 验证提交、回滚、取消、连接失效和属性回读。
- 验收结果：结果表 headless 测试确认框入口和取消保留草稿通过；dbclient 全量 338 项测试、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。下一项为 `DB-UX-004B-2`，补齐本机 Docker MySQL 8.4/PostgreSQL 17 的真实 DML 与事务回读。

#### DB-UX-004B-2：结果编辑器 DML 与事务真实回读（已完成）

- 设计：为 MySQL 8.4 和 PostgreSQL 17 分别增加 Docker 集成测试，使用结果编辑器同类的主键定位 UPDATE；验证自动提交影响行数、事务内读取、回滚后独立连接保持原值，以及提交后独立连接读取新值。
- 测试数据：每个测试创建带主键的临时表，使用进程号生成表名，测试末尾删除；不修改共享种子数据，不依赖远程服务。
- 验收条件：测试明确使用 `127.0.0.1:13306` 的 MySQL 8.4 和 `127.0.0.1:15432` 的 PostgreSQL 17-alpine；两个驱动均通过自动提交 UPDATE、回滚和提交回读；测试失败时保留数据库错误并清楚标识后端。
- 不做事项：不在本切片改 UI 或驱动协议，不覆盖连接失效和原生窗口交互；这些留给后续 `DB-UX-004B-3`。
- 验收结果：MySQL 与 PostgreSQL 新增 DML 回读测试各 1 项通过；两个基础设施 crate 的 `--all-targets` 测试全部通过，workspace Clippy、fmt、源码尺寸和 `git diff --check` 通过。测试使用 `ramag-db-test-*` 专用容器和卷，临时表已删除，容器保持健康运行供后续窗口验收复用。

#### DB-UX-004B-3A：DML 连接失败状态与重试可见性（已完成）

- 设计：DML 请求返回数据库错误后，结果状态栏显示有界的失败摘要；未提交草稿继续保留，原“提交修改”按钮保持可用并作为重试入口，避免错误只存在于短暂 Toast 中。
- 状态清理：开始新的提交时清除旧失败状态；提交成功或用户撤销全部草稿后清除；刷新/切换结果时不把旧错误带到新结果。
- 改动范围：ResultPanel DML 错误状态、结果状态栏错误锚点与 headless 响应式回归；不改数据库协议和错误原文的日志记录。
- 验收条件：错误摘要在 `280/360/1024px` 不越界；失败后草稿数量不变，提交入口仍可见；成功/撤销/新结果会清理旧错误。Computer Use 原生窗口证据因当前原生窗口列表不可用，单独记录为未完成。
- 验收结果：headless GPUI 回归在 `280/360/1024px` 验证错误锚点、草稿保留和撤销清理；dbclient 全量 338 项、fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。Computer Use 在 `ramag.exe` 进程有窗口标题时仍返回空原生应用列表，未执行鼠标/键盘交互，原生窗口验收转入 `DB-UX-004B-3B`。

#### DB-UX-004B-3B：所有结果编辑 DML 的失效连接反馈（实现、Docker/headless 与单元格 UPDATE 原生窗口验证完成）

- 设计：单元格 UPDATE、行内 DELETE、批量 DELETE 和 INSERT 的数据库失败统一写入结果面板的持久错误状态；状态栏继续显示有界摘要，当前操作入口保持可用，用户恢复连接后可从同一入口重试。
- 草稿规则：数据库请求失败不得清理未提交单元格修改或新增行输入；成功、撤销全部草稿、刷新结果或切换结果上下文时清理旧错误，避免错误跨结果集残留。
- 改动范围：ResultPanel DML 错误状态归一化、结果状态栏与对应 headless GPUI 回归；不做自动重连、自动重试、驱动协议或事务语义变更。
- 验收条件：`280/360/1024px` 下错误摘要和 DML 操作区均不越界；四类 DML 失败都能在面板状态中保留错误并保留草稿；本机 Docker MySQL 8.4（`127.0.0.1:13306`）和 PostgreSQL 17（`127.0.0.1:15432`）用失效连接配置得到可识别错误；Computer Use 原生窗口流程单独记录实际可用性。
- 验收结果：UPDATE、单行/批量 DELETE、INSERT 统一写入持久 DML 错误状态；`ramag-tool-dbclient` 全量 338 项 headless 测试通过，覆盖三种窗口宽度、错误状态和最后一项草稿撤销清理；MySQL 8.4 与 PostgreSQL 17 Docker 失效连接测试各 2 项通过，容器保持健康运行。`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- 历史限制：较早的 Computer Use 尝试曾因空应用列表而没有执行原生窗口流程；这不再代表当前工具状态。
- 原生窗口补验（2026-10-26）：MySQL 8.4.9 和 PostgreSQL 17.10 的真实窗口均使用单行临时表 `ramag_ui_txn_acceptance`。先在 `Tx: Manual` 中通过结果网格更新为 `tx_seed`；独立连接仍读到 `before`。MySQL 事务连接由 `information_schema.innodb_trx` 唯一定位为线程 `7431`；PostgreSQL 事务连接由 `pg_stat_activity` 按 `idle in transaction` 和最后一条临时表 UPDATE 唯一定位为 PID `52654`。分别只终止这两个测试连接，没有停止容器或修改数据源配置。随后在网格把值改为 `failure_probe` 并提交：MySQL 界面显示 TLS 连接意外关闭错误，PostgreSQL 显示管理员终止连接错误；两端均保留 1 项未提交修改。撤销本地草稿、结束失效事务并重试查询后，网格和独立连接均读到 `before`。原生窗口只覆盖单元格 UPDATE；行内 DELETE、批量 DELETE 和 INSERT 的失败状态仍由既有 headless 回归覆盖。验收后删除两端临时表，`information_schema` 计数均为 0；两个 Docker 容器仍运行且健康。

#### DB-UX-005A：EXPLAIN ANALYZE 执行风险确认（已完成代码与 headless 验证）

- 问题：`EXPLAIN ANALYZE` 会执行目标查询；当前风险提示只覆盖其中的危险写操作，普通 SELECT 仍缺少执行前确认。
- 设计：对 MySQL/PostgreSQL 的 `EXPLAIN ANALYZE` 选项统一返回有界风险摘要，沿用现有执行代次和上下文变化校验；普通 `EXPLAIN` 保持只读直通，结构化与原始结果视图不变。
- 验收：`EXPLAIN ANALYZE SELECT` 在两种 SQL 驱动上都要求确认；危险 DML 的既有提示继续保留；普通 `EXPLAIN SELECT` 不触发确认；字符串、注释和嵌套查询中的 `ANALYZE` 不误报；通过 dbclient 定向/全量测试、fmt、Clippy、源码尺寸和 diff 检查。
- 结果：定向 SQL 风险测试 32 项、`ramag-tool-dbclient` 全量测试 340 项通过；`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check` 通过。该切片为纯风险检测和执行前确认逻辑，没有新增真实窗口交互；Computer Use 限制单独保留，不影响本次代码验收。

#### DB-UX-005B：迁移执行后的目标结构回读校验（已完成代码与 headless 验证）

- 问题：迁移执行成功后当前流程只刷新源表和目标表元数据，没有根据回读结果判断目标结构是否一致。
- 设计：成功执行迁移后自动回读列、索引和外键；无警告且差异为空时显示回读一致；元数据加载不完整时显示原因；仍有差异时显示有界差异数量并保留差异面板，不能宣称迁移完成。
- 改动范围：`SchemaDiffDialog` 回读状态、刷新回调和通知；不改迁移 SQL 生成、执行审批、驱动协议或差异算法。
- 验收：覆盖一致、警告和仍有差异三种回读结果，并验证请求代次不会让旧回读覆盖新上下文。
- 结果：回读状态测试覆盖一致、警告和仍有差异；`schema_diff_dialog` 定向测试 12 项、`ramag-tool-dbclient` 全量测试 343 项通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口限制沿用现有 Computer Use 记录。

#### DB-UX-005C：无稳定键的结果差异安全边界（已完成代码与 headless 验证）

- 问题：结果差异在两侧没有共同主键或唯一键时，会按共有列内容匹配相同的行；这种匹配无法证明两行来自同一条数据库记录，可能把用户误导为已完成行级对应。
- 设计：只有两侧都明确提供且值可用的主键或非空唯一键时，才生成行级匹配和单元格变化；没有共同稳定键时不再按内容或位置猜测，源侧加载行全部显示为整行删除，目标侧加载行全部显示为整行新增，并在摘要、提示和复制文本中说明比较范围。
- 改动范围：结果差异匹配模式、整行差异统计和用户提示；不改变稳定键来源、数据库查询、结果快照、分页或导出上限。
- 验收条件：无稳定键时即使存在相同内容也不产生“未变化”或单元格差异；整行新增/删除数量覆盖所有已加载行；有稳定键的变更行继续保留单元格定位；`result_diff` 定向测试、dbclient 全量测试、fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 结果：无稳定键统一使用 `Unkeyed` 模式，所有已加载源行显示为删除、目标行显示为新增，不再按内容猜测对应关系；稳定键路径和单元格定位保持不变。`result_diff` 定向测试 8 项、`ramag-tool-dbclient` 全量测试 343 项通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口证据沿用现有 Computer Use 限制。

#### DB-UX-005D：迁移脚本指纹复核（已完成代码与 headless 验证）

- 问题：迁移审批记录已经保存脚本 SHA-256 指纹，但当前迁移预览和确认说明没有展示本次脚本指纹；用户复制或保存脚本后无法快速核对它是否与审批记录中的脚本一致。
- 设计：对当前完整迁移脚本计算稳定的 SHA-256 指纹；预览面板显示完整指纹，确认说明带上同一指纹，复制文本保留指纹和脚本内容的对应关系。指纹只用于复核，不保存 SQL 正文，不改变执行语义。
- 改动范围：迁移脚本指纹展示、复制文本头部、执行确认上下文和对应 headless 回归；不改变 SQL 生成、审批存储格式、生产只读策略或数据库驱动。
- 验收条件：相同脚本指纹稳定，不同脚本指纹不同；预览、复制文本和确认上下文使用同一指纹；长指纹在支持窗口内不溢出；迁移执行和既有审批测试继续通过。
- 结果：预览显示完整 SHA-256，复制内容带同一指纹头部，执行确认说明复用同一指纹函数；`schema_diff_dialog` 定向测试 13 项、`ramag-tool-dbclient` 全量测试 344 项通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口证据沿用现有 Computer Use 限制。

#### DB-UX-005E：唯一重命名候选迁移（已完成代码与 headless 验证）

- 问题：结构差异已经标记唯一的列重命名候选，但迁移生成器仍把完全不同的列名当作删除加新增，可能丢失原列数据。
- 设计：仅当源侧和目标侧各自恰好存在一个未按名称匹配、且完整列定义（类型、主键、空值、默认值、注释、生成/自增属性和已知位置）完全一致的候选时，才生成显式重命名 SQL；位置缺失、候选不唯一或定义有任何差异时继续保守地生成删除/新增或返回现有人工复核提示。MySQL 使用 `CHANGE COLUMN` 保留完整定义，PostgreSQL/SQLite 使用 `RENAME COLUMN`。
- 改动范围：列迁移候选配对、方言 SQL 和迁移单元测试；不改变结构差异候选算法、索引/外键依赖顺序、审批指纹或执行确认。
- 验收条件：唯一候选只生成一次重命名且不生成 DROP/ADD；定义变化或候选歧义不自动重命名；MySQL/PostgreSQL/SQLite 方言测试、全量 dbclient 测试、fmt、Clippy、源码尺寸和 `git diff --check` 通过。
- 结果：MySQL 使用 `CHANGE COLUMN` 保留定义，PostgreSQL/SQLite 使用 `RENAME COLUMN`；仅唯一且完整定义一致时采用重命名，歧义或定义变化仍显示删除/新增。迁移定向测试 19 项、`ramag-tool-dbclient` 全量测试 349 项通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口证据沿用现有 Computer Use 限制。

#### DB-UX-005F：迁移阶段逐段复核（已完成代码与 headless 验证）

- 问题：迁移预览已经按删除外键、删除索引、处理字段、恢复索引和恢复外键分阶段统计，但复制脚本和执行确认只显示总语句数，复核记录缺少阶段级风险摘要。
- 设计：沿用生成器提供的阶段顺序和破坏性计数，生成有界的阶段复核摘要；复制内容在脚本指纹后附带摘要，执行确认在总量说明后附带相同摘要。阶段摘要只描述现有 SQL，不改变语句顺序、审批指纹或执行逻辑。
- 改动范围：阶段摘要格式化、复制文本和确认说明；不增加逐阶段执行、自动跳过或新的数据库协议。
- 验收条件：预览、复制文本和确认说明使用相同阶段顺序与计数；空阶段不输出虚假项目；长阶段标题和摘要在支持宽度内可滚动；现有迁移、审批、回读和全量测试继续通过。
- 结果：复制文本和执行确认复用同一阶段摘要，覆盖阶段顺序、语句数和破坏性计数；`schema_diff_dialog` 定向测试 14 项、`ramag-tool-dbclient` 全量测试 351 项通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 通过。真实窗口证据沿用现有 Computer Use 限制。

#### A-DB-005：迁移差异回放收口（2026-09-27，Docker 验证完成）

- 目标：用真实 MySQL/PostgreSQL 服务回放当前迁移生成器覆盖的多语句脚本，确认字段、索引、主键和外键动作能够回读；确认 PostgreSQL 事务失败时保留旧结构。
- Docker：本机 `ramag-db-test-mysql` 使用 `mysql:8.4`、`127.0.0.1:13306`，本机 `ramag-db-test-postgres` 使用 `postgres:17-alpine`、`127.0.0.1:15432`；两者在测试前已健康运行，测试创建进程后缀临时表/Schema 并自行删除，没有停止用户复用的容器、网络或数据卷。
- 结果：MySQL `migration_replay` 1 项通过；PostgreSQL `migration_replay` 2 项通过，包含失败回滚；未设置 `RAMAG_TEST_*` 时的跳过结果不计入通过。
- 限制：迁移对话框的真实 Windows 鼠标/键盘流程仍待 Computer Use 环境恢复；下一项为 `A-DB-RED-02`。

### DB-UX-005：分析、差异和迁移（覆盖 `DB-RED-07`）

**范围**：原始/结构化 EXPLAIN、只读 Schema Diagram、表/结果差异、DDL 预览、迁移阶段摘要、脚本指纹、审批、执行和回读。

**验收**：

1. 解析失败保留数据库原文；`EXPLAIN ANALYZE` 等可能执行查询的动作显示风险并要求确认。
2. 差异比较不猜测不可靠的行键；迁移脚本变化后旧确认记录失效；生产目标默认只读。
3. 迁移执行前可复制/保存/复核，执行后读取目标元数据确认列顺序、生成列、自增/IDENTITY、索引和外键动作没有静默丢失。

### DB-UX-006：大规模数据与导航性能

**范围**：1,000 级别对象树、10,000+ 行结果、超宽列、1 MiB 以上字段、多标签并行查询和取消。

**验收**：

- 对象树按需读取并虚拟化，表大小统计不阻塞展开。
- 结果网格有行数、字段预览和内存预算；排序、过滤、比较、导出都可取消。
- UI 首帧、滚动和标签切换以可复现测量记录，不用主观“感觉流畅”作为结论。

#### DB-UX-006A：10,000 行只读结果加载和内存占用（真实窗口验收完成）

- 范围：使用本机 PostgreSQL 测试库中的 `bulk_records` 执行只读查询，最多选择 `id`、`group_id`、`status`、`amount` 和 `title`，限制为 10,000 行；不读取 `body` 或其他大字段。
- 环境：使用 Ramag `0.7.0` Debug 窗口和健康运行的 PostgreSQL 17.10 容器 `ramag-db-test-postgres`（`postgres:17-alpine`，`127.0.0.1:15432`）。
- 查询：执行 `SELECT id, group_id, status, amount, title FROM public.bulk_records ORDER BY id LIMIT 10000;`。结果区显示 `10000 行`，耗时 `91 ms`；首屏显示第 1、2 行，拖动垂直滚动条到底部后显示第 9999、10000 行，结果状态区仍显示行数和耗时。
- 内存抽样：查询前工作集为 `194,433,024 B`，私有字节为 `165,724,160 B`；查询完成后分别为 `205,066,240 B` 和 `176,730,112 B`。本次抽样增量约为 `10.1 MiB` 工作集和 `10.5 MiB` 私有字节；查询期间另一采样点为 `205,082,624 B` 工作集和 `176,791,552 B` 私有字节。
- 边界：只运行 SELECT，不读取 `body`，没有修改测试数据；容器保持运行且健康。Rust 源码未变，自动化测试不适用。本记录只确认本机窗口及这些进程内存采样点，不替代更长时间的峰值分析。

#### DB-UX-006B：1 MiB 以上字段预览和内存占用（2026-10-27 真实窗口验收完成，记录短时无响应限制）

- 范围：用 PostgreSQL 只读表达式生成一个超过 1 MiB 的文本字段，不持久化大字段；检查结果网格截断、单元格详情、复制入口和应用内存变化。
- 验收：结果可完成读取；网格预览保持有界，详情只显示当前字段；其他列和状态区不越界；记录查询耗时及查询前后的 Ramag 工作集和私有字节。
- 边界：不创建表、不写入数据、不复制整段字段到剪贴板；发现资源持续增长或窗口无响应时取消并记录。
- 环境：使用隔离的 Ramag Debug 窗口，连接 WSL Docker 中的 `ramag-db-test-postgres`（`postgres:17-alpine`，PostgreSQL `17.11`，`127.0.0.1:15432`）。测试后容器仍为 `Up` 且健康。
- 查询：执行 `SELECT repeat('x',1048577) AS large_text;`。结果为 1 行、1 列，耗时 `14 ms`。PostgreSQL 表达式只在查询结果中生成文本，没有创建表或写入数据。
- 显示：真实窗口的结果单元格只显示有界文本并以省略号截断；底部状态区仍显示字段摘要，没有越过状态栏。查看器标题为第 1 行的 `large_text`，元数据显示 `TEXT`、`1,048,577 bytes`，正文只包含当前字段并可纵向滚动。文本、CSV、JSON 和 SQL 值复制入口可见；本次没有执行复制。
- 内存：查询前 3 次工作集为 `169.48/169.29/169.82 MiB`，私有字节为 `156.01/155.81/156.34 MiB`。查看器打开后的 3 次分别为 `210.03/209.60/210.21 MiB` 和 `195.91/195.48/196.09 MiB`，比查询前均值增加约 `40.42 MiB` 工作集、`39.78 MiB` 私有字节。关闭查看器后的 3 次为 `207.45/207.90/206.76 MiB` 和 `193.13/193.58/192.43 MiB`，进程响应正常，内存仍高于查询前均值约 `37.84/36.99 MiB`。这组采样没有证明内存回收速度或峰值。
- 响应限制：关闭查看器时窗口标题短暂显示“未响应”；等待约 2 秒后，查看器关闭且窗口恢复响应。没有观察到持续增长或持续无响应。该现象和内存未回到基线均保留为后续大字段性能检查的证据。
- 剪贴板事件：定位右键菜单时误触“复制为 JSON”，本次生成的样例值短暂覆盖系统剪贴板，随后已清空；原有剪贴板内容无法恢复。之后没有再复制字段。
- 边界：只运行 SELECT，没有创建对象、修改测试数据或复制字段。Rust 源码未变，自动化测试不适用。临时连接配置保存在隔离的数据副本中；自动清理被系统策略拒绝，因此该临时目录仍保留。

#### DB-UX-006C：千级 PostgreSQL 表树按需加载和虚拟滚动（2026-10-27 真实窗口验收完成）

- 范围：在本机 PostgreSQL 测试库中创建一个唯一临时 Schema 和 1,000 张空表；使用真实窗口检查 Schema 按需加载、表数量、首尾行、筛选、滚动和表大小状态。
- 验收结果：PostgreSQL 17.11 测试容器中创建 `_ramag_db_ux_006c_20261027` 和 1,000 张空表。容器镜像为 `postgres:17-alpine`，地址为 `127.0.0.1:15432`，验收前后均为 `Up` 且健康。建表前确认同名 Schema 不存在；创建后目录计数为 `1000|table_0001|table_1000`。
- 数据库耗时：与驱动相同的表元数据查询返回 1,000 条，首尾名称正确，`pg_total_relation_size` 汇总为 0；psql 记录耗时 `68.028 ms`。这项 SQL 测量不代表 UI 总耗时。
- 真实窗口：折叠时只显示 Schema 行；展开后出现 `tables 1000`，每张空表显示 `0 B`。首次展开后的两次窗口观察在约 0.5 秒内显示完整列表。滚动视口分别显示首屏 `table_0001` 起始行、中段 `table_0760` 附近和末尾 `table_0984` 至 `table_1000`；视口只呈现当前可见行。筛选 `table_1000` 后仅剩该行。连接上下文和 SQL 工作区保持可见，进程持续响应。列表由现有 `uniform_list` 虚拟列表渲染。
- 内存：展开前 3 次工作集为 `157.56/157.80/156.87 MiB`，私有字节为 `151.32/151.56/150.64 MiB`；展开及筛选后分别为 `160.79/161.51/160.84 MiB` 和 `154.81/155.53/154.84 MiB`。抽样均值增加约 `3.64 MiB` 工作集和 `3.89 MiB` 私有字节。
- 清理：按验收边界删除唯一临时 Schema。PostgreSQL 查询确认其计数为 `0`；`public.bulk_records` 在本次测试库基线中不存在。只创建空表，没有写入表数据。容器和数据卷保持运行，未停止或重建。
- 验证边界：Rust 源码未改，自动化测试不适用。`cargo fmt` 和严格 Clippy 在本次文档提交的提交前钩子中运行。

#### DB-UX-006D：多标签查询结果隔离与单项取消（2026-10-27 部分完成，取消未验收）

- 范围：使用 PostgreSQL 只读查询，在两个查询标签中运行带不同标记的短查询和可取消查询；检查结果归属、单项取消、另一标签继续运行及后续结果状态。
- 环境：WSL 发行版 `Ubuntu-24.04`；`ramag-db-test-postgres` 使用 `postgres:17-alpine`，PostgreSQL `17.11`，端口映射为 `127.0.0.1:15432`。测试前后容器均为 `Up` 且健康，`pg_isready` 报告接受连接。未启动、停止或重建容器，也未更改数据卷。
- 标签结果：隔离 Ramag Debug 窗口连接到测试库。`TAB_A` 执行 `SELECT pg_sleep(60), 'DBUX006D_TAB_A' AS marker;`，查询历史记录两次成功返回 1 行，耗时分别为 `60,052 ms` 和 `59,950 ms`。`TAB_B` 执行 `SELECT pg_sleep(2), 'DBUX006D_TAB_B' AS marker;`，真实窗口结果表显示 1 行和 `DBUX006D_TAB_B`，查询历史耗时为 `2,006 ms`。后续调试中，`TAB_A2` 的 240 秒查询在约 `240,117 ms` 和 `240,118 ms` 返回；真实窗口显示 `DBUX006D_TAB_A2_20261027`。本轮重新验收时，`TAB_A3` 的 60 秒查询返回 1 行，窗口耗时为 `60,926 ms`，标记为 `DBUX006D_TAB_A3_20261027`；`TAB_A4` 的 120 秒查询返回 1 行，窗口耗时为 `122,014 ms`，标记为 `DBUX006D_TAB_A4_20261027`。`TAB_B3` 已设置唯一 SQL 标记，但没有执行。各次查询均未在另一标签的长查询仍执行时确认短查询结果，因此并行结果隔离未验收。
- 取消状态：此前的 `TAB_A`、`TAB_A2`、本轮 `TAB_A3` 和 `TAB_A4` 均自然完成。`TAB_A4` 执行约 78 秒时，`pg_stat_activity` 显示两条匹配该标记的活动记录；查询自然完成后，活动记录数为 0。真实窗口曾显示 `TAB_A4` 正在执行，但没有取得服务端确认取消的结果。自然完成不能作为取消证据，因此单项取消仍未验收，本切片保持部分完成。
- 边界和清理：仅执行 `SELECT` 与 `pg_sleep`，没有创建对象或写入数据。关闭隔离调试窗口；保留临时测试数据目录 `C:\Users\likanug\AppData\Local\Temp\codex-ramag-dbux006d-20261027-01a17345-run2`。PostgreSQL 容器与数据卷继续运行，容器健康且接受连接；服务端没有遗留匹配 `TAB_A4` 的活动查询。
- 下一步：重新验收 `DB-UX-006D`。两个标签使用不同标记。先确认长查询仍在执行，并确认停止按钮属于该长查询标签；再运行短查询并立即停止长查询。记录两个标签的结果、查询历史和停止耗时，并检查服务端活动查询。若长查询自然结束，该次运行不作为取消证据。
- 回滚边界：只涉及本节验收记录；不修改数据库对象、数据、WSL 配置、容器或数据卷。

## 6. 驱动边界

- **MySQL/PostgreSQL**：优先实现截图中的 SQL 查询、表格编辑、事务、DDL、EXPLAIN 和 Schema 工作流；分页、排序、生成列、Identity、外键动作按方言适配。
- **MongoDB**：使用文档查询和文档编辑语义，结果可以使用网格但不伪造 SQL 事务或表结构。
- **Redis**：使用 Key-Value/集合/流专用视图，不显示无意义的 Schema、列或 SQL 迁移入口。
- 新驱动先声明能力，再决定渲染哪些按钮；不支持的动作显示原因，不渲染点击后必然失败的入口。

## 7. 测试和环境记录

目标 Rust 测试之外，涉及真实数据库时使用 `scripts/db-test/db-test.sh`：

| 服务 | 镜像基线 | 默认端口 | 说明 |
|---|---|---:|---|
| MySQL | `mysql:8.4` | `127.0.0.1:13306` | 当前 SQL 写入、元数据和迁移基线 |
| PostgreSQL | `postgres:17-alpine` | `127.0.0.1:15432` | 当前 SQL 查询、事务和迁移基线 |
| Redis | `redis:7-alpine` | `127.0.0.1:16379` | Key-Value 专项测试 |
| MongoDB | `mongo:8.2` | `127.0.0.1:27018` | 文档查询专项测试 |

记录 `up`/`status`/`test`/`down` 或 `clean` 的实际结果；未启动服务时明确写未完成，不用 mock、静态 fixture 或仅编译结果代替集成证据。

每个切片至少运行目标 crate 测试、headless GPUI 验收、`git diff --check`、`cargo fmt --all -- --check` 和 `cargo clippy --workspace --all-targets -- -D warnings`。涉及数据库 UI 时，另外记录真实窗口证据或 Computer Use 不可用的具体限制。

## 8. 交付边界

本文件只定义数据库工作区的实现顺序和验收条件；平台壳层、跨工具迁移、分支和提交规则以 [`02-development-roadmap.md`](02-development-roadmap.md) 为准。旧版本的详细实现记录不在本文件重复维护，统一从归档入口追溯。
