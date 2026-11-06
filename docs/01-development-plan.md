# Ramag Platform 主线开发计划

> 状态：唯一现行开发计划
> 更新日期：2026-11-06
> 代码核对基线：提交 `8820c77a`（`v0.9.1` 之后的源码维护提交）；`d76d4784` 至 `b5597119` 的 CI 入口和验收记录，以及 `ea474c6a` 的 MongoDB 结果工具栏修复，均已记录在本计划
> 当前主线提交：`5fd3421a`；workspace 版本 `0.9.1`
> 后续开发基线：提交 `5fd3421a`
> 计划状态：`v0.9.1` 已发布；当前主线已完成选择复制、Redis 扫描边界、数据库导航与导出的大部分真实窗口验收、MongoDB 结果工具栏视觉修复和 MongoDB 多页结果验收，以及发布构建测量记录，后续只推进下表中的未关闭证据项

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
| 数据库 | MySQL/PostgreSQL/SQLite 连接、查询与部分元数据树，触发器子树、连接恢复、事务提交/回滚读回、单元格 UPDATE 以及行内 DELETE、批量 DELETE、INSERT 的失效连接反馈已有真实窗口证据 | 完整 `DB-RED-01..07` 矩阵仍部分完成 |
| 数据库规模与取消 | PostgreSQL 100,000 行分页、10,000 行只读查询、1,000 张表、长表头、双轴滚动、多标签隔离与页面/统计查询取消已验收 | 取消到服务端活动查询归零曾测得约 `629 ms`；这是单次环境记录，不能作为普遍时限 |
| 共享 UI 与监控 | System Pulse 迁入、公共样式、状态反馈、首页响应式网格及卡片视觉维护已进入基线；过渡时长统一为重排 `180ms`、加载 `160ms` | 页面功能、硬件准确性、发布性能和跨平台结果分别判断 |
| 静态插件 | 系统监控试点及 API、剪贴板、VCS、SSH、对象存储、容器、数据库、Kafka、MQTT 九批迁移完成 | 静态装配和代码验证不表示支持外部插件加载 |
| MQTT 独立文案 | 本地服务页已移除外部项目引用，保留配置生效时间和网络暴露提示 | MQTT 库测试 33 项通过；Windows 隔离预览窗口已核对文案，未启动 Broker |
| 更新检测与下载 | 自动检测开关、手动检查、进度、取消、错误反馈和已校验文件入口已实现；支持可选 BSDIFF40 差分，失败时回退完整包 | 2026-11-01 在 Windows 隔离开发构建中验证正式 `v0.9.0` 的发现、下载、取消、错误及目录打开；完整下载与差分重建文件的 SHA-256 均匹配正式安装包。该构建标记为 `0.8.0`，包含新的更新实现，不是正式 `v0.8.0` 安装包 |
| 更新页面反馈 | 已校验下载切换“安装安装包”；手动检查无更新时显示明确结果；整理版本信息与操作区，复用 Ramag 品牌标识并修正产品说明 | 2026-11-01 Windows 明暗主题真实窗口已核对标识、说明、布局和无更新提示；Headless GPUI 验证下载到安装按钮的切换及回调。未执行正式安装器，不能据此认定覆盖安装已验收；这些修复尚未包含在 `v0.9.0` 安装包中 |
| 发布 | `v0.9.1` 已正式发布，作为后续开发基线；包含自动更新与页面反馈修复 | 2026-11-01 核对 [v0.9.1 Release](https://github.com/bruceblink/ramag-platform/releases/tag/v0.9.1) 及 [发布任务 `41277943121`](https://github.com/bruceblink/ramag-platform/actions/runs/41277943121)：Windows、macOS、Linux 打包和发布均成功；5 个安装包和 4 个差分包的名称、大小与 SHA-256 均与更新清单及 GitHub 资产记录一致。标签 `v0.9.1` 指向提交 `6ef0d241` |

### 3.1 2026-11-06 代码审查结论

本次审查以提交 `8820c77a` 为基线。Windows MSVC stable Rust `1.99.1` 下，以下检查均通过：

- `cargo fmt --all -- --check`
- `cargo check --locked --workspace --all-targets`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace -- --test-threads=1`

本次审查未发现需要立即修复的可复现新缺陷。已核对的重点如下：

- `ramag-ui` 的 `Presented` 边界耗时已纳入 `present_p95_us`。该指标仍表示窗口系统提交，不表示屏幕合成器已显示。
- 数据库大字段查看器保留有界预览，并通过独立路径支持复制完整当前值。预览上限和复制行为已有对应测试。
- Redis 前缀扫描使用超时、缓冲树和有限重试边界。刷新失败时保留旧树，相关单元测试已通过。

本次审查还核对了 `v0.9.1` 之后的源码提交范围。`7ef88792` 只扩展 UI 采样器的 Presented 边界统计，
`89c6f053` 和 `d83a21e8` 只收紧 Redis 连接重试与扫描超时，`8f879eab` 和 `bc6b91d8` 只处理
数据库标题选择与筛选状态，均有对应测试或真实窗口记录。随后提交 `ea474c6a` 修复了 MongoDB
结果工具栏的过滤框布局和编辑器滚动条显示。动画仍保持重排 `180ms`、加载 `160ms`，后续只在取得
点击到可见反馈的显示追踪后判断是否需要优化，不以减少动画时长替代流畅度验收。

剩余问题属于验收证据范围。它们不能由本次代码检查直接关闭，也不能用 Headless 或平台提交结果替代真实可见反馈和跨平台窗口证据。

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
后续开发以提交 `ea474c6a` 为基线；`v0.9.1` 保持不变。按 `RAMAG-RELEASE-001` 发布，不移动或覆盖已公开标签。
更新条件和缓存限制见[更新检测与下载](06-desktop-release.md#更新检测与下载)。

按下表顺序选择独立切片。顺序表示优先级，不表示所有任务必须串行。
每项先确认环境和现有证据，再实施、验证并记录结果。缺少某项前提时，记录阻塞原因，
继续处理不依赖该前提的任务；不以其他证据代替。新出现的明确缺陷可单独调整优先级。

| 顺序 | 任务 | 状态与依赖 |
|---|---|---|
| 1 | `RAMAG-UI-001AC-1` 补充测量方法 | 已完成真实相对时间、同窗口绘制与提交关系、乱序/溢出/无样本/提前退出记录；输入事件的窗口标识继续明确保留为缺失 |
| 2 | `DB-UX-006B-R` 大字段性能复核 | 已完成查看器代码修复、本机数据库集成复核和三轮真实窗口打开/关闭复核；响应性通过，内存采样边界已记录 |
| 3 | `A-UI-REAL-1` 数据库导航与查询验收 | 进行中；SQL 对象树搜索、收藏、仅收藏、最近访问和重启保存已覆盖 PostgreSQL/MySQL/SQLite；SQLite 表、视图、列、索引、触发器、DDL、只读视图和 JSONL 已补验；PostgreSQL/MySQL/SQLite 选中行 JSONL、MongoDB 选中行 Extended JSONL、Redis 数据库/单 Key/前缀 JSONL 已回读；MongoDB/Redis 专用对象、跨连接切换、PostgreSQL/MySQL/MongoDB/Redis 刷新失败保留旧树、MySQL 权限受限 Server Objects、PostgreSQL 受限角色的空 public、可读 Server Objects 和直接权限拒绝已有真实窗口记录；PostgreSQL 和 MySQL Virtual views 的只读会话快照已在真实窗口复核；MongoDB 和 SQLite 查询结果的列过滤、行过滤、排序、分页及 MongoDB 工具栏样式已在真实窗口复核，单行过滤框不再显示内部滚动条。未关闭内容只包括矩阵中尚未取得的驱动×动作证据；下一次验收先按矩阵列出具体驱动和动作，再逐项关闭或标记不适用，不制造用户数据风险。 |
| 4 | `A-UI-REAL-2` 数据库写入与迁移验收 | 已完成；PostgreSQL 和 MySQL 手动事务提交、回滚及清理，MySQL DDL、结构差异、迁移执行、目标元数据回读、EXPLAIN ANALYZE 风险确认、失败查询草稿恢复，以及行内 DELETE、批量 DELETE、INSERT 的失效连接反馈均已有真实窗口证据 |
| 5 | `RAMAG-UI-001AC-2` 发布构建性能验收 | 进行中；修复后的 Windows Release 采样已取得 `364` 个 draw、`363` 个 Presented 边界，`input/draw/present p95` 分别为 `0.239/8.599/1.090ms`，无日志丢失或溢出；设置页独立空闲观察已补齐，下一步只补点击到可见反馈。不能取得显示合成器或同步录像证据时保持未完成，不用降低动画时长替代流畅度验收。 |
| 6 | `RAMAG-UI-001A` 监控交互补验 | 已完成；Windows 真实窗口已核对 Disk 页和 Network 接口选择/切换、磁盘选择器、页面溢出滚动及设备不可用提示 |
| 7 | `RAMAG-UI-001B` 剪贴板交互补验 | 已完成；隔离 redb 数据和真实窗口已覆盖搜索、删除、撤销、当前结果回读和详情查看，未读取或写入用户系统剪贴板 |
| 8 | `RAMAG-UI-001C` 首页与侧栏排序补验 | 已完成；隔离配置下已检查首页和侧栏拖动、拖出取消、保存及重启后的顺序回读 |
| 9 | `RAMAG-PLATFORM-001` 跨平台补验 | 进行中；提交 `d76d4784` 已在 WSL Ubuntu-24.04 完成 Linux workspace 全目标编译，GitHub Actions 的 Linux、macOS、Windows 质量任务均成功。Linux/macOS 原生窗口、系统凭据存储和独立测试账户证据仍未取得，继续保持未完成 |
| 持续 | `RAMAG-RELEASE-001` 平台与发布维护 | 有相关变更或用户要求发布时执行；不预设发布日期 |

表中已完成项只保留结果和证据边界。当前只推进“进行中”或受环境条件约束的事项；持续维护按触发条件启动。每个任务的结果只能覆盖实际执行的平台、驱动和步骤。
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

- `DB-RED-01..04`：连接切换、SQLite 表/视图/元数据、MongoDB/Redis 专用对象行为，以及 PostgreSQL/MySQL Server Objects 和 Virtual views 的只读操作已有真实窗口证据；PostgreSQL、MySQL 和 SQLite 对象树搜索、收藏、仅收藏筛选和最近访问筛选已在重启前后复核。PostgreSQL、MySQL、MongoDB 和 Redis 刷新失败保留旧树已有真实窗口证据，MySQL 权限受限 Server Objects 和 PostgreSQL 受限角色的 public 空状态、Server Objects 可读边界及直接权限拒绝已有真实窗口证据，Redis 最终连接失败提示已补验。SQLite Virtual views 明确显示本地驱动不支持，未计为失败。
- `DB-RED-05` / `DB-UX-003`：补排序、筛选后的页码和上下文、NULL/二进制/长字段区分、查看/复制/导出；PostgreSQL、MySQL、MongoDB 和 SQLite 的排序/筛选/分页记录已补齐对应范围，已有列宽、滚动和分页记录继续有效。
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

2026-11-06 继续修复数据库只读标题的选择交互。DDL 弹框标题
`ramag_test.type_matrix` 改用单行可选择文本；移除弹框级鼠标按下拦截，保留左侧专用拖动手柄，
因此标题拖选不会移动弹框，弹框仍可拖动。结果表把列名和类型合并为同一段可选择标题，
例如 `id bigint unsigned`；普通点击仍执行排序。Windows Debug 真实窗口中，DDL 标题拖选后
使用 Ctrl+C 粘贴到过滤列输入框得到 `ramag_test.type_matrix`，DDL 代码双击选择
`CREATE` 后可复制；结果表头拖选后得到 `id bigint unsigned`。本次没有读取或改动系统剪贴板历史，
只使用应用内过滤列输入框回读剪贴板内容。`cargo fmt --all`、`cargo test -p ramag-tool-dbclient
--lib`（379 项）和 Windows Debug 构建通过；随后完成 Windows MSVC release 构建。

2026-11-06 使用 Windows MSVC Debug 构建补验 SQLite 本地文件流程。临时数据库文件包含
`acceptance_records` 表、中文和 JSON 文本、BLOB 列、索引、触发器，以及只读视图
`acceptance_recent`。连接表单测试显示“连接成功”；真实窗口的对象树显示 `main` 下 1 张表和 1 个
视图，展开表后可见 6 列、1 个索引和 1 个触发器。打开表显示 3 行，条件 `amount >= 10` 执行后保留
2 行；打开 `acceptance_recent` 显示 2 行只读结果；DDL 视图显示 `CREATE TABLE` 定义。选中 1 行并
选择 JSONL 导出，保存到本轮临时目录后回读为 155 字节、1 条可解析记录，中文标题、JSON 文本、数值、
时间和 `note` 字段均存在。SQLite 的 Virtual views 刷新显示“功能尚未实现”，符合本地文件驱动的能力边界，
不记录为失败。临时 Debug 进程、数据库、隔离配置和导出文件已清理；本次只补验且未改源码，Rust 测试不适用。

2026-11-06 使用 Windows MSVC Debug 构建补验 SQLite 对象树导航。隔离数据库包含
`alpha_records`、`beta_records` 两张表和 `alpha_recent` 视图；真实窗口搜索 `alpha` 后只显示匹配的
表和视图，选择“收藏”后表行显示星标，切换“仅收藏”后只保留 `alpha_records`。打开该表显示 2 行，
切换“最近访问”后显示该表。关闭并重新打开同一隔离配置的 Debug 进程，收藏星标和“最近访问”筛选
仍保留。SQLite Virtual views 显示本地驱动不支持，符合已记录的能力边界。临时 Debug 进程、数据库和
隔离配置已清理；本次只补验且未改源码，Rust 测试不适用。该记录关闭 SQLite 驱动的 `DB-RED-02`
搜索、收藏、最近访问和重启保存子项。

同日使用 Windows MSVC Debug 构建和隔离 SQLite 文件补验查询结果矩阵。数据库文件为
`.ramag/ui-acceptance/sqlite-query-20261106/data/sqlite_query.sqlite3`，包含 `query_records` 表的
205 行和 `query_records_recent` 只读视图；连接测试显示“连接成功”。打开表后第一页显示
`1-100 of 205`，下一页显示 `101-200 of 205`，末页显示 `201-205 of 205`。点击 `sort_value` 列头后
工具栏显示 `ORDER BY sort_value ASC`，首行变为 `id=205`、`sort_value=1`；运行当前查询后输入
`amount >= 10`，结果收敛为 `1-84 of 84`。列过滤输入 `id,label` 后网格只保留 `id` 和 `label` 两列，
行过滤结果仍为 84 行，连接状态全程为“已连接”。SQLite 的 Virtual views 继续显示“功能尚未实现”，
符合本地文件驱动的能力边界；本次没有执行写入。停止隔离应用后回读 SQLite 文件仍为 `rows=205`，
只读视图仍存在。本次关闭 SQLite `DB-RED-05` 的排序、行过滤、列过滤和多页分页子项；本次只更新
验收记录，Rust 测试不适用。

同日继续在 WSL `Ubuntu-24.04` 的 Docker `postgres:17-alpine`（Windows 端口 `25432`）中补验
PostgreSQL JSONL 导出。WSL 保持前台 `sleep infinity` 进程，确保 Docker 服务和本地端口转发在真实窗口操作期间持续运行；
四个数据库测试容器均为 healthy。使用 `sslmode=disable` 连接 `ramag_test` 的 `public.type_matrix`，
选中全部 3 行并选择 JSONL，保存到本轮临时目录后回读为 `3062` 字节和 3 条可解析记录。记录包含
中文、JSON/JSONB、NULL、数组、时间、UUID、XML、bytea 和 PostgreSQL 数值边界；三条记录的 `id` 为
`1`、`2`、`3`。临时 Debug 进程、隔离配置和导出文件已清理；该切片关闭 PostgreSQL JSONL 选中行导出子项，
本次只补验且未改源码，Rust 测试不适用。

2026-11-06 在同一套 WSL `Ubuntu-24.04` Docker PostgreSQL `17-alpine`（Windows 端口 `25432`）中补验
对象树导航状态。真实窗口搜索 `type_matrix` 后只显示匹配表；清除搜索后选择“收藏”，表行显示星标；
切换“仅收藏”后只保留该表，再切换“最近访问”后显示该表。关闭并重新打开同一隔离配置的 Debug 进程，
收藏星标和“最近访问”筛选结果仍保留。临时 Debug 进程、隔离配置和测试数据目录已清理；本次只补验且未改源码，
Rust 测试不适用。该记录关闭 PostgreSQL 驱动的 `DB-RED-02` 搜索、收藏、最近访问和重启保存子项，
其他驱动仍按矩阵分别记录。

同日使用 WSL `Ubuntu-24.04` Docker MySQL `8.4`（Windows 端口 `23306`）补验对象树导航状态。
真实窗口搜索 `type_matrix` 后只显示匹配表；清除搜索后选择“收藏”，表行显示星标；切换“仅收藏”
后只保留该表。打开表显示 3 行数据，再切换“最近访问”后显示该表。关闭并重新打开同一隔离配置的
Debug 进程，收藏星标和“最近访问”筛选结果仍保留。临时 Debug 进程、隔离配置和测试数据目录已清理；
本次只补验且未改源码，Rust 测试不适用。该记录关闭 MySQL 驱动的 `DB-RED-02` 搜索、收藏、最近访问
和重启保存子项，其他驱动仍按矩阵分别记录。

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

同日继续在该 MongoDB 真实窗口中选中 `type_matrix` 的 2 行，点击导出并保存到
`.ramag/ui-acceptance/2026-11-05/exports/ramag-mongodb-ramag_demo-type_matrix-data-20261106-062028.jsonl`。
文件回读为 2 行、2 条可解析的 Extended JSON 记录；`$numberLong`、`$numberDecimal`、`$date`、
`$timestamp`、`$oid`、`$binary`、正则、数组、嵌套对象和 `null` 均保留。该切片关闭 MongoDB
选中行 JSONL 导出子项；Redis 的 Key/前缀/数据库导出仍按矩阵保持未完成。

同日继续使用 Windows MSVC Debug 构建和独立数据目录补验 MongoDB 查询结果动作。WSL
`Ubuntu-24.04` Docker MongoDB `8.2`（Windows 端口 `27028`）保持 healthy；在
`ramag_demo.type_matrix` 真实窗口中搜索集合名 `type_matrix` 后只显示匹配集合，点击
`int32Value` 列头后行顺序按数值升序变化；行过滤输入 `string-id` 后结果为 `1 / 2` 行，
列过滤输入 `_id,stringValue` 后结果表只保留 2 个指定列。清除过滤后连接状态仍为“已连接”，
数据仍为 2 行；本次只读查询，没有写入文档。该切片关闭 MongoDB `DB-RED-05` 的集合搜索、
排序、行过滤和列过滤子项；只有 2 行测试数据，因此没有把多页翻页场景记为已覆盖。

同日继续使用 WSL Docker MongoDB `8.2` 创建临时集合
`ui_acceptance_pagination_20261106`，插入 205 条带有 `page-001` 至 `page-205` 标记的测试文档。
Windows MSVC Debug 真实窗口打开该集合后，第一页显示 100 行并保留“第 1 页”状态；点击“下页”后
显示 `id=101..200` 和“第 2 页”；再次点击“下页”后末页显示 `id=201..205` 共 5 行和“第 3 页”。
连接状态全程为“已连接”，页码切换没有改变列上下文。验收结束后停止隔离应用并删除临时集合，
MongoDB 回读 `remaining=0`；本次关闭 MongoDB `DB-RED-05` 的多页分页子项，未修改既有测试集合。

同日完成 WSL Docker Redis `7-alpine`（Windows 端口 `26379`）补验。连接名为 `db-test-redis`，
窗口显示 DB0 和约 `45,014` 个键；左侧按 Hash、Large、List、Set、Special、Stream、String、ZSet
分组，未伪造 Schema 或列。打开 Hash 键 `1` 后显示 Hash 类型、永久 TTL 和 3 个元素
（`name=user-1`、`score=1`、`active=1`）；打开 List 键 `1` 后显示 List 类型、永久 TTL 和 3 个
按序元素（`first`、`value-1`、`last`）。本次只读键值，没有修改 Redis 数据。该切片关闭 MongoDB
集合树/筛选和 Redis 类型分组/详情子项；连接切换、失败刷新保留旧树、Server Objects、Virtual
views 以及其他未覆盖驱动项仍按矩阵保持未完成。

同日继续在该 Redis 真实窗口的 DB0 菜单中选择“导出库”，保存到
`.ramag/ui-acceptance/2026-11-05/exports/ramag-redis-db0-20261106-062829.jsonl`。
文件回读为 `45,038` 行，包括 1 行导出头和 `45,037` 条 Key 记录；记录保留 Key、类型、TTL 和
对应值，List 与 String 的值结构可解析。该切片关闭 Redis 数据库 JSONL 导出子项；Redis 的
单 Key、前缀导出和其他未覆盖矩阵项继续按计划补验。

同日继续在 `list:10` 的 Key 右键菜单中选择“导出”，保存到
`.ramag/ui-acceptance/2026-11-05/exports/ramag-redis-db0-key-list_10-20261106-063344.jsonl`。
文件回读为 2 行，包括导出头和 1 条 Key 记录；记录保留 `list:10`、类型 `list`、永久 TTL
以及 `first`、`value-10`、`last` 三个元素。该切片关闭 Redis 单 Key JSONL 导出子项；Redis
前缀导出和其他未覆盖矩阵项继续按计划补验。

同日继续在 `list` 前缀右键菜单中选择“导出前缀”，保存到
`.ramag/ui-acceptance/2026-11-05/exports/ramag-redis-db0-prefix-list-20261106-063808.jsonl`。
文件回读为 3,001 行，包括导出头和 3,000 条 `list:*` Key 记录；记录均保留类型 `list`、永久
TTL 和 List 值。该切片关闭 Redis 前缀 JSONL 导出子项；其他未覆盖矩阵项继续按计划补验。

同日继续在同一真实窗口切换 `db-test-mysql`、`db-test-mongo` 和 `db-test-redis` 标签。切回
MySQL 后查询工作区和 `type_matrix` 对象树仍可见；切回 MongoDB 后 `ramag_demo.type_matrix`
及 `string-id` 行筛选仍可见；切回 Redis 后 DB0 的 `list:1` 详情仍可见。三个标签的连接状态
均保持“已连接”，未重新创建连接或修改测试数据。该操作关闭连接切换子项；失败刷新保留旧树和其他未覆盖驱动项
仍按矩阵记录。

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
不能替代刷新动作失败时保留旧树的验收，后者在下一段完成；受限角色的权限/空状态不在本步骤验证。

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
PostgreSQL 受限角色的权限/空状态在下一段隔离服务切片补验。

同日使用 WSL `Ubuntu-24.04` 的 Docker PostgreSQL `17-alpine`（Windows 端口 `25432`）建立临时
`ramag_ui_ro` 角色，角色仅保留登录和数据库连接能力，没有超级用户、建库、建角色、复制或绕过行级安全
权限。使用该角色连接 `ramag_test` 后，`public` Schema 的 `information_schema.tables` 查询结果为 `0`，
而 `pg_catalog.pg_collation` 和 `pg_catalog.pg_roles` 仍分别返回 `913` 和 `17` 条，说明 PostgreSQL
目录读取不因该角色限制而变成权限拒绝。Windows Debug 隔离数据目录的真实窗口显示 `public（空）`，
展开 Server Objects 后显示 `collations 913`；对象搜索 `users` 返回 `users 17`，搜索角色名返回
`ramag_ui_ro · LOGIN`，连接测试显示“连接成功”。本次未复现 Server Objects 的权限拒绝状态，也未把
目录可读误报为空结果；因此该切片只确认“空表树 + Server Objects 可读”的数据库边界，权限拒绝状态仍
没有可记录的真实证据。复核后删除隔离连接并删除临时角色，角色回读计数为 `0`；四个 WSL Docker
测试容器均保持 `healthy`。本切片只更新验收记录，Rust 测试不适用。

同日继续使用 WSL `Ubuntu-24.04` Docker PostgreSQL `17-alpine`（Windows 端口 `25432`）验证直接权限错误。
为隔离测试创建临时角色 `ramag_ui_denied`，只授予登录和数据库连接，撤销其对 `pg_catalog.pg_roles` 的读取权限；
Windows MSVC Debug 真实窗口连接成功后，`public` Schema 显示“（空）”，展开 `Server Objects` 后显示红色
“Server Objects 刷新失败：查询执行失败：权限不足…”提示，连接状态仍为“已连接”。该结果与前一段“空表树 +
Server Objects 可读”不同，关闭 PostgreSQL Server Objects 直接权限拒绝子项。验收结束后恢复
`pg_catalog.pg_roles` 的公共读取权限、撤销数据库 CONNECT 并删除 `ramag_ui_denied`；回读
`role_remaining=false` 且 `ramag` 仍可读取角色。本次只更新验收记录，Rust 测试不适用。

同日继续使用 WSL `Ubuntu-24.04` Docker MySQL `8.4`（Windows 端口 `23306`）补验 Virtual views。
`db-test-mysql` 真实窗口展开 `Virtual views` 后显示 `sessions`，行说明为“只读会话快照”；打开后结果表
显示 `ID`、`USER`、`HOST`、`DB`、`COMMAND`、`TIME`、`STATE` 等会话字段和 5 行当前会话记录，
连接状态保持“已连接”。该结果使用独立的只读会话查询，未执行写入；当前对象不进入表编辑路径，
DDL 和结构比较入口保持不可用。点击对象树刷新后，表、Server Objects、Virtual views 和 `sessions` 仍保留，
结果表继续可用。本次关闭 MySQL `DB-RED-04` 的 Virtual views 只读会话快照子项；本次只更新验收记录，
Rust 测试不适用。

同日继续在 WSL Docker MongoDB `8.2` 的 `db-test-mongo` 标签补验刷新失败。展开 `ramag_demo` 后
显示 `capped_events`、`large_documents`、`metrics`、`orders`、`products`、`type_matrix` 和
`users` 7 个集合；停止 MongoDB 容器并点击刷新后，标签变为“连接失败”，左侧仍保留原有数据库和
7 个集合，同时显示“连接失败：无法选择服务端（请检查 host/port/TLS）”和“刷新”入口。恢复容器
并点击刷新后，标签恢复“已连接”，7 个集合继续可用。该切片关闭 MongoDB 刷新失败保留旧树子项；
Redis 最终连接失败提示在后续正确构建复验中完成。

同日首次补验 WSL Docker Redis `7-alpine`（Windows 端口 `26379`）时启动了过期的 `target\debug\ramag.exe`，
该运行不代表当前 MSVC target 的源码修复。`db-test-redis` 刷新前显示 DB0 的 8 类 Key 分组和 `45,014` 个 Key；
停止 Redis 容器后点击刷新，扫描期间左侧旧分组继续可见。等待超过 `90s` 仍未得到最终失败文案，因此点击
“停止扫描”；停止后旧树没有被清空。该次运行只记录了旧树保留和手动中断，不能作为修复后的失败提示证据。

随后使用正确的 Windows MSVC Debug 构建 `target\x86_64-pc-windows-msvc\debug\ramag.exe` 重新复验。
`db-test-redis` 刷新前显示 DB0 的 8 类 Key 分组和 `45,014` 个 Key；停止 Redis 容器后点击刷新，
约 `10s` 内标签变为“连接失败”，底部显示“刷新失败”文案，8 类旧 Key 分组继续可见。恢复 Redis 容器并
再次刷新后，标签恢复“已连接”，底部回到 `45,014` 个 Key，分类树继续可用。该切片关闭 Redis 刷新失败
保留旧树、最终失败反馈和服务恢复子项；隔离应用与临时数据已清理。

同日对上述 Redis 刷新等待过长的问题做源码复核。`redis 0.32.7` 的 `ConnectionManager` 默认
包含 6 次内部重连，`RedisService` 在连接失败后还会清池并重试一次；两层重试叠加后，真实窗口
在服务停止超过 `45s` 时仍处于“连接中”。在 `crates/ramag-infra-redis/src/pool.rs` 将连接管理器
内部重连次数设为 `0`，保留服务层清池后的单次重试，使扫描失败可以更快进入已有的“刷新失败”
分支，同时继续保留旧树。`cargo fmt --all -- --check`、`cargo test --locked -p ramag-infra-redis --lib`
（46 项通过）、`cargo clippy --locked -p ramag-infra-redis --all-targets -- -D warnings` 均通过；
Windows MSVC Debug 构建在当前 PowerShell 进程显式使用已安装的 Windows SDK `10.0.26100.0` 后
通过。Redis 容器已恢复 `healthy`，隔离应用和临时数据已清理。

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

同日复核采样器实现时确认，GPUI 在绘制帧存在时把平台提交作为
`IntervalBoundary::Presented` 记录，不生成独立的 `ForegroundEvent::Present`。此前采样器只从后者
收集提交耗时，所以真实窗口会出现 `present_events=0`，但已有 `presented_boundaries` 没有被用于
`present_p95`。本次让 `Presented` 边界中的提交时长进入同一有界样本集，保留独立事件计数的原含义，
并增加回归测试。`cargo test --locked -p ramag-ui --features ui-profiling --lib profiling` 的 7 项、
默认 `ramag-ui` 单元测试的 167 项、默认严格 Clippy 和 Windows MSVC `ui-profiling` Release 构建均通过。
带 `ui-profiling` 的 Clippy 仍会命中原有 profiling 测试中的 14 个 `expect_used`，该问题不由本次新增
逻辑引入，也不扩大为无关测试重构。修复后仍需重新取得点击到可见反馈和 10 秒空闲动画观察，
`RAMAG-UI-001AC-2` 继续保持未完成。

随后在同一 Windows 11、NVIDIA GeForce RTX 3060、`2560×1440`、系统报告 `59Hz` 的环境中，
使用包含该修复的 Windows MSVC Release 构建重新采样 `60.070s`。真实窗口先在数据库客户端和系统监控
页面间执行固定标签切换，再保持系统监控页面运行；采样日志为
`.ramag/ui-acceptance/2026-11-06-release-profile.stderr.log`。结果为 `560` 个输入事件、`364` 个 draw、
`363` 个同窗口 Presented 边界，`input_p95_us=239`、`draw_p95_us=8599`、`present_p95_us=1090`，
`journal_lost=0`、`duration_samples_dropped=0`、`overflow=false`，且 `invalid_presented_pairs=0`。
该次运行关闭了活动帧数量、同窗口平台提交关系和 platform present p95 的缺口；仍没有屏幕合成器时间或
点击到可见反馈追踪，也没有单独记录动画结束后的 10 秒空闲观察，因此 `RAMAG-UI-001AC-2` 继续保持未完成。

同日使用相同 Windows MSVC Release 构建执行独立空闲观察。启动采样后切换到设置页，在页面完成切换后
保持鼠标和键盘不动；原始日志保留在 `.ramag/ui-acceptance/2026-11-06-idle-profile-settings.stderr.log`。
采样总时长为 `15.118s`，页面切换后的最后一次 draw 起点约为 `3.122s`，其后约 `11.996s` 没有新的
draw 事件；`journal_lost=0`、`duration_samples_dropped=0`、`overflow=false`。该记录关闭独立 10 秒
空闲观察子项，但启动绘制的 p95 不用于活动帧性能结论，点击到可见反馈仍缺少显示合成器或同步录像证据，
所以 `RAMAG-UI-001AC-2` 继续保持进行中。

### 4.5 现有页面交互补验

以下切片分别记录操作、结果、环境和未覆盖状态。先复用已有 Headless GPUI 测试，
再补真实窗口证据；发现可复现缺陷后才修改对应代码。

2026-11-06 修复 MongoDB 查询结果工具栏的视觉不一致。原列过滤编辑器使用多行 `EditorState` 的默认
滚动条，在单行过滤框内显示垂直分隔条；行过滤输入也没有使用共享筛选外框。修复后列过滤编辑器关闭
换行、隐藏编辑器滚动条并保留补全能力；列过滤和行过滤统一使用 `filter_field` 外框，工具栏复用
`responsive_toolbar`、边框和最小可用宽度。Windows MSVC Debug 真实窗口显示红框内分隔条消失，两个
过滤框高度和基线一致，MongoDB `type_matrix` 2 行数据仍可见。Headless layout test 覆盖 `360x240`、
`360x280`、`1024x420`、`1440x420`；`ramag-tool-mongodb` 105 项测试、严格 Clippy 和 Debug 构建通过。
WSL Docker MongoDB `8.2`（Windows 端口 `27028`）健康，验收只使用隔离测试数据，未触碰用户数据或系统
剪贴板。该切片关闭 MongoDB 结果工具栏的内部滚动条和布局不一致问题；数据库驱动×动作矩阵仍按
`A-UI-REAL-1` 继续收敛。

2026-11-06 在 Windows 发布构建真实窗口中补验系统监控的 Disk 和 Network 页面。Disk 页显示当前
`C:\` 文件系统已用约 `248.1 / 299.8 GiB`、读写速率曲线和底层设备信息；该页面没有额外磁盘选择器，
页面内容在当前窗口内完整显示，因此没有滚动操作，也没有改变磁盘配置。Network 页默认显示 `以太网`，
接口选择器列出 `Teredo Tunneling Pseudo-Interface`、`vEthernet (Default Switch)`、
`vEthernet (WSL Hyper-V firewall)` 和 `以太网`；切换到前两个接口后分别观察到零速率和累计流量，
再切回 `以太网`，网络连接未被修改。未复现设备不可用提示。该记录关闭了 Disk/Network 页面可见性和
Network 接口选择/切换子项，但磁盘选择器、溢出滚动和不可用设备提示仍缺少适用或失败证据，
`RAMAG-UI-001A` 继续保持进行中；本次只读补验且未改源码，Rust 测试不适用。

同日尝试使用 Windows MSVC Release 构建补验 `RAMAG-UI-001A`。Release 构建不启用连接数据目录隔离，
启动后显示已有数据库工作区；为避免触碰用户连接和偏好，未继续操作并停止进程，临时系统目录已清理。
本次没有取得 Disk/Network 的新增窗口证据，磁盘选择器、溢出滚动和设备不可用提示继续保持未完成。
`cargo test --locked -p ramag-tool-system --lib -- --test-threads=1` 的 `114` 项均通过；这些测试覆盖
合成快照、设备选择状态和不可用文案，不能替代缺少真实窗口前提的磁盘选择器与设备失败证据。

同日使用 Windows MSVC Debug 构建和隔离的 `RAMAG_SYSTEM_STATE_DIR` 补齐 `RAMAG-UI-001A`。
在真实窗口的 Disk 页打开设备选择器，看到 `C:\`、`D:\`、`E:\`、`F:\` 和 `G:\`，切换到 `F:\` 后容量读数、读写速率和历史曲线随设备更新；向下滚动后看到“底层设备信息”和页面说明，确认内容溢出时可滚动。
随后停止窗口，在隔离 `workspace.json` 中保存不存在的 `volume:missing:test-device`，重新启动同一 Debug 构建。
Disk 页显示按钮“不可用 · volume:missing:test-device”和卡片“所选设备不可用”，并保留上方设备选择入口；本次未修改真实磁盘、网络或用户系统状态。
该切片关闭磁盘选择器、页面溢出滚动和设备不可用提示三个子项，`RAMAG-UI-001A` 标记为已完成；本次只读验收且未改源码，Rust 测试不适用。

同日使用 Windows MSVC Debug 构建和隔离数据目录补验 `RAMAG-UI-001B` 与 `RAMAG-UI-001C`。
剪贴板页显示“采集已关闭”和 `0` 条记录；为避免监听、读取或写入用户系统剪贴板，未伪造真实记录，
搜索、删除、撤销和当前结果回读继续保持未完成。首页先将“数据库客户端”拖到“API 测试”之后；
随后在侧栏将“API 测试”拖到“数据库客户端”之前，两个拖动都改变了可见顺序。两次将工具拖到窗口外后，
当前顺序均保持不变。重启同一隔离配置后，首页和侧栏均回读“API 测试”在“数据库客户端”之前。
该切片关闭 `RAMAG-UI-001C`；本次只读验收且未改源码，Rust 测试不适用。剪贴板 headless 测试
`cargo test --locked -p ramag-tool-clipboard --lib -- --test-threads=1` 的 `28` 项均通过，
但测试使用合成数据和不会读取系统剪贴板的驱动，不能替代 `RAMAG-UI-001B` 的真实窗口证据。

2026-11-06 使用同一 Windows MSVC Debug 构建和独立 redb 数据目录补齐 `RAMAG-UI-001B`。
测试程序先通过 `ramag-infra-storage` 的临时测试入口写入 4 条文本记录：3 条普通记录和
`delete-me clipboard sentinel`；测试入口使用系统凭据库中的现有 Ramag 主密钥，临时源文件在数据写入后删除。
真实窗口打开剪贴板页后，将搜索词设为 `sentinel`，结果收敛为 1 条；点击删除后显示“已删除该条目”
和“撤销”，结果区显示没有匹配项。点击“撤销”后，同一搜索重新显示该记录；清空搜索后回读 4 条记录，
点击恢复的记录后，详情区显示 `delete-me clipboard sentinel`、类型“文本”和字节数 `28 B`。
本次没有启用采集，不读取或写入用户系统剪贴板，也没有复制测试内容到系统剪贴板。隔离数据目录和应用进程
已在验收后清理；临时 fixture 测试 1 项通过。该切片关闭 `RAMAG-UI-001B`，本次未修改产品源码，
Rust 产品测试不适用。

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

2026-11-06 在 WSL `Ubuntu-24.04` 对提交 `d76d478460472cf5407093a17a40e60ee5c5990b` 执行
`cargo build --locked --workspace --all-targets`。稳定 Rust `1.99.1` 的 host/target 为
`x86_64-unknown-linux-gnu`，构建耗时约 `6m20s`，exit code 为 `0`。本次只做 Linux 编译，未在
WSL 重复运行测试；临时构建过程未修改仓库文件。

同一提交通过 GitHub Actions run `42139276053` 的三项质量任务：Linux x86_64 job
`127635730149`、Windows x64 job `127635730137`、macOS job `127635729809` 均为
`completed/success`。Linux 和 macOS 测试使用 CI 结果记录，未用 WSL 或 Windows 结果替代。
本轮增加 `.github/workflows/desktop-ci.yml` 的 `workflow_dispatch` 入口，保留原有 push/PR 路径
过滤和质量步骤。由于仍缺少 Linux/macOS 原生窗口、系统凭据和独立测试账户证据，
`RAMAG-PLATFORM-001` 继续保持进行中。

### 4.7 发布维护

`RAMAG-RELEASE-001` 按需检查 Windows、Linux、macOS 质量结果、真实窗口、安装包、SHA-256
和更新清单。Linux/macOS 秘密存储、签名、公证、DPI 与多显示器须有各自的环境证据。
不移动已公开标签；发现发布缺陷时使用新提交和新版本修复。

### 4.8 2026-11-06 审查后的下一步

| 顺序 | 主线动作 | 通过条件 | 验证和回滚 |
|---|---|---|---|
| 1 | `A-UI-REAL-1` 收敛剩余驱动×动作矩阵 | 对 `04-ui-acceptance-standard.md` 中尚未有记录的组合，写明驱动、动作、结果和“不适用”理由；直接权限拒绝只有在安全测试账号和真实服务可用时才尝试 | WSL `Ubuntu-24.04` Docker、Windows MSVC Debug 真实窗口；只清理本轮创建的测试对象；无源码变更时 Rust 测试不适用 |
| 2 | `RAMAG-UI-001AC-2` 点击到可见反馈证据 | 使用显示合成器追踪或与输入时间同步的视频，取得点击到首次可见反馈的 p95；保持现有动画时长，不能用减少动画代替流畅度证据 | Windows Release `ui-profiling` 采样和真实窗口；若平台不能提供显示时间，记录缺口，不修改渲染代码 |
| 3 | `RAMAG-PLATFORM-001` 原生平台验收 | 在独立 Linux/macOS 测试账户中完成窗口启动、系统凭据保存、退出重启回读和密钥不可读错误反馈 | Linux/macOS 使用 GitHub CI 做测试；原生窗口和系统凭据需要对应平台环境；缺少环境时保持未完成 |
| 4 | `RAMAG-RELEASE-001` 触发发布评估 | 只有新增源码或用户明确要求发布时，才按标签、质量任务、安装包、SHA-256 和更新清单生成新版本 | 新提交、新标签和新发布物；不移动或覆盖公开标签 |

上述顺序是当前唯一主线队列。完成一个动作后，在本文件追加结果并更新表格状态；不创建并行计划或专项计划。

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

Rust 变更从 workspace 根目录执行。当前 Windows 主机已配置 MSVC 环境，不执行仓库的
`enable-msvc-toolchain.ps1`。只有在环境验证失败或需要建立干净的 CI 仿真进程时，才单独处理工具链初始化。
先确认 stable Rust 使用 MSVC 主机：

```powershell
rustup run stable rustc -Vv
# 预期 host 为 x86_64-pc-windows-msvc
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads=1
cargo build --locked -p ramag-bin
./scripts/windows/check-source-size.ps1
git diff --check
```

Linux 编译使用 WSL 中的仓库路径和 Linux stable Rust。Linux/macOS 测试不在本机替代执行，
由 `.github/workflows/desktop-ci.yml` 的对应 job 提供结果。CI 结果记录仓库、运行编号、提交和 job 状态。

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
