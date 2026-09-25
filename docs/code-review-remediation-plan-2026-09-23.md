# 代码审查与修复优化计划（2026-09-23）

> 状态：R1-R8、R11 已进入已验证、已推送的 `main`；R3、R4 以及本次临时源分支均已清理；R9-R10 尚待实施。
>
> 范围：本计划基于当前 `main` / `v0.2.0` 基线，重点检查近期 API 工作台、数据库工作台、MQTT 工作台、启动生命周期和本地集成测试维护。计划只安排可复现、可独立验收的修复；不把真实 Windows 窗口证据或新的协议能力混入同一次提交。

## 术语表与命名约定

| 规范中文名 | English / Acronym | 当前计划中的职责边界 | 不代表什么 |
|---|---|---|---|
| 代码审查 | Code Review | 依据源码、测试和现有记录识别缺陷、回归风险与验证缺口 | 不代表已经完成修复 |
| 交互回归 | Interaction Regression | 使用 GPUI headless 测试模拟点击、输入、滚动并检查状态和布局 | 不代表真实 Windows 窗口验收 |
| 本机 Docker 集成测试 | Local Docker Integration Test | 使用本机 Docker 服务验证协议、数据库或外部服务适配 | 不代表远程集群或生产服务验证 |
| 请求代次 | Request Generation | API 工作台用来拒绝迟到异步结果的递增标识 | 不代表网络请求 ID 或持久化记录 ID |
| 配置上下文 | Profile Context | MQTT 工作台用来隔离配置切换前后异步结果的状态标识 | 不代表 MQTT 长连接或 Broker 会话 |
| 真实窗口证据 | Native Window Evidence | 在 Windows 原生窗口中通过截图和输入证明用户可见流程 | 不代表 headless 渲染测试结果 |

## 1. 审查结论

当前基线整体可维护，近期已经主动补充了大量紧凑窗口和异步上下文回归测试；没有在本轮审查中发现可以直接确认的 P0 数据破坏或凭据泄露问题。主要风险集中在四类：

1. API 工作台把异步读取失败静默降级为空工作区，用户无法区分“没有保存数据”和“读取失败”。
2. API 工作台的保存回调没有请求代次保护，启动读取或其他异步工作区更新迟到时可能覆盖新状态。
3. API Collection 的历史写入错误会通过 `?` 直接结束整个集合，UI 收不到已完成结果汇总，缺少明确的持久化失败策略。
4. 审查时数据库测试与文档残留 MySQL 8.0 表述，而仓库规则已要求 MySQL 8.4+；R4 已在本计划执行记录中完成文档校正，原历史数据不再被描述为当前验收证据。
5. 审查时表设计器生成的 MySQL `CHANGE COLUMN` 定义没有保留 `AUTO_INCREMENT` 等生成属性，SQLite 对已存在数据的必填新增字段也没有在预览阶段拒绝；R6、R7 已按计划修复并完成对应数据库验证。
6. 审查时 MQTT 订阅消息进入有界 UI 队列后遇到背压会静默丢弃，SSH 远程覆盖提交在新文件已替换成功后可能仍报告失败，Linux 单实例旧 Socket 清理存在并发竞态；R8 已修复，R9、R10 仍待实施。
7. R11 的 workspace Clippy 基线问题已修复；后续切片仍必须在提交前通过统一 workspace 检查。

本轮还确认一个验证缺口：API 搜索清除按钮已有实现，但现有测试通过直接写入空字符串验证恢复列表，没有真正模拟清除按钮点击或焦点回归。因此它应作为第一项交互回归切片，而不是继续把现有“已验证”记录当作完整交互证据。

## 2. 发现项与证据

### R1：API 工作区读取失败被静默当成空状态（P1）

- 位置：`crates/ramag-tool-api/src/view.rs` 的 `load_saved_workspace`。
- 现状：`list_workspaces()` 使用 `result.ok()`；失败或无工作区都会直接返回，页面继续显示默认空工作区。
- 影响：Storage 损坏、权限错误或迁移失败时，用户可能误以为请求丢失并继续保存，增加覆盖或误操作风险；日志和界面都没有可操作的失败提示。
- 修复方向：保留失败分支，设置有界、脱敏的错误提示；区分“无工作区”和“读取失败”；失败时禁用覆盖性保存或要求用户显式重试。
- 验收：Storage 注入失败、空列表和成功加载各有测试；错误正文不包含密钥、Token、请求正文；UI headless 测试验证提示、重试和保存按钮状态。

### R2：API 保存异步回调缺少代次隔离（P1）

- 位置：`crates/ramag-tool-api/src/operations.rs` 的 `save`。
- 现状：发送、Collection 和 gRPC 发现使用 generation/cancellation；保存只设置 `saving`，回调返回后无条件写入 `workspace`、`active_request_id` 和 `notice`。
- 影响：保存完成前若启动读取、导入或其他异步工作区更新迟到，较旧的 Storage 结果可能覆盖较新的编辑状态。当前按钮通常会禁用重复保存，因此主要风险是启动加载与用户保存交错，而不是重复点击本身。
- 修复方向：新增保存代次和取消标记，或把保存结果与 workspace ID/草稿版本绑定；回调仅接受当前代次，失败也不能清除较新的提示。
- 验收：延迟 Storage 驱动下连续保存、保存与导入交错、旧成功/旧失败迟到三组测试；最终视图只接受最新代次。

### R3：Collection 历史写入错误缺少部分结果策略（P2）

- 位置：`crates/ramag-app/src/usecases/api_service_collection.rs` 的 `run_collection`。
- 现状：`execute_record` 已将协议驱动错误转换为失败 outcome，但每次 outcome 的历史写入失败仍由 `execute_record(...).await?` 直接上抛；Collection 不返回已经完成的部分汇总，也不区分“请求失败”和“历史持久化失败”。
- 影响：Storage 短暂不可写时，用户只看到 Collection 总体失败，无法确认哪些请求已经执行或历史是否已保存；重试可能重复触发外部 API。
- 修复方向：明确持久化失败策略：保留部分 outcome 和停止原因，或在领域层提供“执行结果已完成、历史写入失败”的独立状态；不能为了显示完整而无限重试或重复发送请求。
- 验收：协议失败继续产生失败 outcome；历史写入失败时 UI 显示明确的持久化错误和已完成数量；取消仍停止后续请求；敏感错误继续脱敏。

### R4：MySQL 版本基线和证据不一致（P1，已修复）

- 位置：`docs/performance.md`、`docs/development-roadmap.md`、`docs/database-client-datagrip-roadmap.md` 及相关验收记录。
- 审查时现状：仓库 Compose 已使用 MySQL 8.4，但多处文档和历史验证记录仍写 MySQL 8.0；全局规则要求 MySQL 8.4+。
- 影响：读者无法判断当前测试是否满足最低版本要求；复制旧命令可能启动不符合基线的服务，导致行为和性能结论不可比。
- 修复结果：当前 Compose 已核实使用 MySQL 8.4；相关路线图和性能报告明确 MySQL 8.4+ / PostgreSQL 17+ 当前基线，历史 8.0 结果保留原值并标注为旧基线，不作当前验收证据。
- 验收：README、CI 与 scripts 未发现低于 MySQL 8.4 的当前镜像或测试命令；全文检索命中均为已标注历史记录、计划中的审查说明或非版本基线的官方文档 URL。详情与检查命令见第 2.2 节 R4 记录。

### R5：API 搜索清除交互缺少真实点击与焦点回归（P2）

- 位置：`crates/ramag-tool-api/src/grpc_catalog_tests.rs` 的请求侧栏筛选测试。
- 现状：测试把输入状态直接设置为空字符串，只验证列表恢复和清除按钮消失。
- 影响：按钮命中区域、Change 事件派发和 `InputState::focus` 可能回归而测试仍通过；这正是近期共享 `cleanable_input` 改动的关键行为边界。
- 修复方向：使用 `gpui_kit::test::TestWindowExt::click` 点击 `api-request-search-clear`，断言搜索值为空、列表恢复、按钮消失且输入焦点回到搜索框。
- 验收：目标 headless 测试通过；360/640px 至少有一组边界检查；Computer Use 可用时补原生窗口证据，不可用时明确记录限制。

### R6：MySQL 字段修改可能丢失自增属性（P1，已修复）

- 位置：`crates/ramag-tool-dbclient/src/views/table_designer/sql.rs` 的 `mysql_field_sql` 和 `mysql_definition`。
- 现状：字段发生任意变化时使用 `CHANGE COLUMN`，但新定义只生成类型、可空、默认值和注释，没有根据原字段元数据保留 `AUTO_INCREMENT` 或其他生成表达式属性。
- 影响：用户只修改注释或类型时，执行预览 SQL 可能把自增列变成普通列；后续插入可能失败或产生错误的主键分配。现有测试构造的字段都没有自增/生成属性，未覆盖该风险。
- 修复方向：为 MySQL 生成定义时显式保留可安全重放的生成属性；对无法在当前编辑器中安全表达的属性拒绝生成 SQL，并提示使用完整 DDL/重建表流程。主键索引是否由数据库保留必须用 MySQL 8.4 Docker 实测确认，不凭静态字符串推断。
- 验收：自增主键仅修改注释、类型或可空性的 SQL 回读测试；本机 MySQL 8.4 执行前后读取列元数据，确认属性不变；非法或无法映射的生成属性有明确错误。
- 修复结果（2026-09-25）：`CHANGE COLUMN` 现在保留 `AUTO_INCREMENT`、生成表达式和 `VIRTUAL`/`STORED` 属性，并对不完整生成元数据和不支持的身份元数据返回明确错误；MySQL 8.4 Docker 回读测试已确认变更前后属性一致。

### R7：SQLite 已有数据表新增必填字段会生成不可执行 SQL（P1，已修复）

- 位置：`crates/ramag-tool-dbclient/src/views/table_designer/sql.rs` 的 `sqlite_field_sql`。
- 现状：新增字段直接生成 `ALTER TABLE ... ADD COLUMN ... NOT NULL`，没有知道表是否为空，也没有要求默认值。
- 影响：SQLite 对非空表新增无默认值的 `NOT NULL` 字段会拒绝执行；用户在预览阶段看到可执行样式的 DDL，点击执行才失败，且没有迁移重建指导。
- 修复方向：在设计器上下文提供表行数/空表信息，或保守地拒绝“无默认值的非空新增字段”；需要保留该能力时生成有界的重建表迁移方案并明确风险。
- 验收：本机 SQLite 空表和非空表分别验证；非空表必填无默认值在预览阶段被拒绝；有默认值、可空字段和空表场景生成并执行成功。
- 修复结果（2026-09-25）：SQLite 打开表设计器时先执行 `SELECT 1 ... LIMIT 1` 探测是否存在数据；空表允许无默认值的必填字段，非空表要求默认值或允许 `NULL`，探测失败或未知时拒绝生成危险 SQL。headless 测试和 SQLite 驱动实际执行测试均已通过。

### R8：MQTT 订阅背压会静默丢消息（P1，已修复）

- 位置：`crates/ramag-tool-mqtt/src/mqtt_view/mqtt_operations.rs` 的 32 条消息队列，以及 `crates/ramag-infra-mqtt/src/native/mqtt_data_plane.rs` 的 MQTT 3.1.1/5 发布处理。
- 现状：UI 接收队列满时返回 `Backpressured`；两个 Native 数据面只对 `Closed` 停止，对 `Backpressured` 继续读取并丢弃当前消息，没有计数、状态提示或可选暂停策略。
- 影响：高吞吐订阅期间用户看到的消息列表可能缺少消息，却无法知道丢失发生；这会影响调试、审计和问题复现。
- 修复方向：明确产品策略：至少记录有界丢弃计数并在 UI 显示“已丢弃 N 条”，或让数据面在背压时暂停读取/断开订阅；不能静默把 `Backpressured` 当作成功。
- 验收：人为阻塞 UI 接收方并填满队列，确认背压被记录、状态可见且不会误报完整接收；MQTT 3.1.1 和 MQTT 5 共用同一策略。
- 修复结果（2026-09-25）：选择暂停读取策略；两个 Native 协议路径在 sink 返回 `Backpressured` 时保留原消息并重试，取消或接收端关闭时安全退出，不再静默丢弃。

### R9：SSH 远程覆盖提交的结果状态可能与目标文件不一致（P2）

- 位置：`crates/ramag-infra-ssh/src/transfer/commit.rs` 的 `commit_remote`。
- 现状：临时文件已重命名为目标文件后，再删除旧文件备份；备份删除失败会返回 `Err`，上层因此把已经替换成功的传输显示为失败。
- 影响：用户可能点击重试，导致重复上传或覆盖；备份残留也可能长期占用远程空间。
- 修复方向：把“目标替换成功”和“备份清理失败”分成不同结果；成功替换后返回成功并附带可观测清理告警，或返回结构化的部分成功状态供 UI 明确提示，不得伪装成可安全重试的全失败。
- 验收：SFTP 测试替身只拒绝备份删除，确认目标内容为新文件、结果状态为部分成功/成功带告警；替换失败仍验证回滚。

### R10：Linux 单实例旧 Socket 清理存在并发竞态（P1）

- 位置：`crates/ramag-bin/src/single_instance_linux.rs` 的 `acquire_at` 与 `remove_stale_socket`。
- 现状：两个进程都连接旧 Socket 失败后，分别检查“路径是 Socket”并删除；进程 A 删除后绑定新监听，进程 B 仍可能删除 A 的活动 Socket，再绑定自己的监听。
- 影响：两个 Ramag 进程都可能继续作为主实例，破坏单实例保护并引发共享 Storage 或窗口状态竞争。
- 修复方向：清理前后使用不可替换的身份校验/绑定竞态保护；优先使用原子占用、锁文件或能确认监听者仍不存在的流程，避免“检查后删除”跨越无保护窗口。
- 验收：并发启动压力测试覆盖两个进程同时发现旧 Socket、A 已重新绑定而 B 尚未删除的交错；最终最多一个进程成为 Primary，另一个必须 Secondary 或明确失败。

### R11：workspace Clippy 基线失败（P1，发布前检查阻塞）

- 位置：`crates/ramag-infra-container-docker/src/windows_ssh.rs:26-34`。
- 现状：`Docker::connect_with_custom_transport(...)` 外层使用 `Ok(...?)`，在 `-D warnings` 下触发 `clippy::needless_question_mark`；本次文档审查未修改该代码。
- 影响：即使后续修复只涉及文档或其他 crate，统一 workspace Clippy 仍会失败，无法按仓库规则提交或推送经过完整质量检查的变更。
- 修复方向：移除多余的 `Ok`/`?`，补目标 crate 回归并重新运行 workspace Clippy；不得使用 `#[allow]` 绕过检查。
- 验收：`cargo clippy --workspace --all-targets -- -D warnings` 通过，并确认差异只包含该独立质量修复。

## 2.1 本轮审查与验证记录

- 审查基线：`dev` 与 `origin/dev` 均指向 `229127c7`（`v0.2.0`），工作区初始无未提交业务改动。
- 通过：`cargo test --locked -p ramag-tool-dbclient --lib table_designer -- --nocapture`，18 项通过；该结果同时证明现有测试没有覆盖 R6/R7 的关键元数据和非空表执行边界。
- 通过：`cargo test --locked -p ramag-tool-api --all-targets api_request_sidebar_filters_all_saved_requests_with_collection_context -- --nocapture`，1 项通过；该测试仍通过直接写入空值，未覆盖 R5 的真实清除按钮点击和焦点回归。
- 通过：`cargo fmt --all -- --check`、`git diff --check`。
- 未通过：`cargo clippy --workspace --all-targets -- -D warnings`，被 R11 的既有 `clippy::needless_question_mark` 阻塞；本轮没有修改该运行时代码。
- UI 证据：本轮没有真实 Windows 窗口操作；Computer Use 返回空应用列表，因此不能把现有 headless 结果描述为原生窗口验收。
- 集成证据：本轮未启动或修改 Docker 服务；后续计划中的协议/数据库集成测试必须按本机 Docker、镜像版本、端口和清理状态单独记录。

## 2.2 修复执行记录（截至 2026-09-25）

- R5 已完成：`58675ea2 test: cover api request search clear interaction` 已随整合提交进入 `main`，补充清除按钮交互回归。此项提供 headless 证据，不代表真实 Windows 窗口验收。
- R11 已完成：`480fc9b1 fix: remove redundant docker transport question mark` 已随整合提交进入 `main`；workspace Clippy 当前通过。
- R1 已完成：`de232c7b fix: surface API workspace load failures` 已随整合提交进入 `main`。
- R2 已完成：`fde33b9f fix(api): protect workspace save lifecycle` 已随整合提交进入 `main`。
- R3 已完成代码与分支验证：`1204b397 fix(api): preserve collection results on history failure` 已推送到 `origin/feat/api-collection-results`。历史写入失败时保留已完成结果和当前响应，停止后续请求并显示不含敏感数据的提示；新增兼容旧数据的 serde 默认字段。失败注入测试确认仅失败前后实际执行的请求计入结果，不因存储错误重发请求。
- R3 验证通过：`cargo test --locked -p ramag-tool-api --all-targets`（29 项）；`cargo test --locked -p ramag-app --all-targets`（224 项单元测试及集成测试）；`cargo test --locked -p ramag-domain --lib api -- --nocapture`（27 项）；`cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets -- -D warnings`；`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/check-source-size.ps1`；`git diff --check`。
- R3 Docker 环境：HTTP 服务 `ramag-api-http-test` 使用 `ramag-api-http-test:python-3.12.11-alpine-3.22`，映射 `127.0.0.1:18089->8080` 和 `127.0.0.1:18091->8443`；gRPC 服务 `ramag-api-grpc-test` 使用 `ramag-api-grpc-test:rust-1.91.0-bookworm`，映射 `127.0.0.1:18090->50051` 和 `127.0.0.1:18092->50052`。执行期间两者均为 healthy，且在测试前已运行；本次未启动、停止或清理容器，测试后仍保持运行。
- R3 UI 证据：API Docker 测试执行 headless GPUI 交互，先打开“断言”响应页签，再检查断言和 Collection 汇总；未运行真实 Windows 窗口验收。
- R3 集成状态：已通过 `f7c74ea8` 合并到 `dev`，随后由 `9c14fa7a` 整合到 `main` 并推送 `origin/main`；目标分支通过 `cargo test --locked -p ramag-tool-api --all-targets`（29 项）、`cargo test --locked -p ramag-app --all-targets`（224 项单元测试、2 项 data-sync live、6 项 SQL live、9 项 transfer live）、`cargo fmt --all -- --check`、workspace Clippy、源码尺寸和 `git diff --check`。源分支无未合并/未推送提交且无关联 worktree，已删除本地和远程引用。
- R4 已进入 `main`：`docs/performance.md`、`docs/development-roadmap.md` 和 `docs/database-client-datagrip-roadmap.md` 明确 MySQL 8.4+ / PostgreSQL 17+ 当前基线，并给历史 MySQL 8.0 测量标注旧基线及“不是当前验收证据”；`scripts/db-test/compose.yaml` 已确认固定使用 MySQL `mysql:8.4`、PostgreSQL `postgres:17-alpine`、Redis `redis:7-alpine`、MongoDB `mongo:8.2`，分别绑定 `127.0.0.1:13306`、`:15432`、`:16379`、`:27018`。`db-test.sh` 使用 `docker compose up --detach --wait` 启动、普通停止保留数据卷、清理命令删除数据卷和本地测试凭据；README、CI 和脚本未发现 MySQL 8.0 镜像或当前测试命令。本切片没有改写历史结果，也没有运行或操作 Docker 服务。源分支已清理。
- R4 验证：功能分支提交 `15513a44` 通过 `e247d884` 合并到 `dev`，再由 `9c14fa7a` 整合到 `main` 并推送；目标分支和整合后的 `main` 均通过 `git diff --check`、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` 和源码尺寸检查。`rg -n -i "mysql.{0,45}8\\.0|8\\.0.{0,45}mysql|mysql:8\\.0|mysql-8\\.0"` 检出的文档命中均明确标作历史旧基线或审查记录；`crates/ramag-infra-mysql/src/errors.rs` 中 `/8.0/` 只属于 MySQL 官方错误参考文档 URL，不是运行版本或测试镜像。此文档切片未运行集成测试。源分支已清理。
- R6 已完成并推送 `main`：`CHANGE COLUMN` 定义显式保留 `AUTO_INCREMENT`、生成表达式以及 `VIRTUAL`/`STORED` 属性，对不完整生成元数据和不支持的身份元数据拒绝生成 SQL。`cargo test --locked -p ramag-tool-dbclient --lib table_designer -- --nocapture`（21 项）、`cargo test --locked -p ramag-tool-dbclient --lib`（320 项）和 `cargo test --locked -p ramag-infra-mysql --test column_metadata -- --nocapture`（2 项）均通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 均通过。测试使用本机 `ramag-r6-mysql84` / `mysql:8.4`，端口 `127.0.0.1:13316->3306`，测试完成后执行 `docker rm -f`，容器和临时卷均已清理。Computer Use 原生应用接口不可用且应用列表为空，已改用系统截图 `artifacts/ui-screenshots/r6-system-fallback-window.png` 和 headless 表设计器测试；截图只证明 Ramag 窗口启动，不证明真实表设计器交互。
- R7 已完成并推送 `main`：表设计器只在确认 SQLite 表为空时生成无默认值的 `NOT NULL` 新字段；非空表要求默认值或允许 `NULL`，未知状态拒绝生成。`cargo test --locked -p ramag-tool-dbclient --lib table_designer -- --nocapture`（25 项）和 `cargo test --locked -p ramag-infra-sqlite --lib -- --nocapture`（5 项）均通过；fmt、workspace Clippy、源码尺寸和 `git diff --check` 均通过。SQLite 验证使用本机临时文件，未依赖外部服务。Computer Use 启动 Ramag 后仍返回空应用列表，已改用系统截图 `artifacts/ui-screenshots/r7-ramag-window-fallback.png` 和 headless 表设计器测试；截图只证明新构建可启动，不证明真实表设计器交互。
- R8 已完成并推送 `main`：`Backpressured` 现在暂停两个 Native MQTT 协议路径的事件读取并重试原消息。`cargo test --locked -p ramag-infra-mqtt --features native --lib -- --nocapture`（17 项通过、1 项忽略）、`cargo test --locked -p ramag-tool-mqtt --lib`（32 项）和本机 Docker 两协议背压集成测试（2 项）均通过；workspace 全量测试、fmt、默认与 native feature Clippy、源码尺寸和 `git diff --check` 均通过。Docker 使用 `ramag-mqtt-test` / `eclipse-mosquitto:2.0.20` / `127.0.0.1:18883->1883`，测试后执行 clean 删除容器和网络，无命名卷残留。Computer Use 启动最新 Ramag 后仍返回空应用列表，已改用系统截图 `artifacts/ui-screenshots/r8-ramag-window-fallback.png` 和 headless MQTT UI 测试；截图只证明数据库客户端窗口可启动，不证明真实 MQTT 订阅交互。

## 3. 分阶段落地计划

### 阶段 A：交互回归与现有证据收敛（已完成）

1. 实现 R5，单独提交 `test(api): cover request search clear interaction`。
2. 运行 `cargo test --locked -p ramag-tool-api --all-targets`、`cargo fmt --all -- --check`、目标 Clippy、源码尺寸和 `git diff --check`。
3. 更新 API 专项路线图，区分 headless 交互证据和真实 Windows 窗口证据。

### 阶段 B：API 工作区生命周期可靠性（已完成）

1. 设计并实现 R1 的读取失败状态、重试入口和保存保护。
2. 实现 R2 的保存代次/草稿版本隔离，并保留现有发送、Collection、gRPC 发现的取消语义。
3. 为 Storage 失败、旧结果迟到和导入交错增加应用层与 headless UI 测试。
4. 使用独立提交边界：读取错误处理与保存代次不得合并为一个提交；每项通过测试后立即推送当前开发分支。

### 阶段 C：Collection 结果语义（已完成并进入 main）

1. 先补 R3 的失败继续测试和错误分类设计，再修改 `run_collection`。
2. 明确可继续错误、停止错误、取消和 Storage 持久化失败的边界。
3. 更新 API 工作台汇总显示和历史记录验收，使用本机 Docker HTTP/gRPC 服务复核成功、业务失败和取消。

### 阶段 D：数据库基线与文档校正（已完成）

1. 按 R4 逐文件修正文档和验收记录，历史旧事实保留“旧基线”标识。
2. 检查 MySQL、PostgreSQL、Redis、MongoDB Compose 镜像、端口、启动和清理说明，确保当前示例满足 PostgreSQL 17+、MySQL 8.4+。
3. 文档修改单独提交，避免与运行时代码或测试混合。

### 阶段 E：表设计器 DDL 安全性（R6、R7 已完成）

1. R6 已通过 MySQL 8.4 Docker 元数据回读测试，保留属性的最小 SQL 表达范围已固定并推送到 `main`。
2. R7 已通过 SQLite 空表/非空表执行测试；表行探测失败时采用拒绝生成的安全策略，没有伪造重建表迁移。
3. R6、R7 与 R8 保持独立提交；下一步进入 R9，并继续每项只提交实现、对应测试和必要文档。

### 阶段 F：消息、传输与进程边界可靠性（R8 已完成，R9-R10 待实施）

1. R8 已固定背压语义并修改两个 Native MQTT 数据面，MQTT 3.1.1/5 本机 Docker 同构测试通过。
2. R9 建立 SFTP 提交结果模型，先验证替换成功/备份清理失败的部分成功状态，再调整 UI 文案和重试入口。
3. R10 先用可控并发测试复现 Socket 竞态，再选择锁或原子重试方案；不得仅增加 sleep 或扩大重试次数掩盖竞态。
4. R9、R10 分别提交、分别运行目标测试和必要的本机 Docker/真实端点验收；真实 Windows 窗口证据与协议正确性证据分开记录。

### 阶段 G：质量基线恢复（已完成）

1. 先独立修复 R11，恢复 workspace Clippy 质量基线。
2. 通过 fmt、Clippy、目标测试、源码尺寸和差异检查后，才开始提交本计划后续运行时代码切片。

## 4. 每个切片的验收条件

- 代码：目标测试通过；Rust workspace 最终内容通过 `cargo fmt --all -- --check` 和 `cargo clippy --workspace --all-targets -- -D warnings`。
- UI：用户可见行为先通过 headless GPUI 交互/边界测试；Computer Use 可用时补真实窗口截图和输入，不可用时在记录中写明限制。
- 集成：需要外部服务时只使用本机 Docker，记录服务名、镜像版本、端口、healthy/running 状态以及启动和清理状态。
- 资源：运行前确认错误消息、历史、日志和测试输出不包含密码、Token、完整请求正文或证书私钥。
- Git：每个独立可验收功能一次 Conventional Commit；默认直接在 `main` 上验证、提交和推送；明确使用功能分支时，验证后合并回 `main` 并清理已合并源分支。

## 5. 暂不处理项

- 不在本计划中扩展 Authorization Code、Device Code、Refresh Token 等新的 OAuth2 流程。
- 不把 API、MQTT、Kafka 和数据库真实 Windows 窗口证据混成一个验收结论。
- 不恢复已明确暂缓的本机镜像推送、标记、删除和清理联调。
- 不用静态 fixture、远程集群或仅编译结果替代本机 Docker 集成测试。
