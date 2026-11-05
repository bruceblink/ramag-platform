# Ramag Platform 主线开发计划

> 状态：唯一现行开发计划
> 更新日期：2026-11-06
> 代码核对基线：`v0.9.1`；当前 workspace 版本 `0.9.1`
> 后续开发基线：`v0.9.1` 带注释标签指向的提交
> 计划状态：自动更新与页面修复已随 `v0.9.1` 发布；接下来按队列补测量与验收证据

本文件统一维护范围、执行顺序、验收条件和未关闭事项。架构、接口设计、开发指南和
测试报告只提供技术依据，不单独安排开发任务。

## 1. 术语与维护方式

| 术语 | 含义与边界 |
|---|---|
| 主线任务 | 本文件执行队列中的具体工作；持续维护类别本身不作为可一次完成的任务 |
| 代码基线 | 已进入仓库的实现；不表示全部平台、硬件和真实窗口均已验收 |
| 原生窗口验收 | 使用 Computer Use 在真实应用窗口中操作并核对结果；启动检查和 Headless GPUI 结果分别记录 |
| 平台提交（Platform Submit） | GPUI 向窗口系统提交绘制结果；不能据此认定画面已显示 |
| 可见反馈（Visible Feedback） | 预期结果首次出现在显示画面；需要平台呈现追踪或同步录像确认 |

新增工作先写入本文件。每项说明问题、范围、通过条件、验证命令和回滚范围。
完成后删除执行队列中的条目，只在基线说明或版本变更记录中保留必要结论。
不再新增专项计划、发布计划、计划历史副本或归档计划。
旧计划的详细设计和逐次记录通过 Git 历史查阅，不恢复为当前待办。

## 2. 产品范围与固定边界

Ramag 保留数据库、Redis、MongoDB、API、Kafka、MQTT、Git、SSH/SFTP/PTY、端口转发、
对象存储、容器、剪贴板、系统监控、更新、设置、加密本地存储和 GPUI Shell。
当前只安排现有功能的缺陷、性能、稳定性、安全、兼容性、UI、构建和发布维护。
新增业务功能须由用户明确提出，并先更新本计划。

- 本机协作（`collaboration`）和独立 JSON Path 提取器（`json-path-extractor`）已移除，不恢复入口。
- API 工作台内部的 JSON Path 变量和断言继续保留。
- Ramag 与 `it-tools-template` 独立实现，不共享进程、运行时、代码包、接口协议、凭据、存储或发布物。
- MQTT 工作台按本项目的现有行为和明确需求独立维护，已移除的外部设计不再作为开发基线。
- 第一方工具采用编译期静态 Rust/GPUI 插件。插件接口设计、系统监控试点和九批工具迁移已进入代码基线，不再列为未来阶段。
- 新增业务能力按插件模块交付。平台核心负责通用注册、生命周期、授权、设置、任务、诊断和 UI 宿主服务。
- 动态库加载、第三方代码、外部插件进程、WASM 执行、插件市场和远程分发没有进入当前实现范围。
- 维护或移除工具时保留其他工具的用户配置、凭据、历史和数据库文件。

静态插件接口和维护方法统一见[静态插件接口与开发手册](05-plugin-development-guide.md)。

## 3. 已有基线

以下内容用于防止重复安排已完成工作，不是待执行队列。

| 领域 | 已有结果 | 证据边界 |
|---|---|---|
| 工具入口与列表 | 工具显示位置管理、数据库数据源列表、对象存储账号列表、Container/VCS/插件列表已完成对应切片 | 完整流程按工具分别验收；不扩展为所有错误与平台均已通过 |
| 连接工作区 | API 首次读取状态修复、双栏布局、两条分隔栏真实拖动，以及 API/Kafka/MQTT/SSH 全局回归已完成 | 专项数据库操作和其他未列出的原生流程仍按下节补验 |
| 数据库 | MySQL/PostgreSQL 连接、触发器子树、连接恢复、事务提交/回滚读回、单元格 UPDATE 以及行内 DELETE、批量 DELETE、INSERT 的失效连接反馈已有真实窗口证据 | 完整 `DB-RED-01..07` 矩阵仍部分完成 |
| 数据库规模与取消 | PostgreSQL 100,000 行分页、10,000 行只读查询、1,000 张表、长表头、双轴滚动、多标签隔离与页面/统计查询取消已验收 | 取消到服务端活动查询归零曾测得约 `629 ms`；这是单次环境记录，不能作为普遍时限 |
| 共享 UI 与监控 | System Pulse 迁入、公共样式、状态反馈、首页响应式网格及卡片视觉维护已进入基线；过渡时长统一为重排 `180ms`、加载 `160ms` | 页面功能、硬件准确性、发布性能和跨平台结果分别判断 |
| 静态插件 | 系统监控试点及 API、剪贴板、VCS、SSH、对象存储、容器、数据库、Kafka、MQTT 九批迁移完成 | 静态装配和代码验证不表示支持外部插件加载 |
| MQTT 独立文案 | 本地服务页已移除外部项目引用，保留配置生效时间和网络暴露提示 | MQTT 库测试 33 项通过；Windows 隔离预览窗口已核对文案，未启动 Broker |
| 更新检测与下载 | 自动检测开关、手动检查、进度、取消、错误反馈和已校验文件入口已实现；支持可选 BSDIFF40 差分，失败时回退完整包 | 2026-11-01 在 Windows 隔离开发构建中验证正式 `v0.9.0` 的发现、下载、取消、错误及目录打开；完整下载与差分重建文件的 SHA-256 均匹配正式安装包。该构建标记为 `0.8.0`，包含新的更新实现，不是正式 `v0.8.0` 安装包 |
| 更新页面反馈 | 已校验下载切换“安装安装包”；手动检查无更新时显示明确结果；整理版本信息与操作区，复用 Ramag 品牌标识并修正产品说明 | 2026-11-01 Windows 明暗主题真实窗口已核对标识、说明、布局和无更新提示；Headless GPUI 验证下载到安装按钮的切换及回调。未执行正式安装器，不能据此认定覆盖安装已验收；这些修复尚未包含在 `v0.9.0` 安装包中 |
| 发布 | `v0.9.1` 已正式发布，作为后续开发基线；包含自动更新与页面反馈修复 | 2026-11-01 核对 [v0.9.1 Release](https://github.com/bruceblink/ramag-platform/releases/tag/v0.9.1) 及 [发布任务 `41277943121`](https://github.com/bruceblink/ramag-platform/actions/runs/41277943121)：Windows、macOS、Linux 打包和发布均成功；5 个安装包和 4 个差分包的名称、大小与 SHA-256 均与更新清单及 GitHub 资产记录一致。标签 `v0.9.1` 指向提交 `6ef0d241` |

本轮源码修复在 Windows MSVC x64、stable Rust `1.99.1` 下通过格式、workspace 全目标检查、
严格 Clippy、应用构建和源文件大小检查。应用层 `284` 个单元测试、`17` 个集成测试通过；
UI 的 `166` 个单元及 Headless GPUI 测试通过；更新基础设施的 `11` 个单元测试通过。
需要 Docker 的测试在普通 `cargo test` 中保持忽略，单独执行结果如下。

本机 Docker HTTPS 更新测试使用 WSL `Ubuntu-24.04` 与 `nginx:1.27-alpine`，
覆盖清单、差分重建、损坏补丁回退、取消和摘要不匹配。首次回环连接失败，原因未能复现；
随后两次完整重跑及原端口诊断均通过，测试容器已清理。`v0.9.1` 发布后的公网测试通过：
GitHub 最新 Release 接口返回 `v0.9.1`，更新清单与标签一致，5 个安装包和 4 个差分包
的名称、大小及 SHA-256 与 GitHub 资产记录一致；`ramag-infra-update` 公网集成测试 1 项通过。
另从 `v0.8.0` 导出源码，只调整测试入口的 TLS 初始化和断言，验证旧驱动可读取正式 `v0.9.0`
清单；该结果不代表已安装旧版程序的完整升级流程。
Windows 正式安装、Linux/macOS 原生更新流程和下一次发布仍按下文的适用条件验证。

## 4. 当前执行队列

`RAMAG-UPDATE-001` 的更新实现已随 `v0.9.0` 发布；页面反馈修复已随 `v0.9.1` 正式发布。
后续开发以 `v0.9.1` 标签对应的提交为基线；按 `RAMAG-RELEASE-001` 发布，不预设日期或移动现有标签。
更新条件和缓存限制见[更新检测与下载](06-desktop-release.md#更新检测与下载)。

按下表顺序选择独立切片。顺序表示优先级，不表示所有任务必须串行。
每项先确认环境和现有证据，再实施、验证并记录结果。缺少某项前提时，记录阻塞原因，
继续处理不依赖该前提的任务；不以其他证据代替。新出现的明确缺陷可单独调整优先级。

| 顺序 | 任务 | 状态与依赖 |
|---|---|---|
| 1 | `RAMAG-UI-001AC-1` 补充测量方法 | 已完成真实相对时间、同窗口绘制与提交关系、乱序/溢出/无样本/提前退出记录；输入事件的窗口标识继续明确保留为缺失 |
| 2 | `DB-UX-006B-R` 大字段性能复核 | 已完成查看器代码修复、本机数据库集成复核和三轮真实窗口打开/关闭复核；响应性通过，内存采样边界已记录 |
| 3 | `A-UI-REAL-1` 数据库导航与查询验收 | 进行中；连接、对象树、查询、筛选、排序、滚动、分页、详情、CSV 和 JSONL 导出，MongoDB/Redis 专用对象，以及本地 PostgreSQL Server Objects 和 Virtual views 已有真实窗口记录；PostgreSQL、MySQL 和 MongoDB 刷新失败保留旧树已有真实窗口记录，Redis 刷新期间旧树保留并可停止后恢复，MySQL 权限受限对象已有真实窗口记录，PostgreSQL Server Objects 权限/空状态仍待补验 |
| 4 | `A-UI-REAL-2` 数据库写入与迁移验收 | 已完成；PostgreSQL 和 MySQL 手动事务提交、回滚及清理，MySQL DDL、结构差异、迁移执行、目标元数据回读、EXPLAIN ANALYZE 风险确认、失败查询草稿恢复，以及行内 DELETE、批量 DELETE、INSERT 的失效连接反馈均已有真实窗口证据 |
| 5 | `RAMAG-UI-001AC-2` 发布构建性能验收 | 进行中；Windows 发布构建的输入和绘制 p95 已取得，事件无丢失；两轮活动帧仅 `103` 和 `124` 个，低于 `300`，platform present p95 和可见反馈追踪仍缺失 |
| 6 | `RAMAG-UI-001A` 监控交互补验 | 分别检查 Disks 和 Network；需要可用设备与真实窗口 |
| 7 | `RAMAG-UI-001B` 剪贴板交互补验 | 使用隔离数据；不读取或改动用户剪贴板历史 |
| 8 | `RAMAG-UI-001C` 首页与侧栏排序补验 | 使用隔离配置；分别检查两个区域的顺序和保存回读 |
| 9 | `RAMAG-PLATFORM-001` 跨平台补验 | 需要对应 Linux/macOS 原生环境与独立系统测试账户 |
| 持续 | `RAMAG-RELEASE-001` 平台与发布维护 | 有相关变更或用户要求发布时执行；不预设发布日期 |

上述任务均为待执行；持续维护按触发条件启动。每个任务的结果只能覆盖实际执行的平台、驱动和步骤。
只发现问题时先记录可复现条件；修复另按受影响范围验证。尚未取得的设备、账号、服务或窗口证据保持未完成。

### 4.1 原生 UI 测量方法

现有 `ui-profiling` 特性位于 `ramag-ui` 和 `ramag-bin`，运行时显式设置
`RAMAG_UI_PROFILE=1` 才启动。采样最长 `120s`，每 `250ms` 汇总一次，
原始事件最多记录 `10,000` 条。正常构建和正常启动不启用采样，不为采样主动请求重绘。
缺失指标保留缺失状态，不能填零。

当前 `profiling.rs` 记录输入、draw、present 的耗时与各自 p95，但日志中的输入事件没有窗口 ID，
也没有足够的事件时间信息证明某次输入与哪一帧、哪一次实际显示对应。现有 `present` 指平台提交，
不是显示合成器呈现。缺失窗口归属或时间时不得推断关联。

2026-11-01 已按 [Cargo.lock](../Cargo.lock) 中的 `gpui-kit 0.7.3`、`gpui-pre 0.3.9`
和 `gpui-pre-windows 0.3.9` 静态核对依赖源码：

| 数据来源 | 可取得的信息 | 仍缺少的信息 |
|---|---|---|
| `journal::InputTiming` | 输入种类、开始与结束时间、是否触发失效重绘 | 原始输入事件没有窗口 ID 或帧 ID |
| `FrameTiming` | 窗口 ID、绘制开始与结束时间；可选的首次失效及帧请求时间 | 没有屏幕实际显示时间 |
| `PresentTiming` | 窗口 ID、平台提交开始与结束时间 | 没有单次输入 ID 或实际显示时间 |
| `IntervalBoundary::Presented` | 直接包含对应的绘制与提交记录，可保留这组已知关系 | 不能由此补出缺失的输入窗口归属 |
| `Window::input_latency_snapshot()` | 单窗口输入到提交的聚合直方图、每帧输入数和绘制期间丢弃的输入数 | 不能还原每次输入的原始时间关系，也不表示可见反馈 |

Windows 路径在 `Window::present` 中记录 `platform_window.draw` 调用前后的时间；
DirectX 渲染器调用交换链的 `Present` 后返回。上述公开记录没有提供显示合成器的呈现时间。
此结论来自源码核查，不是性能实测；Linux/macOS 平台路径尚未在本轮核查。

`RAMAG-UI-001AC-1` 接下来只补充能够真实取得的时间及已知绘制/提交关系；
输入窗口标识取得不到时明确记为缺失，不能按时间接近推断关联。
记录事件总数、原始记录数量、丢失及耗时样本丢弃数，不把前 `10,000` 条样本冒充全部事件。
当前提前退出仅关闭采样开关，没有输出最终汇总；后续切片需补齐提前退出结果。
默认功能和正常动画保持不变，不在本任务中修改 GPUI 依赖或推断实际显示时间。

完成条件：记录可用字段和限制；可关联的同窗口事件能够回读时序，跨窗口、乱序、溢出、
无样本和提前退出分别得到明确结果。若平台不提供必要信息，完成字段核查后说明缺口，
`RAMAG-UI-001AC-2` 的对应性能条件继续保持未完成。
涉及代码时运行 `cargo test --locked -p ramag-ui --features ui-profiling --lib profiling::tests -- --test-threads=1`，
并执行第 6 节的适用检查。回滚只涉及采样模块、可选特性入口、对应测试和计划记录。

2026-11-04 已完成本项实现。`crates/ramag-ui/src/profiling.rs` 现在记录采样器起点后的真实微秒时间、
绘制与平台提交的窗口标识和时间关系，并在 `ui_profiling_complete` 中记录停止原因、采样数量、
同窗口或跨窗口关系、时间乱序、时间戳早于采样起点、日志环溢出和输入窗口标识缺失。
应用退出回调会输出提前退出结果；重复完成请求只保留一条最终记录。Windows 本地定向测试
`profiling::tests` 共 6 项通过，格式检查和 `git diff --check` 通过。该切片没有改变默认渲染、动画或布局，
真实窗口验收不适用；发布构建性能条件仍未完成。

### 4.2 大字段性能复核

`DB-UX-006B-R` 复核 2026-10-27 的 PostgreSQL Debug 窗口结果。当时，`SELECT repeat('x',1048577)` 的网格截断和
当前字段查看已通过功能验收。查看器打开后工作集抽样均值增加约 `40.42 MiB`，
关闭后仍高于查询前约 `37.84 MiB`；关闭时曾短暂显示“未响应”，约 `2s` 后恢复。
这些抽样没有证明峰值、回收速度或持续泄漏。

2026-11-05 在 Windows Debug 真实窗口复核中重现了同一风险：`SELECT repeat('x',1048577)` 的结果查看器约两秒后才显示内容，
关闭操作期间窗口进入“未响应”。已将只读预览上限调整为 `256 KiB`，并让超过 `64 KiB` 的预览跳过 Markdown 解析，
复制按钮仍读取当前单值。`ramag-tool-dbclient` 的 376 项单元与渲染测试、严格 Clippy 和 Debug 构建通过。

同日修复后使用 WSL Docker 中的 PostgreSQL 测试库，在 Windows Debug 真实窗口完成三轮打开/关闭复核。
每轮都显示 `1,048,577 bytes` 和“仅显示前 256 KiB”，关闭后回到结果网格；第二、三轮关闭耗时分别约 `204 ms` 和 `196 ms`，
第三轮从打开菜单到查看器可见约 `808 ms`。复核后的进程保持 `Responding=True`，工作集为 `189,976,576` 字节，
Private Bytes 为 `173,277,184` 字节。该内存值是关闭后的单点采样，不能证明峰值或持续泄漏；本项只据此确认操作可恢复，
不把单点内存值当作回收或泄漏结论。

复核只使用本机测试库的只读表达式和隔离配置。记录重复打开/关闭、恢复时间、工作集和私有字节。
不向系统剪贴板复制样例，不改业务数据。只有复现出具体问题后，才修改对应查看器或数据路径。
通过条件是结果有界、操作可恢复、测量可复现；未解决的响应停顿继续明确列出。
涉及代码时运行 `cargo test --locked -p ramag-tool-dbclient --lib -- --test-threads=1`，
并执行第 6 节的适用检查。回滚只涉及字段查看器、对应测试和记录；不改用户数据或测试服务卷。

### 4.3 数据库真实窗口验收

完整区域定义保留在 [UI 规范的数据库矩阵](04-ui-acceptance-standard.md#4-数据库工作区功能验收)。
先核对已有证据，补验时不重复计入已经通过的查询取消和规模测试。

- `DB-RED-01..04`：连接切换、MongoDB/Redis 专用对象行为，以及本地 PostgreSQL Server Objects 和 Virtual views 的只读操作已有真实窗口证据；本地 PostgreSQL、MySQL 和 MongoDB 刷新失败保留旧树已有真实窗口证据，Redis 刷新期间旧树保留并可停止后恢复，MySQL 权限受限 Server Objects 已有真实窗口证据，Redis 最终连接失败提示和 PostgreSQL Server Objects 权限/空状态仍待补验。
- `DB-RED-05` / `DB-UX-003`：补排序、筛选后的页码和上下文、NULL/二进制/长字段区分、查看/复制/导出；已有列宽、滚动和分页记录继续有效。
- `DB-RED-06` / `DB-UX-004B-3B`：失败查询草稿保留和恢复，以及行内 DELETE、批量 DELETE、INSERT 的失效连接反馈均已有真实窗口证据。原生 UPDATE 已通过，其他路径的 Headless GPUI 结果不能替代窗口操作。
- `DB-RED-07` / `DB-UX-005A..F`：DDL 查看/复制/刷新、稳定键结构差异、迁移指纹/阶段摘要、执行和目标元数据回读，以及 EXPLAIN ANALYZE 风险提示均已有真实窗口证据。代码、headless 和 Docker 回放结果仍按原范围有效。
- 首帧、滚动、标签切换和长期内存的可复现测量仍按性能标准检查；窗口截图不证明发布性能达标。

MySQL/PostgreSQL 按各自方言验证；MongoDB 不使用 SQL 事务语义，Redis 不伪造 Schema 或列。
不支持的动作显示原因，不用空结果表示成功。

`A-UI-REAL-1` 先验连接、对象树、查询、筛选、排序、滚动、详情和导出。
`A-UI-REAL-2` 在连接与回读路径可用后，验隔离表的事务、失败后的草稿保留、DDL、差异与迁移回读；
不要求所有只读补验项先关闭。
每项分别记录驱动、镜像版本、端口、测试对象、窗口操作、结果和清理；不能操作业务库。
仅补验不改代码时，Rust 测试不适用；有修复时执行 `ramag-tool-dbclient` 目标测试和第 6 节的适用检查。
清理只限本次创建的测试对象，保留既有容器和数据卷。任何未覆盖的矩阵项继续标为未完成。

2026-11-05 使用 WSL `Ubuntu-24.04` 中的 Docker PostgreSQL `17-alpine`（Windows 端口 `15432`）
在新 Debug 构建真实窗口复核 `public.bulk_records`。打开表后显示 `100,000` 行；输入
`status = 'active'` 并按 Enter 后显示 `33,333` 行；点击 `amount` 列头后排序生效，滚动和下一页
保持结果上下文，页码显示 `101-200 of 33333`；右键 `body` 单元格可打开详情并显示 Unicode 文本。
复核还发现并修复了一个状态不一致：筛选结果上点击“运行”后，旧版本会恢复 `100,000` 行但继续显示
WHERE 条件。修复后同一操作会清空 WHERE 输入并恢复 `100,000` 行，真实窗口和 `ResultPanel` 回归测试结果一致。
同日选中 3 行并选择 CSV，保存到隔离路径
`.ramag/ui-acceptance/2026-11-05/exports/bulk-selected.csv`；文件回读为 4 行、933 字节，
包含表头和 3 行数据，中文、JSON 字段和 CSV 转义均保留。JSONL 菜单可见但未单独选择和回读，
因此只关闭 CSV 导出子项，不能据此关闭完整驱动矩阵。

2026-11-05 在真实窗口完成 `A-UI-REAL-2` 的 PostgreSQL 事务切片。环境为 WSL
`Ubuntu-24.04` 的 Docker `postgres:17-alpine`，因 `keystone-persistent-postgres` 持续占用
`15432`，本次隔离测试使用 Windows 端口 `25432`；MySQL、Redis、MongoDB 同步使用
`23306`、`26379`、`27028`。测试卷为本项目专用卷，重建后由 `scripts/db-test/db-test.sh seed`
播种并通过数据量核对。WSL 使用前台 keepalive 保持 `dockerd` 运行，避免 WSL 空闲退出并停止
测试容器；未修改应用源码。

在 `public.type_matrix` 的 `id = 1` 行执行以下窗口操作：

1. 开始手动事务，把 `nullable_value` 从 `NULL` 更新为 `ui-tx-commit`；查询回读到该值，
   点击“提交”，再次查询仍为 `ui-tx-commit`。
2. 开始第二个手动事务，把同一字段更新为 `ui-tx-rollback`；事务内查询回读到该值，
   点击“回滚”，重新查询恢复为已提交的 `ui-tx-commit`。
3. 开始清理事务，把字段恢复为 `NULL` 并提交；最终查询显示 `NULL`。连接状态保持“已连接”，
   虚拟视图刷新成功并显示只读会话快照。

本切片通过条件已满足：事务状态提示、受影响行数、提交后回读、回滚后回读和清理结果均在真实
窗口中观察到。`A-UI-REAL-2` 继续保持进行中，后续补验 MySQL、DDL、差异和迁移回读；本次
仅补验且未改源码，Rust 测试不适用。

2026-11-05 完成数据库只读文本选择修复。DDL 预览改用可选择的 `TextView` SQL 代码块，保留
现有语法高亮和横向滚动；结果表列名改用单行可选择文本。列标题的普通点击仍执行排序，拖选
文本时会跳过排序事件。Windows Debug 真实窗口打开 `public.api_keys` 后，DDL 内容可见，列名
拖选后使用 Ctrl+C 并粘贴到条件框，观察到列标题字符进入输入框。共享 UI 的拖选复制测试、
数据库客户端 379 项单元与渲染测试、严格 Clippy 和 Debug 构建均通过。该记录只覆盖只读文本
选择交互，不关闭 MySQL、差异和迁移回读项。

2026-11-06 在同一套 WSL `Ubuntu-24.04` Docker MySQL `8.4` 测试服务中完成 MySQL 的写入与迁移补验。
服务使用 Windows 端口 `23306`，数据库为 `ramag_test`，连接用户为 `ramag`；操作对象为
`ramag_test.type_matrix`，执行后保持连接状态为“已连接”。事务步骤为：将 `id = 1` 的
`nullable_value` 更新为 `ui-mysql-commit` 并提交，提交后查询仍返回该值；再次更新为
`ui-mysql-rollback`，事务内查询返回临时值，回滚后查询恢复为 `ui-mysql-commit`；最后将字段
恢复为 `NULL` 并提交，最终查询返回 `NULL`。

同一窗口打开 DDL 只读视图，看到 `CREATE TABLE` 定义；点击复制显示“复制成功”，点击刷新后
定义仍可见。随后在表树中对 `type_matrix` 选择“比较表结构”，对比临时表
`type_matrix_migration_target`；差异视图以稳定列名和位置显示新增的 `migration_probe` 列。
迁移预览显示 1 条语句、1 条删除或修改语句、脚本 SHA-256 指纹和执行阶段摘要；确认后执行
`DROP COLUMN migration_probe`，界面提示“目标表结构已匹配”，回读后的差异视图不再显示该列。
临时迁移表随后执行 `DROP TABLE IF EXISTS` 清理，表树恢复为原有 4 张表。该切片覆盖 MySQL
事务提交/回滚/清理、DDL 查看/复制/刷新、稳定键差异、迁移指纹/阶段摘要、执行和目标元数据
回读；失败后的草稿保留已在下一段完成。本次只补验且未改源码，Rust 测试不适用；WSL Docker
测试服务和数据卷按本地开发约定继续保留。

同日继续在该 MySQL 连接的 SQL 编辑器中输入
`EXPLAIN ANALYZE SELECT COUNT(*) FROM \`ramag_test\`.\`type_matrix\`;` 并按 Ctrl+Enter。
执行前的高危确认明确提示“EXPLAIN ANALYZE 会执行目标查询（可能产生副作用或消耗资源）”，
同时显示目标连接、数据库和完整 SQL；点击“取消”后回到编辑器，未产生查询结果或数据变化。
该操作确认了执行计划入口会把 `EXPLAIN ANALYZE` 作为实际执行查询的高危语句处理；本次不点击
“仍要执行”，因此没有把风险提示误记为真实目标查询执行成功。

同日继续在 `ramag_test.type_matrix` 中选择 3 条结果记录，切换导出格式为 JSONL 并保存到
`.ramag/ui-acceptance/2026-11-05/exports/ramag-mysql-ramag_test-type_matrix-data-20261106-001736.jsonl`。
文件回读为 3 行、3 条 JSON 记录、`2611` 字节；首条记录的 `NULL`、中文和嵌套 JSON 对象可解析，
大整数 `bigint_unsigned` 以字符串保存，末条记录的引号、换行和制表符也保留。该结果关闭了
MySQL JSONL 选中行导出子项；MongoDB、Redis 和其他未覆盖驱动的专用导出项仍按矩阵保持未完成。

同日继续验证失败查询的草稿保留和恢复。在 SQL 编辑器中输入
`SELECT * FROM \`ramag_test\`.\`missing_table_for_draft\`; -- draft-sentinel` 并执行，结果区显示
“表不存在”错误，编辑器仍保留完整 SQL；点击“重试”后错误原因再次显示，草稿文本不变。关闭该
查询标签时确认框明确提示“未保存 SQL 草稿将丢失”，随后点击“重新打开最近关闭的查询”，新建的
`查询4` 标签恢复了相同的 `draft-sentinel` 文本。该验证只读取不存在的表名，没有修改测试库。

同日停止 WSL Docker MySQL `8.4` 和 PostgreSQL `17-alpine` 测试容器，使用已加载的真实结果表
验证失效连接写操作。PostgreSQL `public.type_matrix` 选择 1 行执行行内 DELETE，确认后显示
“已影响 0 行、0 行未匹配后出错：连接失败：连接池等待超时（数据库可能慢或不可达）”；随后选择
2 行执行批量 DELETE，显示相同的连接失败提示，结果表保留原来的 3 行。恢复 PostgreSQL
容器后健康检查通过。MySQL `ramag_test.large_values` 新增草稿填写 `id=2`、短文本、`00` 和
测试 JSON；停止 MySQL 容器后提交，右上角和底部均显示“新增失败：连接失败：连接池等待超时
（数据库可能慢或不可达）”，草稿提交后回到原结果，不产生写入。随后恢复 MySQL 容器并确认
MySQL、PostgreSQL、Redis、MongoDB 四个测试容器均为 healthy。该切片关闭 `DB-RED-06` 的
失效连接反馈子项；`DB-RED-01..04` 的刷新失败保留旧树及 Server Objects 权限/空/失败状态仍待补验。

2026-11-06 在 WSL `Ubuntu-24.04` Docker MongoDB `8.2`（Windows 端口 `27028`）中完成专用对象
补验。连接名为 `db-test-mongo`，数据库为 `ramag_demo`；真实窗口显示“已连接”，展开数据库后
看到 `capped_events`、`large_documents`、`metrics`、`orders`、`products`、`type_matrix` 和
`users` 集合。打开 `type_matrix` 后显示 2 行和 MongoDB 类型列（`_id`、字符串、整数、长整数、
小数等）；输入行筛选 `string-id` 后结果收敛为 `1 / 2` 行，连接状态保持“已连接”。本次只读
集合数据，没有写入文档。

同日完成 WSL Docker Redis `7-alpine`（Windows 端口 `26379`）补验。连接名为 `db-test-redis`，
窗口显示 DB0 和约 `45,014` 个键；左侧按 Hash、Large、List、Set、Special、Stream、String、ZSet
分组，未伪造 Schema 或列。打开 Hash 键 `1` 后显示 Hash 类型、永久 TTL 和 3 个元素
（`name=user-1`、`score=1`、`active=1`）；打开 List 键 `1` 后显示 List 类型、永久 TTL 和 3 个
按序元素（`first`、`value-1`、`last`）。本次只读键值，没有修改 Redis 数据。该切片关闭 MongoDB
集合树/筛选和 Redis 类型分组/详情子项；连接切换、失败刷新保留旧树、Server Objects、Virtual
views 以及其他未覆盖驱动项仍按矩阵保持未完成。

同日继续在同一真实窗口切换 `db-test-mysql`、`db-test-mongo` 和 `db-test-redis` 标签。切回
MySQL 后查询工作区和 `type_matrix` 对象树仍可见；切回 MongoDB 后 `ramag_demo.type_matrix`
及 `string-id` 行筛选仍可见；切回 Redis 后 DB0 的 `list:1` 详情仍可见。三个标签的连接状态
均保持“已连接”，未重新创建连接或修改测试数据。该操作关闭连接切换子项，失败刷新保留旧树、
Server Objects、Virtual views 和其他未覆盖驱动项仍未完成。

2026-11-06 在 WSL `Ubuntu-24.04` Docker PostgreSQL `17-alpine`（Windows 端口 `25432`）中补验
Server Objects 和 Virtual views。连接名为 `db-test-postgres`，数据库为 `ramag_test`，用户为
`ramag`；默认连接 URI 使用 TLS 时测试先超时，改用同一服务的
`postgres://.../ramag_test?sslmode=disable` 后测试连接成功并显示“已连接”。对象树显示
`analytics` 和 `public` Schema，`public` 下有 3 张表、1 个 view，Server Objects 展开后加载
`collations 913`，说明服务器对象读取成功。收起后打开 Virtual views 的 `sessions`，真实窗口
显示“只读会话快照”和 9 行会话记录，包含 pid、用户名、数据库、客户端地址、状态、查询开始时间
和查询文本；本次未执行写入。该切片关闭 PostgreSQL Server Objects 和 Virtual views 只读子项，
刷新失败保留旧树及权限/空/失败状态转由后续隔离服务切片补验。

同日使用同一真实窗口验证连接配置失败边界：将 `db-test-postgres` 临时端口改为未监听的
`25433` 后，应用提示“连接配置已更新，相关标签已暂停”，标签内保留连接名称并显示“重连”入口；
恢复端口 `25432` 后点击“重连”，窗口重新显示“已连接”，`analytics`、`public`、表、view、
Server Objects 和 Virtual views 对象树均恢复。该结果只证明配置更新失败的暂停与恢复流程，
不能替代刷新动作失败时保留旧树的验收，后者在下一段完成；权限/空/失败状态仍待补验。

同日停止 WSL Docker PostgreSQL `17-alpine` 测试容器，在 `db-test-postgres` 标签点击对象树刷新。
刷新期间对象树仍显示 `analytics`、`public`、3 张表、1 个 view、Server Objects 和 Virtual views；
失败后标签状态变为“连接失败”，对象树保留原有节点，底部显示“刷新失败：连接失败：连接池等待超时
（数据库可能慢或不可达）”，并提供“重试”入口。恢复容器并点击“重试”后，标签恢复“已连接”，
结果表和对象树继续可用。该切片关闭本地 PostgreSQL 刷新失败保留旧树子项；MySQL 的刷新失败和
权限受限 Server Objects 继续由下一段补验。

同日继续在 WSL Docker MySQL `8.4` 的 `db-test-mysql` 标签补验权限和刷新失败。展开 Server Objects
后显示 `collations 286`、`users 1`，用户项为 `ramag@% · 权限受限，仅显示当前账号`，说明读取
`mysql.user` 受限时没有伪造空结果。停止 MySQL 容器后点击刷新，旧有 4 张表、1 个 view、Server
Objects 分组和 Virtual views 仍保留；对象树显示“Server Objects 刷新失败：连接失败：连接池等待超时”，
标签状态为“连接失败”并提供“重试”。恢复容器后重试，再刷新对象树，权限提示和失败提示均清除，
连接恢复为“已连接”。该切片关闭 MySQL 刷新失败保留旧树和权限受限 Server Objects 子项；
PostgreSQL Server Objects 权限/空状态仍待补验。

同日继续在 WSL Docker MongoDB `8.2` 的 `db-test-mongo` 标签补验刷新失败。展开 `ramag_demo` 后
显示 `capped_events`、`large_documents`、`metrics`、`orders`、`products`、`type_matrix` 和
`users` 7 个集合；停止 MongoDB 容器并点击刷新后，标签变为“连接失败”，左侧仍保留原有数据库和
7 个集合，同时显示“连接失败：无法选择服务端（请检查 host/port/TLS）”和“刷新”入口。恢复容器
并点击刷新后，标签恢复“已连接”，7 个集合继续可用。该切片关闭 MongoDB 刷新失败保留旧树子项；
Redis 最终连接失败提示和 PostgreSQL Server Objects 权限/空状态仍待补验。

同日使用修复后的 Windows Debug 构建补验 WSL Docker Redis `7-alpine`（Windows 端口 `26379`）。
`db-test-redis` 刷新前显示 DB0 的 8 类 Key 分组和 `45,014` 个 Key；停止 Redis 容器后点击刷新，
扫描期间左侧旧分组继续可见，底部仍显示已加载 `45,014` 个 Key。等待约 `26s` 后连接仍在重试，
未得到最终失败文案，因此点击“停止扫描”；停止后旧树没有被清空。恢复 Redis 容器并再次刷新，
标签恢复“已连接”，底部回到 `45,014` 个 Key，分类树继续可用。该切片验证了刷新缓冲、手动
中断和服务恢复，不替代 Redis 最终连接失败提示的验收；该提示仍待补验。

### 4.4 发布构建性能验收

`RAMAG-UI-001AC-2` 在 `RAMAG-UI-001AC-1` 具备所需计时信息后，使用发布构建，
在固定机器、数据量、刷新率和窗口尺寸下执行：

1. 首轮加载后预热 `5s`，按固定顺序导航、滚动和操作控件至少 `30s`。
2. 保留至少 `30` 次操作、`300` 个活动帧、原始日志、事件丢失数量和步骤次数。
3. 分别计算输入处理、绘制、平台提交和可见反馈的 p95。现有采样器只输出前三项的 p95；可见反馈需要平台显示追踪或同步录像，不能由平台提交时间推算。
4. 按 [UI 验收标准](04-ui-acceptance-standard.md#204-固定性能工作流与计时边界)核对：点击到可见反馈 p95 不超过 `100ms`；`60Hz` 下活动帧处理耗时 p95 不超过 `16.7ms`。
5. 动画结束且没有采样时观察 `10s`，确认没有装饰动画持续请求帧。

样本不足、事件丢失导致结果不可用或缺少呈现追踪时，记录具体限制。
完成条件：记录每项计时起止点、平台和原始证据，并分别判断 `PULSE-P01` 的点击、活动帧和空闲条件。
先保留诊断结论，再决定是否安排渲染优化，不凭主观感受增加优化范围。回滚只涉及本次测量辅助代码与记录。

2026-11-06 在 Windows 发布构建中执行两轮真实窗口采样。构建命令为
`cargo build --locked --target x86_64-pc-windows-msvc -p ramag-bin --features ui-profiling --release`；
本次命令使用已安装的 Windows SDK `10.0.26100.0` 和 MSVC stable Rust `1.99.1`，没有执行仓库的
MSVC 初始化脚本。环境为 Windows 11 Pro for Workstations `10.0.26300`、NVIDIA GeForce RTX 3060、
2560×1440、系统报告刷新率 `59Hz`，Ramag 窗口客户区约 `1431×927`、深色主题和标准字号。
两轮均预热后持续超过 `30s`，操作包含 Overview/CPU/Memory/GPU/Disk/Network/Power/Temperature/Processes
标签切换、进程列表往返滚动、列排序和 `firefox` 筛选后清空；第二轮额外执行 `200` 次交替滚轮滚动，
最后停留在设置页约 `36s`。本次没有读取或修改系统剪贴板，也没有修改数据库或设置值。

第一轮采样持续 `45.082s`：`561` 个 foreground 事件、`6` 个输入事件、`105` 个 draw 事件、
`103` 个已匹配的同窗口呈现边界；输入 p95 为 `6.132ms`，draw p95 为 `10.809ms`，
`journal_lost=0`、`duration_samples_dropped=0`、溢出为 `false`。第二轮持续 `60.106s`：
`692` 个 foreground 事件、`5` 个输入事件、`126` 个 draw 事件、`124` 个已匹配的同窗口呈现边界；
输入 p95 为 `5.294ms`，draw p95 为 `12.146ms`，同样没有日志丢失或样本丢弃。两轮 `present_events=0`，
因此 `present_p95=None`；采样器没有提供输入到屏幕可见反馈的追踪，不能用平台提交时间代替。
原始 stderr 日志保留在本机 `%TEMP%\\ramag-ui-profile-76041fd641b04aae8e97751225faa676.stderr.log`
和 `%TEMP%\\ramag-ui-profile-8a14df13e9c74000bf8195f544b83d4b.stderr.log`。

该切片证明发布构建能够完成真实窗口操作，输入和绘制 p95 低于当前阈值，且采样器没有丢失事件；
但活动帧数量仍低于 `300`，present p95、点击到可见反馈和独立空闲动画观察也没有完整证据，
所以 `RAMAG-UI-001AC-2` 继续保持未完成。本轮没有发现足以安排渲染优化的可复现瓶颈，未修改源码。
发布构建已通过；本次仅补验和更新计划，Rust 测试不适用。

### 4.5 现有页面交互补验

以下切片分别记录操作、结果、环境和未覆盖状态。先复用已有 Headless GPUI 测试，
再补真实窗口证据；发现可复现缺陷后才修改对应代码。

- `RAMAG-UI-001A`：在 Disks 和 Network 页分别检查设备选择、切换、滚动及设备不可用提示。
  使用测试设备或当前只读数据，不改变系统网络和磁盘配置；设备不足时标记对应场景未完成。
  修改代码时运行 `cargo test --locked -p ramag-tool-system --lib -- --test-threads=1`。
- `RAMAG-UI-001B`：在独立测试账户或隔离的测试窗口中，使用专用记录检查搜索、删除、撤销和当前
  搜索结果回读，分别记录失败反馈。先确认测试入口不会监听或读取用户系统剪贴板；无法隔离时停止该项。
  不使用用户剪贴板历史，不向用户系统剪贴板写入测试内容。
  修改代码时运行 `cargo test --locked -p ramag-tool-clipboard --lib -- --test-threads=1`。
- `RAMAG-UI-001C`：检查首页和侧栏各自的拖动排序、取消、保存，以及重新启动后的顺序回读。
  使用隔离配置，不覆盖用户偏好；键盘支持按现有功能核对，不由本任务新增交互。
  修改代码时运行 `cargo test --locked -p ramag-ui --lib -- --test-threads=1`。

每个切片完成条件是所列适用步骤有对应的真实窗口结果，失败和不可用场景明确记录。
涉及源码时另执行第 6 节检查。回滚仅限本切片的视图、测试与验收记录。

### 4.6 跨平台补验

`RAMAG-PLATFORM-001` 在 Linux 和 macOS 分别验收现有原生窗口与系统凭据存储。
每个平台先核对安装包、运行环境和对应提交的 CI 结果，再使用独立系统测试账户或虚拟机，
以及专用测试数据库与凭据，检查启动、保存、退出重启后的回读和无法读取密钥时的错误反馈。
已有加密数据库缺少主密钥时应停止启动，不能覆盖数据库或静默重建密钥。

调试构建的 `RAMAG_UI_TEST_DATA_DIR` 只隔离数据库目录，仍使用系统凭据；发布构建不读取该变量。
因此，不能仅靠该变量隔离凭据，更不能在用户账户中删除主密钥来制造错误。
缺少独立测试账户或对应平台时，保持该平台未完成。分别记录平台、包、操作、证据和清理结果；
CI、headless 与真实窗口结果分开记录。若需要修复，先复现问题，再运行受影响 crate 测试和第 6 节检查。
回滚只涉及该次修复及其测试，不删除用户数据库、系统凭据或未由本次创建的测试资源。

### 4.7 发布维护

`RAMAG-RELEASE-001` 按需检查 Windows、Linux、macOS 质量结果、真实窗口、安装包、SHA-256
和更新清单。Linux/macOS 秘密存储、签名、公证、DPI 与多显示器须有各自的环境证据。
不移动已公开标签；发现发布缺陷时使用新提交和新版本修复。

## 5. 条件限制与未纳入执行的事项

以下内容保留其未验证状态，不因合并文档自动启动新功能。

| 事项 | 处理边界 |
|---|---|
| 平台与硬件 | macOS 网络自动选择、Linux/macOS 原生窗口与秘密存储、GPU/PawnIO 不同硬件、进程授权、托盘恢复和跨 DPI/多显示器，需要对应平台或设备证据；Windows 开发机结果不能代替 |
| 监控差异 | 单位/量程/图例、失败来源、键盘焦点、配置损坏恢复和后台历史连续性按实际缺陷复核；新增温和结束、托盘动态图、偏好同步等能力先取得明确范围 |
| 外部服务 | COS/OSS 云端流程、真实 Kubernetes、Kafka TLS/SASL/ACL 服务兼容及生产 exporter 安全配置，须具备对应测试环境；RustFS、明文 Broker 或代码检查不替代这些结果 |
| 更新库跨程序复用 | [架构说明](02-architecture.md#跨程序复用评估)已评估提取边界与现有方案；尚未安排提取、改变许可证或发布公共库。Ramag 与 `it-tools-template` 保持现有独立边界 |
| 历史扩展设想 | 更多本地 Broker 管理、容器写操作扩展、纯 Rust Kafka 默认替换和外部插件承载，均不从旧路线图自动恢复排期 |
| WSL 启动 | `Ubuntu-24.04` Docker Engine 和登录任务已有手动运行检查；实际重新登录/重启后的启动仍未验证，见[开发入门指南](03-development-guide.md#95-wsl-docker-环境) |
| 历史数据恢复 | [2026-10-06 测试报告](testing/2026-10-06-ui-integration-test-report.md#ssh-清理事故与恢复状态)记录用户 `known_hosts` 未恢复，仍需匹配备份或用户确认重新核验；新测试通过不能关闭此事项 |
| 隔离验收数据 | 保留本轮更新验收的隔离配置、缓存和日志，以及部分旧验收临时目录，便于复核。后续清理先确认专用目录和保留需要，不删除用户配置、凭据或既有服务数据 |

## 6. 验证命令与证据规则

文档变更检查 UTF-8、LF、Markdown 结构、相对链接与章节锚点，并运行 `git diff --check`。
仅改文档时，Rust 编译、UI 操作和 Docker 集成测试标记为“不适用”。

Rust 变更从 workspace 根目录执行。Windows 先在同一 PowerShell 进程加载仓库 MSVC 环境：

```powershell
. ./scripts/windows/enable-msvc-toolchain.ps1
rustup run stable rustc -Vv
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked -p <受影响的-crate> -- --test-threads=1
cargo build --locked -p ramag-bin
./scripts/windows/check-source-size.ps1
git diff --check
```

原生 UI 采样另运行：

```powershell
cargo test --locked -p ramag-ui --features ui-profiling --lib profiling::tests -- --test-threads=1
cargo check --locked -p ramag-bin --features ui-profiling --examples
cargo build --locked --release -p ramag-bin --features ui-profiling
```

- UI 先使用 Computer Use 和真实窗口。不可用时按恢复流程检查，分别记录启动、headless 与未完成的原生交互。
- Headless GPUI 至少覆盖 `360x640`、`1024x768`、`1440x900` 和明暗主题；弹窗追加 `360x240`。原生最小尺寸以下的结果仅作为 headless 边界检查。
- 外部服务集成使用 WSL `Ubuntu-24.04` 的本机 Docker Engine。记录服务、镜像版本、端口、健康、实际连接、测试和清理结果。保留用户要求持续运行的服务。
- 数据库基线为 MySQL `8.4+`、PostgreSQL `17+`。使用仓库 `scripts/db-test/` 流程；未启动真实服务时不能计为集成通过。
- Linux 本机仅用 WSL 编译；Linux/macOS 测试由 `.github/workflows/desktop-ci.yml` 执行。三平台质量检查、发布打包和原生验收分别记录。
- 发布按[桌面端构建与发布](06-desktop-release.md)和标签提交范围生成说明；验证包、SHA-256 和更新清单后再报告发布验收完成。
- 通过状态附具体日期、提交、环境、命令及证据范围。历史环境限制不代表当前工具仍不可用。

## 7. 回滚与文档边界

每个实现切片只回滚其代码、测试和计划条目，不回滚无关工作或删除用户数据。
性能诊断切片的回滚范围是可选特性、采样模块、入口与相关 CI 检查，不涉及正常布局、动画或配置。
数据库窗口验收只清理本轮明确创建的测试对象，不重建用户容器或数据卷。
公开版本标签不移动、不覆盖；已发布缺陷使用新提交和新版本修复。

文档只在对应职责文件维护；编号与入口见[文档索引](README.md)。旧文件名、历史记录和变更前内容
可通过 `git log --follow -- <文档路径>` 与 `git show <提交>:<旧文档路径>` 查阅。
用户全局配置不进入仓库提交。
