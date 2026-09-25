# 数据库工作区主线：DataGrip 风格核心闭环

> 状态：现行专项路线图
> 更新日期：2026-09-25
> 共同视觉与交互基线：[`ui-acceptance-standard.md`](ui-acceptance-standard.md)
> 跨工具顺序：[`development-roadmap.md`](development-roadmap.md)
> 历史实现、待办和验收记录：[`archive/2026-09-25-pre-datagrip-rebaseline/database-client-datagrip-roadmap.md`](archive/2026-09-25-pre-datagrip-rebaseline/database-client-datagrip-roadmap.md)

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
| `DB-RED-07` | `DDL` 控件 | 提供当前对象的 DDL 查看/复制/刷新入口，并明确只读预览与执行/迁移动作边界 | 从表或查询上下文打开 DDL；DDL 内容可滚动、复制；执行前显示目标连接和风险并要求确认，失败保留原文和错误 |

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

#### DB-RED-06：`Tx: Auto` 事务控件（当前切片）

对齐截图工具栏中的 `Tx: Auto` 下拉入口，明确自动提交、手动事务、处理中和错误状态，并把开始、提交、回滚和未提交修改反馈收束到同一会话上下文。复用现有驱动能力声明、事务事件和保存点，不改写 SQL 执行或 DML 生成。

### DB-UX-003：结果数据网格（覆盖 `DB-RED-05`）

**范围**：虚拟化行、稳定表头、行号、列类型图标、列宽、排序、列过滤、`WHERE`/`ORDER BY` 条件条、双轴滚动、分页、单元格详情、复制和导出。

**验收**：

1. 大于当前页的数据只读取并显示有界页，底部明确显示当前范围/总数状态；精确总数不可用时不伪造页码。
2. 列总宽超过视口时可以滚到最后一列；行数超过视口时滚动条、最后一行和分页状态不重叠。
3. NULL、空字符串、二进制、长文本和截断值有不同状态；打开详情只读取当前单元格，不复制整个结果集。
4. 排序/筛选重新加载后保留或明确重置页码、横向位置、连接和 Schema 上下文。

### DB-UX-004：安全编辑与事务（覆盖 `DB-RED-06`）

**范围**：单元格/新增行编辑、未提交标记、批量提交/撤销、影响行数、错误明细、事务开始/提交/回滚/保存点和刷新。

**验收**：

1. 没有可靠定位键、驱动不支持安全写入或目标为只读连接时，写入入口禁用并说明原因。
2. 写入失败保留用户输入和数据库错误，不把部分成功显示成全失败；提交、回滚和切换标签的状态互斥。
3. MySQL/PostgreSQL 在本机 Docker 中验证提交、回滚、取消、连接失效和属性回读；MongoDB/Redis 单独验证文档或 Key-Value 语义。

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

本文件只定义数据库工作区的实现顺序和验收条件；平台壳层、跨工具迁移、分支和提交规则以 [`development-roadmap.md`](development-roadmap.md) 为准。旧版本的详细实现记录不在本文件重复维护，统一从归档入口追溯。
