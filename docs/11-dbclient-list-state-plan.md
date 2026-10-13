# DBClient 数据源列表状态复核与修复

> 切片：`A-POST-0.5-DBCLIENT-001`
> 日期：2026-10-12 至 2026-10-13
> 基线：`8b65421f`；数据源首页主要布局已由 `c4c20d06` 实现
> 状态：代码、headless、真实窗口与全工作区本机 Docker 集成验收完成

## 问题与范围

读取连接列表失败时，页面保留旧连接并显示失败原因和重试入口。但搜索没有匹配旧连接时，渲染分支只显示无匹配提示，隐藏读取失败及重试。用户不能区分无匹配与未能刷新。

本切片统一旧列表上方的失败通知，使它在有匹配和无匹配两种结果下都可见。保留搜索条件、旧连接、稳定连接 ID、固定表头与虚拟列表。复核行操作命中，确认编辑、同步和删除请求不会误触发打开连接。

## 验收条件

- 有旧连接但当前搜索无匹配时，失败原因、重试、无匹配提示和搜索框均保留。
- 搜索改变后重新匹配旧连接时，失败通知仍可见；状态区不遮挡表头或行操作。
- 错误信息过长时，页面显示长度受限；重试仍可操作，悬停可查看完整原因。
- 初次加载、空列表、非空列表与读取失败保持可区分；重试调用现有刷新路径。
- 明暗主题、16px/18px、360x640/1024x768/1440x900 下，状态、搜索和主要操作可达。
- 对应 headless 交互、`ramag-tool-dbclient` 全量测试、workspace fmt/严格 Clippy、源码尺寸、完整构建及 LF/diff 检查通过。

## 验证入口

```powershell
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-dbclient','--lib','--','--test-threads=1')
cargo fmt --all -- --check
& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked','--workspace','--all-targets')
& .\scripts\windows\check-source-size.ps1
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand build -CargoOptions @('--locked','-p','ramag-bin')
git diff --check
```

本切片不改变数据库协议、连接保存或凭据，因此切片本身不依赖 Docker。用户随后要求运行完整数据库集成测试，本机 MySQL、PostgreSQL、Redis 和 MongoDB 驱动测试记录在下文。Computer Use 首次重置后出现截图与控件树不同步；重新观察后控件树恢复到数据源列表，才继续执行搜索操作。真实窗口没有安全的列表读取失败注入入口，因此该失败组合只由 headless 测试验证。

## 验收记录（2026-10-12）

- DBClient lib 测试：375 passed，0 failed；包括无匹配时保留失败通知、长错误仍能访问重试、重试连续失败后保留搜索和旧连接、成功重试后清除失败状态，以及筛选和重排后的行操作身份。
- `cargo fmt --all -- --check`：通过。
- MSVC workspace all-target 严格 Clippy（`-D warnings`）：通过。
- Windows 源码尺寸检查：通过。
- MSVC `cargo build --locked -p ramag-bin`：通过。
- `git diff --check` 和本轮修改文本的 UTF-8/LF 检查：通过。
- 本机 Docker 四库集成：用户单独要求执行；四个数据库 crate 的单元和集成测试共 186 项通过，见下文。
- Computer Use 真实窗口检查：截图显示 1 个临时连接行及 MySQL 类型、地址、账号、编辑/删除图标；搜索 `no-such-connection` 后显示“没有匹配连接”，清空后行恢复。真实窗口没有注入列表读取失败；错误通知组合、行操作、主题与多尺寸原生交互仍待补，headless 结果不作为替代。

错误提示最多直接显示 240 个 Unicode 字符，末尾用省略号标记；悬停提示保留完整错误文本。实现只改变连接列表展示和测试，不改变存储内容。

## 本机 Docker 四库集成验收（2026-10-12）

- Docker Engine：Windows Docker Desktop `29.7.2 linux/amd64`，上下文 `desktop-linux`。测试服务使用已有专用容器和数据卷，没有接触 WSL Docker 引擎。
- 服务：`mysql:8.4`（`127.0.0.1:13306`）、`postgres:17-alpine`（`127.0.0.1:15432`）、`redis:7-alpine`（`127.0.0.1:16379`）、`mongo:8.2`（`127.0.0.1:27018`）。执行前通过 `docker start` 启动四个容器，随后四者均为 `running/healthy`。
- 全量测试集检查：MySQL 为 `100000/3/1/1`；PostgreSQL 为 `100000/3/1/8000`；MongoDB 为 `20000/15000/60000/2/100/20000/10000`。Redis DB 15 为空。Redis DB 0 起初为 `45014`，低于脚本要求的 `46000`；种子中的 TTL 键已过期。仅在专用测试 DB 0 重建 `ttl:1` 至 `ttl:1000` 后，DB 0 为 `46014`，DB 15 仍为空。
- 测试命令：从 `.ramag/db-test.env` 将配置载入当前 PowerShell 进程，不输出凭据；通过 `scripts/windows/invoke-cargo-msvc.ps1` 执行四个数据库 crate 的 `cargo test --locked`，设置 `RAMAG_TEST_DATASET=full` 和 `RUST_TEST_THREADS=1`。MongoDB 48 项、MySQL 36 项、PostgreSQL 40 项、Redis 62 项通过；合计 186 passed、0 failed。Redis 全键空间扫描及全量数据集用例通过。
- 服务启动方式：复用已有容器。容器标签显示其 Compose 项目名和仓库脚本当前指定值不同，因此本次没有让 Compose 重建容器。
- 清理：测试完成后以 `docker stop` 停止四个容器，保留专用容器、数据卷和本地忽略的凭据文件；不删除或清空卷。

## 真实窗口检查（2026-10-12）

- 使用当前工作区 MSVC Debug 程序和隔离数据目录 `C:\Users\likanug\AppData\Local\Temp\ramag-dbclient-ui-acceptance-20261012`。启动后列表为空；保存一条无密码测试连接 `UI acceptance temporary`（`192.0.2.1:3306`）后，后续观察到该连接的失败工作区。没有确认打开连接的触发动作；生产保存事件本身不自动打开新连接。该保留地址用于文档测试，不对应可路由服务；没有再次重试。
- 重置并重新初始化 Computer Use 后，真实窗口截图显示数据源列表包含 1 行，名称、MySQL 类型、地址、账号和编辑/删除图标均可见。首次状态捕获的控件树仍显示连接工作区；再次聚焦搜索框并观察后，控件树恢复为数据源列表。
- 在真实窗口中搜索 `no-such-connection`，截图显示 `0 / 1 个数据源` 和“没有匹配连接”；清空搜索后，原连接行恢复。该操作验证正常列表数据下的筛选与恢复，不验证列表读取失败时的组合状态。
- 原生窗口没有验证列表读取失败注入、编辑/同步/删除操作、主题切换或各尺寸布局。首次状态不同步后仅在重新观察确认控件树恢复后继续搜索，没有使用旧控件索引。

## 回滚边界

只回滚数据源列表状态渲染、对应测试和本切片文档。不修改用户连接、配置、`v0.5.0` 标签或已发布资产。

## 原生失败场景补验计划（2026-10-13）

范围：扩展现有 `ramag-bin` 的 `ui-preview` 示例，注册生产 DBClient 视图和 Shell。进程专属临时 redb 使用固定测试密钥，预置三条无密码 SQL 连接；SQL 驱动不注册，Redis/MongoDB 使用拒绝网络请求的验收驱动。数据库表单测试、连接打开和元数据读取不会创建数据库网络连接。仅示例中的 Storage 包装器支持列表读取失败，其他读写仍作用于临时 redb。

控制：列表首读成功后按 F6 触发失败刷新，F8 触发长错误，F7 恢复读取。通过正常服务保存路径增加连接修订号，生产列表按既有刷新路径读取。命令最后的 `large` 参数选择 18px 字号；省略时使用 16px。

补验中发现完整错误 Tooltip 保持单行，长文本超出真实窗口。修复范围追加：仅在连接列表错误详情中使用按窗口宽度换行、高度有界和可滚动的 Tooltip；保留完整正文，并增加支持尺寸和 360x640 的实际容器渲染检查。

验收：真实窗口保留旧连接和搜索；无匹配时仍显示失败及重试；连续重试失败后状态不丢失；恢复后保持搜索并清除错误。检查编辑、删除和同步对话框入口及取消返回；检查明暗主题、16px/18px、1024x768/1440x900 和实际最小窗口。360x640 继续使用既有 headless 防御测试。记录示例窗口与完整程序证据的范围。

验证：运行 `ramag-bin --all-targets` 测试、workspace fmt、严格 Clippy、源码尺寸、`ramag-bin` 主程序与示例构建、Computer Use、UTF-8/LF 和 diff 检查。UI 补验不改变数据库协议；另按用户要求补齐包含 App 数据同步的本机 Docker workspace 集成。回滚只撤销 UI preview 扩展、示例测试和补验文档。

## 验收记录（2026-10-13）

- 隔离预览：复用生产 Shell 和 DBClient 视图，显示三条仅存在于进程专属 redb 的连接。SQL 驱动未注册；Redis/MongoDB 使用拒绝全部请求的驱动。检查期间没有对用户配置、系统凭据或数据库服务发起请求。
- 真实窗口：在 1024x768 和 1440x900 下检查明暗主题及 16px/18px 字号；另以最小客户区 1008x672 启动窗口，系统窗口截图为 1010x704（含窗口边框）。名称、环境、类型、账号、状态标记和编辑/删除/同步入口可见。
- 真实失败流程：搜索 `no-such-connection` 后按 F6，页面保留查询并同时显示旧列表读取失败、重试和无匹配状态；F8 错误的详情气泡换行并完整留在最小窗口内；F7 后失败状态清除，查询仍保留。重试仍沿生产刷新路径执行。
- 真实行操作：搜索 `source` 后分别打开编辑、同步和删除对话框，再取消并确认列表与搜索不变；没有触发数据库连接或删除样例。
- Headless 尺寸：长错误容器覆盖标准/大字号，以及 360x640、1008x672、1024x768、1440x900；宽、高边界和窗口边界断言通过。360x640 仅为 headless 结果。
- Rust 验证：正确筛选的长 Tooltip 测试 1 passed；`ramag-bin --all-targets` 15 项主程序测试和 1 项预览测试通过；workspace fmt、严格 Clippy、源码尺寸及主程序/示例构建通过。全工作区集成结果见下一节。
- 范围限制：这是 DBClient 切片的原生窗口验收，不代表完整 `ramag.exe` 的启动、其他工具或跨工具流程已经完成 UI 验收。

## 本机 Docker 全工作区集成验收（2026-10-13）

- Docker Engine：Windows Docker Desktop `29.7.2 linux/amd64`，上下文 `desktop-linux`。复用四个专用测试容器；测试前均为 `running/healthy`，没有重建或访问其他项目的容器。
- 服务：`mysql:8.4`（`127.0.0.1:13306`）、`postgres:17-alpine`（`127.0.0.1:15432`）、`redis:7-alpine`（`127.0.0.1:16379`）、`mongo:8.2`（`127.0.0.1:27018`）。
- 全量测试数据：MySQL 为 `100000/3/1/1`；PostgreSQL 为 `100000/3/1/8000`；MongoDB 为 `20000/15000/60000/2/100/20000/10000`。Redis DB0 为 `46014` 个键，DB15 为空。`ttl:1..1000` 均存在；为避免测试中途过期，仅将这些键的 TTL 延长到 3600 秒，前后值哈希一致。
- 测试命令：`& .\scripts\windows\invoke-cargo-msvc.ps1 test --locked --workspace -- --test-threads=1`。测试进程从 `.ramag/db-test.env` 读取配置，没有打印凭据；设置 `RAMAG_TEST_DATASET=full`、`RUST_TEST_THREADS=1` 和独立 PostgreSQL 参数。命令纳入 `ramag-app/tests/data_sync_live.rs` 与 `data_sync_sql_live.rs`。
- 结果：117 个测试目标摘要合计 2702 passed、0 failed、19 ignored，退出码为 0。汇总未可靠区分每个测试摘要对应的 crate，因此不列分 crate 数量。
- 清理：测试后的 PostgreSQL 数据库列表中已无本轮临时库 `ramag_dbtest_pg_20261013_0e753044`。随后停止四个专用容器，状态均为 `exited`；保留容器和数据卷。未触碰其他容器或卷。
