# Ramag Platform 后续开发计划

更新日期：2026-09-25
状态：已保存；R6、R7 已完成验证，R7 随本次独立提交推送。
适用范围：在现有主线路线图和代码审查修复计划基础上，继续收尾未提交改动、修复已知问题并完善已有功能。

## 术语表与命名约定

| 规范中文名 | English / Acronym | 当前计划中的职责边界 | 不代表什么 |
|---|---|---|---|
| 开发切片 | Delivery Slice | 可以独立实现、验证、提交、推送和回滚的最小功能范围 | 不表示一次包含多个无关问题 |
| 交互回归 | Interaction Regression | 使用 headless GPUI 测试模拟输入、点击、焦点和布局 | 不表示真实 Windows 窗口验收 |
| 本机 Docker 集成测试 | Local Docker Integration Test | 使用本机 Docker 服务验证协议、数据库或外部服务适配 | 不表示远程集群或生产验证 |
| 真实窗口证据 | Native Window Evidence | 通过 Windows 原生窗口截图和输入证明用户可见流程 | 不表示仅通过编译或单元测试 |
| 目标分支 | Target Branch | 默认直接接收已验证提交的 `main`；只有用户明确指定功能分支时，才在验证后合并回 `main` | 不表示可以绕过验证直接合并 |

## 一、总体目标

以收尾现有变更、修复可复现问题、完善已有功能为主线。每次只推进一个可独立验收的切片；完成对应验证后再提交和推送。未经验证的代码、UI 或集成结果不得标记为完成。

现有详细审查项见 [`code-review-remediation-plan-2026-09-23.md`](code-review-remediation-plan-2026-09-23.md)，跨工具排期见 [`development-roadmap.md`](development-roadmap.md)。本文件只记录执行顺序和交付要求，不重新定义专项协议细节。

## 二、阶段安排

### 阶段 1：核对现状与收敛未提交改动

- 核对当前分支、远程跟踪关系、worktree 和未提交差异。
- 按功能拆分未提交改动，标记已完成、待验证、未完成和阻塞项。
- 记录每项问题的代码位置、复现步骤、影响范围、验收条件和测试命令。
- 保留无关改动，不覆盖用户已有内容；不在没有证据时推断问题已解决。
- 验收条件：确定首个切片的边界，并确认不会把无关改动带入提交。

### 阶段 2：完成现有未提交功能

- 优先收尾已经有实现但缺测试或错误处理的功能。
- 补齐异常、失败、取消、重试和状态反馈路径。
- 对用户可见功能先完成 headless 交互验收；真实窗口可用时补原生截图和输入证据。
- 验收条件：功能可以独立运行和回归，必要测试通过，未完成限制已记录。

### 阶段 3：修复已知问题

- 优先处理数据丢失、崩溃、敏感信息泄露和单实例/并发安全问题。
- 其次处理结果错误、操作无响应、取消失效、资源未释放和状态覆盖。
- 每个问题先建立复现证据，再实现修复和回归测试。
- 验收条件：修复前测试能够暴露问题，修复后通过，相关流程没有回退。

### 阶段 4：完善已有功能

- 从现有队列中选择依赖满足、用户价值明确的最小功能。
- 覆盖正常流程、空状态、失败反馈、重试、取消和边界尺寸。
- 需要外部服务时使用本机 Docker，并记录服务、镜像版本、端口、启动和清理状态。
- 验收条件：正常及异常流程通过，headless 与真实服务证据边界清楚。

### 阶段 5：优化、集成与清理

- 只根据可复现测量优化性能；记录优化前后结果。
- 默认直接在最新 `main` 上开发和推送；没有特殊说明时不创建新的分支或 worktree。
- 用户明确指定功能分支时，先基于最新 `main` 创建；分支验证通过后合并到 `main`，再验证并推送 `main`。
- 目标分支已推送后，确认源分支无未合并/未推送提交且无关联 worktree，再删除本次明确合并的源分支。
- 不删除 `main` 或没有明确纳入本次合并的分支；远程分支受保护或删除失败时保留并报告原因。

## 三、当前执行顺序

1. 已建立本文件及 `docs/code-review-remediation-plan-2026-09-23.md` 的审查基线。
2. R5 请求搜索清除按钮交互回归（`58675ea2`）、R11 workspace Clippy 修复（`480fc9b1`）、R1 工作区读取失败处理（`de232c7b`）和 R2 保存生命周期隔离（`fde33b9f`）已随 `dev` 的整合提交进入 `main`。
3. R3 Collection 历史写入失败处理（`1204b397`）和 R4 MySQL 8.4 基线校正（`15513a44`）已先在原开发流程中验证，随后由 `9c14fa7a` 将已验证的 `dev` 整合到 `main`；`origin/main` 已完成推送。
4. 已确认 `dev`、`feat/r6-mysql-generated-columns` 和 `fix/api-search-clear-regression` 没有未合并/未推送提交或关联 worktree，随后删除本地引用；远程 `origin/dev` 也已删除，当前仅保留 `main`/`origin/main`。R6 表设计器 DDL 安全性已完成实现和验证并推送；R7 已完成 SQLite 非空表新增必填字段保护，下一项按专项计划实施 R8、R9、R10。

已完成事项及证据以本文件“切片执行记录”和 [`code-review-remediation-plan-2026-09-23.md`](code-review-remediation-plan-2026-09-23.md) 的执行记录为准，不再把已进入 `main` 的改动列作待办。

## 四、每个切片的执行顺序

1. 先写清问题、设计、改动范围、不做事项和验收条件。
2. 实现代码、必要注释、测试和文档；新建或修改文本文件统一使用 LF。
3. 运行与风险匹配的目标测试和验收，记录命令、结果、环境和限制。
4. Rust workspace 从根目录针对最终待提交内容依次通过：
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
5. 测试通过后使用单一功能范围的 Conventional Commit，并立即推送当前开发分支。
6. 默认不创建分支；用户明确要求分支时，先验证功能分支，再合并到 `main`，重新验证并推送 `main`，最后清理本次明确合并的源分支。

## 五、环境和验证要求

- 集成测试只使用本机 Docker；必须记录服务名、镜像版本、端口、启动状态和清理状态。
- 数据库基线为 PostgreSQL 17+ 和 MySQL 8.4+；低版本旧记录必须标为历史基线，不能作为当前完成证据。
- UI 验收优先使用真实窗口；只能使用 headless 时，明确记录真实窗口限制，不把 headless 结果描述为原生窗口验收。
- 错误消息、历史、日志和测试输出不得包含密码、Token、完整请求正文或证书私钥。
- Docker、真实窗口或远程 CI 不可用时，记录为未完成或环境限制，不用 mock、静态 fixture 或仅编译结果替代。

## 六、任务记录模板

每项切片在本文件或对应专项文档中记录：

- 问题证据与优先级；
- 设计和改动范围；
- 验收条件与测试命令；
- 测试环境、服务版本、端口和清理结果；
- UI、真实服务和远程验证的证据边界；
- 未完成项或阻塞项；
- 提交编号、推送结果、合并状态和分支清理结果。

## 七、切片执行记录

### R3：Collection 历史写入失败时保留已执行结果（2026-09-24）

- 设计：将 HTTP/gRPC 驱动执行结果与历史持久化结果分开报告；Storage 写入失败时保留本次 outcome 和当前界面响应，停止启动下一条 Collection 请求，并通过固定提示说明历史未保存。新增字段带 serde 默认值以兼容旧数据。
- 改动范围：API 服务、领域结果、API 工作台状态与提示、Collection Docker UI 回归测试；没有扩大重试或重复发送请求。
- 验收：`cargo test --locked -p ramag-tool-api --all-targets`（29 项通过，含本机 Docker HTTP/gRPC 请求和 headless UI）；`cargo test --locked -p ramag-app --all-targets`（224 项单元测试及集成测试通过）；`cargo test --locked -p ramag-domain --lib api -- --nocapture`（27 项通过）；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、源码尺寸脚本及 `git diff --check` 均通过。
- Docker 环境：`ramag-api-http-test`，镜像 `ramag-api-http-test:python-3.12.11-alpine-3.22`，端口 `127.0.0.1:18089->8080`、`127.0.0.1:18091->8443`，状态 healthy；`ramag-api-grpc-test`，镜像 `ramag-api-grpc-test:rust-1.91.0-bookworm`，端口 `127.0.0.1:18090->50051`、`127.0.0.1:18092->50052`，状态 healthy。两项服务在本次执行前已经运行；本次未启动、停止或清理容器，完成后仍保持运行。
- UI 证据：headless GPUI 点击响应“断言”页签后验证断言与 Collection 汇总内容；未完成真实 Windows 窗口操作，因此不作为原生窗口验收证据。
- Git：代码提交 `1204b397` 已推送至原功能分支，随后随 `dev` 整合进入 `main`；`main` 已复验并推送，原功能分支已清理。

### R4：MySQL 8.4 基线文档校正（2026-09-24）

- 设计：将当前可执行基线与历史实测记录分开陈述；保留 MySQL 8.0 原始历史事实，但明确标注其不符合当前 MySQL 8.4+ 要求，不得作为当前验收证据。
- 改动范围：性能报告、跨工具开发路线图、数据库专项路线图和代码审查执行记录；核对 `scripts/db-test/compose.yaml`、README、CI 和测试脚本，未改动运行时配置。
- 验收：已检查 MySQL 8.0 命中均为带历史基线说明的记录或待办描述；README、CI 与 scripts 未发现低于 MySQL 8.4 的当前镜像或测试命令。`git diff --check`、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` 和源码尺寸检查均通过。
- Docker：本切片仅校正文档，没有运行集成测试，也未启动、停止或清理 Docker 服务。已静态核对 Compose：MySQL `mysql:8.4` / `127.0.0.1:13306`、PostgreSQL `postgres:17-alpine` / `127.0.0.1:15432`、Redis `redis:7-alpine` / `127.0.0.1:16379`、MongoDB `mongo:8.2` / `127.0.0.1:27018`；`db-test.sh` 使用 `docker compose up --detach --wait` 启动，普通停止保留数据卷，清理命令会删除数据卷和本地测试凭据。此切片未运行这些命令。
- Git：提交 `15513a44` 已在原文档分支验证并推送，通过 `e247d884` 合并到 `dev`，再由 `9c14fa7a` 整合到 `main`；目标检查通过后 `origin/main` 已推送。已确认源分支无未合并/未推送提交且无关联 worktree，随后删除本地和远程引用。

### R6：MySQL 字段修改保留生成属性（2026-09-25）

- 设计：MySQL `CHANGE COLUMN` 必须保留可安全表达的 `AUTO_INCREMENT`、生成表达式和 `VIRTUAL`/`STORED` 存储属性；当前字段模型无法完整表达的身份元数据或生成属性直接拒绝生成 SQL，避免静默降级。
- 改动范围：`crates/ramag-tool-dbclient/src/views/table_designer/sql.rs` 保留字段生成属性并拒绝不完整元数据；`crates/ramag-infra-mysql/tests/column_metadata.rs` 增加 MySQL 元数据回读和变更验证；表设计器单元测试拆分到独立模块以保持源码尺寸限制。没有扩大到 R7 的 SQLite 非空表迁移。
- 验收：`cargo test --locked -p ramag-tool-dbclient --lib table_designer -- --nocapture`（21 项通过）；`cargo test --locked -p ramag-tool-dbclient --lib`（320 项通过）；`cargo test --locked -p ramag-infra-mysql --test column_metadata -- --nocapture`（2 项通过）；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、源码尺寸检查和 `git diff --check` 均通过。
- Docker 环境：本机容器 `ramag-r6-mysql84` 使用 `mysql:8.4`，绑定 `127.0.0.1:13316->3306`；测试期间启动并运行集成测试，完成后执行 `docker rm -f ramag-r6-mysql84`，容器和临时卷均已确认不存在。
- UI 证据：Computer Use 原生应用接口仅返回浏览器运行时且应用列表为空，排查后无法完成真实窗口交互；已使用系统截图 `artifacts/ui-screenshots/r6-system-fallback-window.png` 和 headless 表设计器测试作为替代证据。截图只证明 Ramag 窗口可启动，不证明真实表设计器导航或点击流程，原生窗口覆盖范围仍未完成。
- Git：R6 以单一 Conventional Commit 提交并推送 `main`；提交和远程状态以当前 `main`/`origin/main` 为准，R7-R10 继续保持独立切片。

### R7：SQLite 非空表新增必填字段保护（2026-09-25）

- 设计：表设计器在生成 SQLite `ADD COLUMN ... NOT NULL` 前，异步执行限定为首行探测的 `SELECT 1 ... LIMIT 1`。确认表为空时允许无默认值的必填字段；确认表已有数据时要求默认值或允许 `NULL`；探测未知或失败时拒绝生成危险 SQL。
- 改动范围：`TableDesignerConfig` 和 `TableDesigner` 保存行存在性证据；表树打开设计器时为 SQLite 连接启动行探测；新增字段 SQL 保护、headless 表设计器测试和 SQLite 驱动实际执行测试。没有生成重建表迁移，也没有扩大到 R8 消息处理。
- 验收：`cargo test --locked -p ramag-tool-dbclient --lib table_designer -- --nocapture`（25 项通过）；`cargo test --locked -p ramag-infra-sqlite --lib -- --nocapture`（5 项通过）；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、源码尺寸检查和 `git diff --check` 均通过。
- SQLite 测试环境：使用本机临时 SQLite 文件，不依赖 Docker；分别验证空表新增必填字段成功、非空表无默认值被 SQLite 拒绝、非空表带默认值成功，并回读列元数据确认约束和默认值。
- UI 证据：Computer Use 原生应用接口在启动 Ramag 后仍返回空应用列表，无法完成真实窗口交互；已使用系统截图 `artifacts/ui-screenshots/r7-ramag-window-fallback.png` 和 headless 表设计器测试替代。截图只证明新构建可启动，不证明真实表设计器导航、行探测等待或点击流程。
- Git：R7 以单一 Conventional Commit 提交并推送 `main`；R8-R10 继续保持独立切片。
