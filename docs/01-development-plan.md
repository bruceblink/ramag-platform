# Ramag Platform 主线开发计划

> 状态：唯一现行开发计划
> 更新日期：2026-11-13
> 代码核对基线：提交 `bd5e05e0`；历史代码审查基线 `1aacf006`、`8820c77a` 以及后续功能和验收记录均已保留在本计划
> 当前主线代码基线：以 `main` 当前提交为准；workspace 版本 `0.9.2`
> 后续开发基线：当前 `main` 分支（`v0.9.2` 发布包从标签提交 `37ae32b5` 构建）
> 计划状态：`v0.9.2` 已正式发布，资产、校验清单和更新清单均已核对。数据库适用组合、Windows UI 回归和 Windows 发布构建回归已完成。当前只保留性能证据、跨平台证据和触发式发布维护，不自动恢复历史功能设想

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
- 第一阶段第一方工具采用编译期静态 Rust/GPUI 插件。插件接口设计、系统监控试点和九批工具迁移已进入代码基线。
- 新增业务能力按插件模块交付。平台核心负责通用注册、生命周期、授权、设置、任务、诊断和 UI 宿主服务。
- 当前第二阶段已开始建立可安装插件的清单和本地状态模型。清单解析、兼容性校验、安装状态持久化和设置页展示先于外部代码执行。
- 动态库加载、第三方插件包的正式执行策略、WASM 执行、插件签名、沙箱、更新回滚、插件市场和远程分发仍未实现；当前仅有进程外协议和本地归档安装的最小基础。
- 维护或移除工具时保留其他工具的用户配置、凭据、历史和数据库文件。

插件接口、可安装清单和维护方法统一见[插件接口与开发手册](05-plugin-development-guide.md)。

## 3. 已有基线

以下内容用于防止重复安排已完成工作，不是待执行队列。

| 领域 | 已有结果 | 证据边界 |
|---|---|---|
| 工具入口与列表 | 工具显示位置管理、数据库数据源列表、对象存储账号列表、Container/VCS/插件列表已完成对应切片 | 完整流程按工具分别验收；不扩展为所有错误与平台均已通过 |
| 连接工作区 | API 首次读取状态修复、双栏布局、两条分隔栏真实拖动，以及 API/Kafka/MQTT/SSH 全局回归已完成 | 专项数据库操作和其他未列出的原生流程仍按下节补验 |
| 数据库 | MySQL/PostgreSQL/SQLite 连接、查询与部分元数据树，触发器子树、连接恢复、事务提交/回滚读回、单元格 UPDATE 以及行内 DELETE、批量 DELETE、INSERT 的失效连接反馈已有真实窗口证据；SQLite 活动事务期间的连接池重试保护已修复并复核；SQLite 结构比较、迁移预览、执行和目标元数据回读已补齐 | `DB-RED-01..07` 的适用组合已完成；MongoDB/Redis 的 SQL 事务、DDL 和结构迁移按驱动能力标记为不适用 |
| 数据库规模与取消 | PostgreSQL 100,000 行分页、10,000 行只读查询、1,000 张表、长表头、双轴滚动、多标签隔离与页面/统计查询取消已验收 | 取消到服务端活动查询归零曾测得约 `629 ms`；这是单次环境记录，不能作为普遍时限 |
| 共享 UI 与监控 | System Pulse 迁入、公共样式、状态反馈、首页响应式网格及卡片视觉维护已进入基线；过渡时长统一为重排 `180ms`、加载 `160ms` | 页面功能、硬件准确性、发布性能和跨平台结果分别判断 |
| 静态插件 | 系统监控试点及 API、剪贴板、VCS、SSH、对象存储、容器、数据库、Kafka、MQTT 九批迁移完成 | 静态装配和代码验证不表示支持外部插件加载 |
| MQTT 独立文案 | 本地服务页已移除外部项目引用，保留配置生效时间和网络暴露提示 | MQTT 库测试 33 项通过；Windows 隔离预览窗口已核对文案，未启动 Broker |
| 更新检测与下载 | 自动检测开关、手动检查、进度、取消、错误反馈和已校验文件入口已实现；支持可选 BSDIFF40 差分，失败时回退完整包 | 2026-11-01 在 Windows 隔离开发构建中验证正式 `v0.9.0` 的发现、下载、取消、错误及目录打开；完整下载与差分重建文件的 SHA-256 均匹配正式安装包。该构建标记为 `0.8.0`，包含新的更新实现，不是正式 `v0.8.0` 安装包 |
| 更新页面反馈 | 已校验下载切换“安装安装包”；手动检查无更新时显示明确结果；整理版本信息与操作区，复用 Ramag 品牌标识并修正产品说明 | 2026-11-01 Windows 明暗主题真实窗口已核对标识、说明、布局和无更新提示；Headless GPUI 验证下载到安装按钮的切换及回调。未执行正式安装器，不能据此认定覆盖安装已验收；这些修复尚未包含在 `v0.9.0` 安装包中 |
| 发布 | `v0.9.2` 已正式发布 | 2026-11-07 核对 [v0.9.2 Release](https://github.com/bruceblink/ramag-platform/releases/tag/v0.9.2) 及 [发布任务 `42289929133`](https://github.com/bruceblink/ramag-platform/actions/runs/42289929133)：Linux、macOS、Windows 打包和发布任务均成功；Release 为正式版本，包含 5 个安装包、4 个差分包、`SHA256SUMS.txt` 和 `update-manifest.json`。9 个资产的 SHA-256 均重算匹配，更新清单的 5 个安装包名称、大小和标签均与 Release 一致。标签 `v0.9.2` 是附注标签，解析到提交 `37ae32b5` |

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
后续开发以提交 `bd5e05e0` 为基线；`v0.9.2` 标签已推送，发布包从标签提交 `37ae32b5` 构建，
`RAMAG-RELEASE-001` 本次发布维护已完成。不移动或覆盖已公开标签。
更新条件和缓存限制见[更新检测与下载](06-desktop-release.md#更新检测与下载)。

按下表顺序选择独立切片。顺序表示优先级，不表示所有任务必须串行。
每项先确认环境和现有证据，再实施、验证并记录结果。缺少某项前提时，记录阻塞原因，
继续处理不依赖该前提的任务；不以其他证据代替。新出现的明确缺陷可单独调整优先级。

| 顺序 | 任务 | 状态与依赖 |
|---|---|---|
| 1 | `RAMAG-UI-001AC-1` 补充测量方法 | 已完成真实相对时间、同窗口绘制与提交关系、乱序/溢出/无样本/提前退出记录；输入事件的窗口标识继续明确保留为缺失 |
| 2 | `DB-UX-006B-R` 大字段性能复核 | 已完成查看器代码修复、本机数据库集成复核和三轮真实窗口打开/关闭复核；响应性通过，内存采样边界已记录 |
| 3 | `A-UI-REAL-1` 数据库导航与查询验收 | 已完成（适用组合）；SQL 对象树搜索、收藏、仅收藏、最近访问和重启保存已覆盖 PostgreSQL/MySQL/SQLite；SQLite 表、视图、列、索引、触发器、DDL、只读视图、JSONL、结构差异、迁移预览、执行和回读已补验；PostgreSQL/MySQL/SQLite 选中行 JSONL、MongoDB 选中行 Extended JSONL、Redis 数据库/单 Key/前缀 JSONL 已回读；MongoDB/Redis 专用对象、刷新失败保留旧树、跨连接切换、Server Objects、Virtual views、查询过滤排序分页及 MongoDB 工具栏样式已有对应真实窗口记录；MongoDB/Redis 的 SQL 事务、DDL、结构迁移按源码能力标记为不适用。 |
| 4 | `A-UI-REAL-2` 数据库写入与迁移验收 | 已完成；PostgreSQL、MySQL 和 SQLite 手动事务提交、回滚及清理，SQLite 活动事务期间的连接池重试保护，MySQL DDL、结构差异、迁移执行、目标元数据回读、EXPLAIN ANALYZE 风险确认、失败查询草稿恢复，以及行内 DELETE、批量 DELETE、INSERT 的失效连接反馈均已有真实窗口证据 |
| 5 | `RAMAG-UI-001AC-2` 发布构建性能验收 | 条件等待；Windows Release 的 `input/draw/present p95` 已取得，采样无丢失或溢出；现有录像约 `6.44fps`，NVENC 约 `12.79fps`，点击到可见反馈估计受采集量化影响，不能判定 `100ms` 条件。下一次只在取得显示合成器时间或可用的高帧率同步录像后继续；动画时长保持重排 `180ms`、加载 `160ms`。 |
| 6 | `RAMAG-UI-001A` 监控交互补验 | 已完成；Windows 真实窗口已核对 Disk 页和 Network 接口选择/切换、磁盘选择器、页面溢出滚动及设备不可用提示 |
| 7 | `RAMAG-UI-001B` 剪贴板交互补验 | 已完成；隔离 redb 数据和真实窗口已覆盖搜索、删除、撤销、当前结果回读和详情查看，未读取或写入用户系统剪贴板 |
| 8 | `RAMAG-UI-001C` 首页与侧栏排序补验 | 已完成；隔离配置下已检查首页和侧栏拖动、拖出取消、保存及重启后的顺序回读 |
| 9 | `RAMAG-PLATFORM-001` 跨平台补验 | 条件等待；WSL Ubuntu-24.04 的 Linux workspace 编译入口已验证，Linux Secret Service、macOS Keychain 和原生窗口 Smoke 入口已进入 CI 配置。Linux/macOS 的退出重启回读、缺少密钥错误、独立测试账户和真实窗口证据仍未取得。普通开发不读取 GitHub CI 状态，只有发布 release 或用户明确要求时才核对对应运行。 |
| 持续 | `RAMAG-RELEASE-001` 平台与发布维护 | `v0.9.2` 本次发布已完成；后续有相关变更或用户明确要求发布时再执行 |

表中已完成项只保留结果和证据边界。当前只推进“条件等待”中已经具备环境的动作，以及用户明确提出的新缺陷。持续维护和 release 按触发条件启动。每个任务的结果只能覆盖实际执行的平台、驱动和步骤。
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
- `DB-RED-06` / `DB-UX-004B-3B`：失败查询草稿保留和恢复，SQLite 手动事务提交/回滚读回，以及行内 DELETE、批量 DELETE、INSERT 的失效连接反馈均已有真实窗口证据。SQLite 活动事务期间跳过连接池失效和重复读重试，避免在 Tokio 外释放 `PoolConnection`；原生 UPDATE 已通过，其他路径的 Headless GPUI 结果不能替代窗口操作。
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

同日继续使用 WSL `Ubuntu-24.04` Docker MySQL `8.4`（Windows 端口 `23306`）补验查询结果矩阵。
在 `ramag_test` 创建临时表 `ui_acceptance_query_20261106` 并插入 205 行；Windows MSVC Debug
真实窗口打开该表后，第一页显示 `1-100 of 205`，下一页显示 `101-200 of 205`，末页显示
`201-205 of 205`。点击 `sort_value` 列头后工具栏显示 `ORDER BY sort_value DESC`，首行变为
`id=1`、`sort_value=205`；输入 `amount >= 10` 后结果显示 `1-84 of 84`，列过滤输入 `id,label`
后网格只保留两列且仍为 84 行。连接状态全程为“已连接”，本次没有执行写入；验收结束后删除临时表，
MySQL 回读 `remaining=0`，容器保持 healthy。本次关闭 MySQL `DB-RED-05` 的排序、行过滤、列过滤
和多页分页子项；本次只更新验收记录，Rust 测试不适用。

同日继续使用该隔离 SQLite 文件补验手动事务。第一次 Windows MSVC Debug 真实窗口验收在后台列元数据
读取超时后触发了 `sqlx` 的 Tokio 上下文 panic；日志确认 SQLite 单连接池被手动事务占用，重试路径在
UI 执行器中清理事务句柄是具体原因。提交 `1aacf006` 增加活动事务检测：连接池失效和事务清理在活动
事务期间跳过，读操作也不再重复重试；`prefetch_columns_for_used_tables` 在手动事务期间直接跳过。
共享事务层和 SQLite 驱动回归测试、应用层 `284` 项单元测试、严格 Clippy、Windows Debug 构建均通过。
随后在同一真实窗口中把 `amount` 从 `42` 改为 `43` 并提交，重新打开表读回 `43`；第二个手动事务把值
改为 `42`，应用修改后回滚，重新打开表仍读回已提交值 `43`。窗口和连接全程保持可用，最终日志没有
新的 `application_panic` 或事务期间的池失效重试。验收结束后停止进程并将隔离文件恢复为 `amount=42`、
`rows=205`；该切片关闭 SQLite 手动事务提交、回滚和活动事务重试保护子项。

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

2026-11-07 使用 Windows MSVC Debug 构建和隔离副本
`.ramag/ui-acceptance/matrix-20261107-01` 补验 PostgreSQL 与 MySQL 的跨连接切换。真实窗口先切到
`db-test-postgres`，确认 `ramag_test/public`、表树和“已连接”状态；再切到 `db-test-mysql`，确认
`ramag_test`、表树和“已连接”状态；两个标签的连接上下文和对象树均未串写。该结果补充 PostgreSQL
参与跨连接切换的真实窗口记录，没有执行写入。隔离副本同时新增了 MongoDB 测试连接并通过可达性测试，
但打开元数据时返回 MongoDB `code=13` 权限错误；原因是本轮表单未填写 Docker 测试服务要求的固定账号，
该次失败不计入 MongoDB 通过证据，也不修改产品数据。应用进程已停止，测试服务保持运行。

同日按源码适用性复核 `DB-RED-03/04/06/07` 的剩余组合，并保留真实窗口证据与源码边界的区别。
`Driver::list_server_objects` 和 `Driver::list_virtual_views` 的默认实现返回不支持；当前只有
PostgreSQL/MySQL 驱动提供 Server Objects 和 Virtual views，因此 SQLite、MongoDB、Redis 的
`DB-RED-03/04` 组合标记为“不适用”，不把没有该对象模型误记为失败。`DriverKind::supports_transactions`
只对 MySQL、PostgreSQL、SQLite 返回真，MongoDB/Redis 的 `DB-RED-06` 标记为“不适用”，不显示
SQL 手动事务控件。`build_ddl_query` 对 Redis/MongoDB 返回空字符串，表结构对比入口也只接受
MySQL/PostgreSQL/SQLite，因此 MongoDB/Redis 的 `DB-RED-07` 标记为“不适用”。这些结论来自
`ramag-domain` 和驱动实现的源码核查，不替代仍需取得的真实窗口结果；A-UI-REAL-1 继续只补验
实际存在且尚未记录的驱动×动作组合。

同日使用 WSL `Ubuntu-24.04` Docker MongoDB `8.2`（Windows 端口 `27028`）和隔离副本
`.ramag/ui-acceptance/matrix-20261107-02` 取得不适用项的真实窗口证据。打开
`ramag_demo.type_matrix` 集合后，结果工具栏显示文档过滤、排序、分页和导出；窗口没有 SQL 手动事务、
DDL 或表结构比较入口，连接状态保持“已连接”，2 条文档仍可读。该结果关闭 MongoDB 的
`DB-RED-06/07` 不适用窗口项；没有执行写入。Redis 的同类不适用项仍由源码边界和既有专用对象窗口记录支持，
不把 MongoDB 结果替代 Redis 证据。

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

2026-11-07 手动触发的 GitHub Actions run `42274299158` 在 Linux 和 Windows 质量任务成功后，
macOS job 被取消；GitHub job 记录的 `runner_id=0` 且没有执行步骤，不能作为 macOS 失败或通过证据。
随后重新触发 run `42281721656`。该 run 已完成，Linux x86_64 job `128112125964`、Windows x64 job
`128112125621` 和 macOS job `128112125101` 均为 `completed/success`。这组结果只证明对应 CI 质量任务通过，
不能替代 Linux/macOS 原生窗口、系统凭据和独立测试账户证据；`RAMAG-PLATFORM-001` 继续保持进行中。

### 4.7 发布维护

`RAMAG-RELEASE-001` 按需检查 Windows、Linux、macOS 质量结果、真实窗口、安装包、SHA-256
和更新清单。Linux/macOS 秘密存储、签名、公证、DPI 与多显示器须有各自的环境证据。
不移动已公开标签；发现发布缺陷时使用新提交和新版本修复。

2026-11-07 按用户决定将仓库根许可证恢复为 `GPL-3.0-only`。根 `LICENSE`、Cargo SPDX 元数据、README
和架构说明已同步；`ramag-infra-system` 继续保留其上游 `GPL-3.0-or-later` 文本和来源记录。
同日准备并发布 `0.9.2` 版本，Windows MSVC 格式、Clippy、构建和 profiling 定向测试通过；标签 `v0.9.2`
指向 `37ae32b5`，Desktop Release run `42289929133` 的 Linux、macOS、Windows 打包及发布步骤全部成功。

### 4.8 2026-11-07 后续主线重排

本节定义当前唯一的后续执行顺序。状态分为可执行、条件等待和触发维护。历史验收记录继续保留在本文件中，但不再自动生成新的开发任务。

| 优先级 | 主线任务 | 当前动作 | 通过条件 | 验证与回滚 |
|---|---|---|---|---|
| 1 | `RAMAG-UI-001AC-2` 可见反馈性能证据 | 保留现有动画和渲染实现。等待显示合成器追踪或高帧率同步录像条件满足后再采样 | 能把输入、绘制、提交和实际显示时间放在同一时间轴，并得到可复核的点击到可见反馈 p95。无法取得显示时间时记录缺口，不修改动画时长 | Windows Release 真实窗口、`ui-profiling` 日志和采集原始文件。回滚只涉及性能采样入口和证据记录 |
| 2 | `RAMAG-PLATFORM-001` Linux/macOS 平台证据 | 保留 WSL Linux 编译和 CI 测试入口。取得对应平台、独立测试账户或 release 验收条件后再执行 | 平台窗口启动、系统凭据保存、退出重启回读和缺少密钥错误均有对应平台证据。CI 测试不能替代真实窗口和独立账户证据 | Linux 使用 WSL 编译，Linux/macOS 测试使用 GitHub CI 或对应平台窗口。回滚只涉及本次平台测试入口和记录 |
| 3 | `RAMAG-RELEASE-001` 触发式发布维护 | 没有 release 请求时不创建版本、标签或发布任务。收到 release 请求后从当前主线生成变更说明、构建包、校验清单和更新清单 | 三平台质量检查、打包、资产摘要、更新清单和公开 Release 记录全部匹配 | 按[桌面端构建与发布](06-desktop-release.md)执行。公开标签不移动，缺陷使用新版本修复 |
| 4 | `RAMAG-UI-UPDATE-002` 更新下载反馈打磨 | 已完成：增加下载百分比和进度条，使用明确的“安装更新”文案；下载校验、取消和安装入口保持不变 | 已知总大小显示有界百分比和进度条；未知总大小显示准备状态；下载完成后显示安装更新；360px、1024px 和 1440px 宽度均不越界 | `ramag-ui` 更新页定向测试、严格 Clippy 和格式检查已通过。回滚只涉及更新页渲染与测试，不回滚下载逻辑 |
| 5 | `RAMAG-UI-ACCESS-001` 设置导航可访问性 | 已完成：增加按钮语义、稳定辅助技术标签、键盘焦点和焦点反馈；当前导航布局和鼠标点击行为保持不变 | 桌面纵向和窄窗口横向导航入口均可获得键盘焦点，辅助技术可区分入口名称，焦点反馈不改变布局边界 | `ramag-ui` 设置导航定向测试、严格 Clippy 和格式检查已通过。回滚只涉及导航渲染和测试 |
| 6 | `RAMAG-UI-ACCESS-002` 工具可见性开关可访问性 | 已完成：为 Home/侧边栏开关补充工具名称标签，并分离开关组与控件标识 | 辅助技术能区分工具和作用面；组标识和开关标识不重复；360px、1024px 和 1440px 布局均保持在卡片内 | `ramag-ui` 工具可见性定向测试、严格 Clippy 和格式检查已通过。回滚只涉及工具可见性渲染与测试 |
| 7 | `RAMAG-PLUGIN-FAILURE-001` 插件存储失败隔离验收 | 已完成：宿主在设置快照读取或秘密解封失败时阻止插件回调，标记插件失败并移除工具入口，同时保留初始化诊断 | 设置读取失败和秘密解封失败均生成 `Initialize` 诊断；插件回调不执行；注册表不保留失败插件；后续插件仍可继续初始化 | `ramag-app` 插件生命周期定向测试、严格 Clippy、格式检查和差异检查已通过。回滚只涉及宿主测试与插件测试清单中的敏感字段 |
| 8 | `RAMAG-DOCS-PLUGIN-001` 插件开发手册基线同步 | 已完成：将插件开发手册适用基线同步到当前 `0.9.2` 代码版本，并更新维护日期；静态插件改为第一阶段基线 | 手册入口版本与 workspace 版本一致；第一阶段静态基线、第二阶段可安装路线和未实现边界一致 | Markdown、UTF-8、LF 和差异检查已通过。回滚只涉及手册头部和本节记录 |
| 9 | `RAMAG-PLUGIN-PACKAGE-001` 可安装插件清单与本地状态 | 已完成：增加可序列化插件包清单、入口路径/能力/API/摘要校验、安装状态的串行原子持久化，并接入插件设置页展示；不启动外部代码 | 清单支持 `id`、版本、宿主 API、`process` 运行时、入口和 SHA-256；状态支持 `installed`、`enabled`、`disabled`、`quarantined`；启动后可恢复并在设置页展示；损坏文件、重复 ID、路径穿越和未知能力均拒绝 | `ramag-domain` 清单测试、`ramag-app` 安装状态测试、`ramag-ui` 插件页测试、`ramag-bin` 组合编译、严格 Clippy、格式检查和差异检查已通过。回滚只涉及清单模型、状态存储、组合根和插件页 |
| 10 | `RAMAG-PLUGIN-IPC-001` 进程外插件协议基础 | 已完成：增加长度前缀帧、JSON 信封、握手、入口调用、响应和关闭消息的纯协议层；不启动真实第三方包 | 帧长度、消息类型、插件 ID、API 主版本、能力集合和请求 ID 均校验；输入/输出使用 base64；零长度、截断、超帧、错误响应和协议版本不匹配都有确定错误；协议层不读取凭据、不执行 Shell | `ramag-app` 协议测试 8 项、严格 Clippy、格式检查和差异检查已通过。回滚只涉及协议模块、依赖锁文件和测试 |
| 11 | `RAMAG-PLUGIN-PROCESS-001` 进程外插件宿主最小闭环 | 已完成：安全解析包内入口，清空继承环境后直接启动进程，完成握手、串行入口调用、超时终止、关闭和有界 stderr 诊断；加入真实跨平台测试辅助进程 | 入口必须位于包根且不能经过符号链接；握手身份/API/能力匹配；调用超时会终止会话；关闭后不能继续调用；真实辅助进程完成握手、回显调用和关闭 | `ramag-app` 进程宿主单元测试、真实辅助进程测试、严格 Clippy、格式检查和差异检查已通过。回滚只涉及进程宿主、测试辅助进程和本节记录 |
| 12 | `RAMAG-PLUGIN-INSTALL-001` 插件包目录安装 | 已完成：读取 `plugin.json`，校验清单、包内入口、符号链接和有界包大小，核对目录摘要后原子替换版本目录并写入安装状态 | 摘要不匹配、源目录位于包目录、符号链接和不支持文件类型均拒绝；有效包完成复制、替换和状态记录；状态保存失败可恢复旧版本目录 | `ramag-app` 包安装定向测试、严格 Clippy、格式检查和差异检查已通过。回滚只涉及包安装模块、安装状态存储和本节记录 |
| 13 | `RAMAG-PLUGIN-UNINSTALL-001` 本地插件卸载与目录清理 | 已完成：校验已安装版本目录，先改名为同级临时备份，再删除安装状态；状态删除或备份清理失败时恢复目录和状态；设置页加入确认后的卸载入口 | 未知插件返回无变化；包目录缺失、符号链接和路径越界均拒绝；有效卸载同时移除状态和版本目录，不影响其他插件或版本；设置页确认后同步移除插件行 | `ramag-app` 包安装器定向测试 9 项、`ramag-ui` 插件页测试 4 项、组合编译、严格 Clippy、格式检查和差异检查已通过；macOS 隔离调试窗口已完成真实操作验收。回滚只涉及包安装器、安装状态存储、插件设置页、组合根和本节记录 |
| 14 | `RAMAG-PLUGIN-INTEGRITY-001` 启动前包摘要核验 | 已完成：进程宿主启动前按安装器相同规则重新计算包内容摘要，并与清单摘要比较；摘要不一致时不启动入口 | 安装后修改任意包文件会在启动前失败；摘要一致时真实辅助进程仍能完成握手、调用和关闭 | `ramag-app` 进程宿主单元测试 5 项、真实辅助进程测试 1 项、严格 Clippy、格式检查和差异检查已通过。回滚只涉及摘要复用、进程宿主、对应测试和本节记录 |
| 15 | `RAMAG-UI-PLUGIN-ERROR-001` 插件设置页错误反馈 | 已完成：复用设置页统一错误状态卡，补充图标、边框、错误标题和稳定调试选择器；插件最近错误与状态保存、卸载失败使用同一反馈层级 | 最近错误和插件操作错误在 360px、1024px、1440px 的明暗主题下均保持在安装状态区域内；错误原因仍可读且不改变状态逻辑 | `ramag-ui` 插件诊断定向测试 5 项、严格 Clippy、格式检查和差异检查已通过。回滚只涉及插件设置页反馈渲染、通用错误状态卡接口、对应测试和本节记录 |
| 16 | `RAMAG-PLUGIN-ARCHIVE-001` 受限本地 `tar.gz` 包安装 | 已完成：读取本地归档快照，先扫描再解包到临时目录，拒绝路径穿越、链接、特殊类型、重复路径、超量条目和超大解包内容，再复用目录安装器完成清单、摘要和状态校验 | 有效 `tar.gz` 能安装并写入状态；归档大小最多 `64 MiB`，条目最多 `1024` 项，解包内容最多 `64 MiB`；失败时不写入安装状态，临时目录由清理守卫回收 | `ramag-app` 归档安装定向测试 5 项、严格 Clippy、`ramag-bin` 组合检查、格式检查和差异检查已通过。回滚只涉及归档安装模块、安装器桥接、依赖锁文件和本节记录 |
| 17 | `RAMAG-UI-PLUGIN-ARCHIVE-001` 设置页本地归档安装入口 | 已完成：加入 `tar.gz`/`tgz` 文件选择器，后台执行归档安装，成功后更新或新增插件行，失败时显示统一错误状态卡；安装中按钮禁用并显示进行中状态 | 插件设置页在 360px、1024px、1440px 明暗主题下显示本地归档按钮且不越界；入口使用后台任务执行安装，避免把解包和摘要核验放在 UI 更新路径中 | `ramag-ui` 插件诊断定向测试 5 项、严格 Clippy、`ramag-bin` 组合检查、格式检查和差异检查已通过。文件选择器和后台结果回写的真实系统对话框未在本轮自动化窗口中操作，回滚只涉及插件设置页入口、对应测试和本节记录 |
| 18 | `RAMAG-PLUGIN-CATALOG-001` 本地插件目录清单解析 | 已完成：增加受限 JSON 目录清单和包候选校验；只解析元数据，不联网、不下载、不解包、不执行、不授予权限 | 清单最多 `1 MiB`、`256` 个条目；每项校验插件 ID、名称、SemVer、宿主 API、`process` 运行时、HTTPS 地址、归档 `archive_sha256`、能力集合和重复 ID/版本；继续兼容旧 `package_sha256` 字段 | `ramag-app` 目录清单定向测试 6 项、严格 Clippy、`ramag-bin` 组合检查、格式检查和差异检查已通过。回滚只涉及目录清单模块、依赖锁文件和本节记录 |
| 19 | `RAMAG-UI-PLUGIN-CATALOG-001` 设置页本地目录候选展示 | 已完成：启动时读取插件状态目录中的 `catalog.json`，校验成功后注入设置页并展示插件名称、ID、版本、API、能力和下载地址；损坏清单只显示空状态，不联网、不下载、不执行 | 候选区域在 360px、640px、1024px、1440px 窗口中保持边界；有候选和空目录均有明确标题、引导和稳定选择器；设置页展示不改变安装状态 | `ramag-ui` 插件诊断定向测试 5 项、`ramag-bin` 组合检查、严格 Clippy、格式检查和差异检查已通过。真实窗口目录文件读取未在本轮自动化窗口中操作，回滚只涉及组合根、插件设置页和对应测试 |
| 20 | `RAMAG-PLUGIN-DOWNLOAD-001` 有界 HTTPS 插件包下载 | 已完成：对已校验的 `PluginPackageCatalogEntry.archive_sha256` 使用生产构造器下载到新的本地缓存文件，限制 HTTPS、HTTP 200、禁止重定向、64 MiB 响应大小和最终目标父目录符号链接；流式计算归档 SHA-256，`flush`/`sync_all` 后原子发布；不调用安装器或执行包内容 | `Content-Length` 和流式累计均不得超过 `MAX_PLUGIN_DOWNLOAD_BYTES = 64 MiB`；最终目标父目录必须是普通目录，目标文件必须不存在；归档摘要不匹配、HTTP 错误、重定向和写入失败都清理当前临时文件并保留已有文件；原子发布不会覆盖并发出现的目标文件；生产 `new()` 的 TLS provider 由应用启动层预先安装 | 下载器定向测试 3 项、Docker HTTPS 集成测试 1 项、`ramag-app` 严格 Clippy、`ramag-bin` 组合检查、格式检查和差异检查通过；集成测试覆盖真实 `tar.gz` 成功落盘、SHA-256、404、302、64 MiB 超限、摘要错误和临时文件清理；回滚只涉及下载器、目录条目复用、集成脚本、依赖锁文件和本节记录 |
| 21 | `RAMAG-UI-PLUGIN-DOWNLOAD-001` 设置页目录候选下载入口 | 已完成：把已校验目录候选接入下载器，在每个候选行显示稳定下载按钮、待下载/下载中/已下载状态和统一错误状态卡；下载任务使用专用 Tokio runtime 和后台执行器，缓存根目录限制在插件状态目录下；不自动安装或执行 | 设置页在 360px、1024px、1440px 明暗主题下显示下载按钮且不越界；同一页面只允许一个下载任务；成功后按钮禁用并保留已校验缓存路径；下载失败保留候选行并显示错误原因；UI 不改变 `installed.json` | `ramag-ui` 插件诊断定向测试 8 项、`ramag-app` 全量测试 337 项、严格 Clippy、`ramag-bin` 组合检查、格式检查和差异检查已通过。真实 HTTPS 服务、真实窗口点击下载和下载后安装仍未验收；回滚只涉及下载器 runtime 入口、组合根注入、插件设置页和对应测试 |
| 22 | `RAMAG-PLUGIN-CATALOG-INSTALL-001` 下载缓存安全安装 | 已完成：安装前重新读取缓存归档并核对 `archive_sha256`，在临时解包目录逐项核对包内清单的 ID、名称、版本、API、运行时和能力集合，再复用现有摘要、入口、原子替换和状态保存流程；安装成功后设置页刷新安装状态并清理缓存 | 归档摘要或目录身份不匹配时不写入 `packages` 或 `installed.json`；能力顺序变化不影响匹配；安装失败保留缓存并显示错误；成功后候选显示“已安装”，缓存文件清理失败只记录诊断 | `ramag-app` 归档定向测试 6 项、目录清单定向测试 6 项、`ramag-ui` 插件诊断定向测试 8 项、严格 Clippy、`ramag-bin` 组合检查、格式检查和差异检查已通过。真实 HTTPS、真实窗口点击和跨进程安装仍未验收；回滚只涉及归档安装器、目录清单字段兼容、插件设置页和对应测试 |
| 23 | `RAMAG-PLUGIN-CACHE-001` 下载缓存跨重启恢复 | 已完成：为下载归档增加受限原子索引；启动时按本地目录候选核验索引身份、单层文件名、普通文件、64 MiB 单文件上限、256 MiB 单次恢复总量和归档 SHA-256，恢复有效缓存的“安装”动作；失效记录只从恢复结果中排除，不写入安装状态 | 索引格式、条目数量和字节数受限；相对路径、路径穿越、索引/归档符号链接、目录、摘要修改和未知目录条目均不进入 UI；安装成功同步删除索引记录和归档；缓存恢复失败只记录警告，不阻止应用启动 | `ramag-app` 缓存定向测试 8 项、`ramag-ui` 插件诊断定向测试 8 项、`ramag-bin` 组合检查、严格 Clippy、格式检查和差异检查已通过。旧随机命名缓存不迁移；真实窗口和跨进程恢复仍未验收；回滚只涉及缓存索引、组合根、设置页注入和对应测试 |
| 24 | `RAMAG-UI-PLUGIN-CATALOG-RESPONSIVE-001` 插件目录候选窄窗口与长文本布局 | 已完成：候选行在窄窗口启用换行，长能力文本允许正常换行，下载地址使用稳定选择器并限制为单行省略；下载、安装、已下载和已安装状态使用不同颜色 | `360×520` 明暗主题下，候选行、下载地址、状态和动作按钮均位于目录区域和根窗口内；长文本不改变下载和安装动作 | `ramag-ui` 插件诊断定向测试 9 项、严格 Clippy、格式检查和差异检查已通过；真实窗口点击仍未验收；回滚只涉及插件目录候选渲染和对应测试 |
| 25 | `RAMAG-PLUGIN-CATALOG-FETCH-001` 远程插件目录获取与本地保存 | 已完成：增加显式的受限 HTTPS 目录获取器和本地原子存储；只在调用方主动请求时联网，启动流程继续读取本地 `catalog.json` | 目录源仅允许 HTTPS、无用户信息/查询/片段；连接 10 秒、总请求 20 秒、禁止重定向、HTTP 200 和 1 MiB 响应上限；解析成功后才原子替换本地清单，失败保留旧文件；不签名、不自动安装、不执行 | `ramag-app` fetcher/store 默认定向测试 8 项通过；Docker `nginx:1.27-alpine` HTTPS fixture 通过 200 保存、404、302、超量和失败后旧文件保留验收；严格 Clippy、格式检查、组合检查和差异检查通过。设置页真实 HTTPS 点击、第三方证书链、签名和来源信任仍未验收；回滚只涉及获取器、存储器、集成脚本、应用导出和本节记录 |
| 26 | `RAMAG-UI-PLUGIN-CATALOG-SOURCE-001` 插件目录源设置与手动刷新 | 已完成：设置页保存并恢复目录源 URL，用户点击“刷新目录”后才请求受限 HTTPS 清单；成功后原子替换本地 `catalog.json` 并刷新候选，失败保留旧候选和旧文件 | 目录源输入可编辑且使用有界 URL；获取器或存储器未配置时刷新按钮禁用；请求不在启动路径执行；成功后保存 URL 偏好，错误显示在目录区域；360px、1024px、1440px 明暗主题布局不越界 | `ramag-ui` 插件诊断定向测试 10 项、真实 macOS 预览窗口、`ramag-bin` 组合检查、严格 Clippy、格式检查和差异检查通过；真实 HTTPS 点击刷新仍未验收；回滚只涉及设置页插件目录控件、组合根注入和对应文档 |
| 27 | `RAMAG-PLUGIN-CATALOG-RUNTIME-001` 目录刷新 runtime 崩溃修复 | 已完成：目录刷新在后台线程同步进入应用层专用 Tokio runtime，避免 GPUI 后台执行器直接驱动 `reqwest` 时触发无 reactor panic；失败仍只更新错误状态并保留旧目录 | 真实窗口点击刷新不退出进程；请求、解析和保存错误可回写设置页；runtime 初始化或请求失败显示明确错误；旧目录文件不被失败请求覆盖 | `ramag-app` fetcher/store 定向测试 8 项、真实 macOS 生产调试窗口、严格 Clippy、组合检查、格式检查和差异检查通过；修复前 `SIGABRT` 崩溃报告保留在本机诊断目录；回滚只涉及目录获取 runtime 包装、UI 调用点和本节记录 |
| 28 | `RAMAG-BUILD-MACOS-LINKER-001` macOS `__eh_frame` 链接器警告修复 | 已完成：为 `aarch64-apple-darwin` 和 `x86_64-apple-darwin` 配置 `ld64` 的 `-no_compact_unwind`，保留 DWARF 展开信息并避免大型 GPUI 调试二进制超过紧凑展开表上限 | 标准 macOS 构建和 DMG 调试构建不再输出 `__eh_frame section too large`；Windows/Linux 目标不使用该参数；不改变应用运行时逻辑 | `cargo build --locked -p ramag-bin`、`scripts/build-dmg.sh --debug`、格式检查和差异检查通过；构建中仍可能显示第三方 future-incompatibility 提示，该提示与本链接器警告无关；回滚只涉及 `.cargo/config.toml` 和本节记录 |
| 29 | `RAMAG-PLUGIN-CATALOG-HTTPS-INT-001` 目录源本地 HTTPS 集成验收 | 已完成：增加可复现的本地 Docker HTTPS fixture，使用自签名 `github.com` 证书和临时端口验证目录获取器的真实 TCP、TLS、HTTP 和本地保存链路 | `catalog.json` 成功获取并保存；404、302 和超过 1 MiB 的响应返回确定错误；每个失败场景都保留已保存目录；fixture 容器运行状态、端口映射、健康检查和清理均有脚本检查 | `./scripts/plugin-test/run-local-catalog-tests.sh` 通过，脚本使用 `nginx:1.27-alpine`、临时目录和命名后台容器，结束时清理容器与临时目录；测试从非 Tokio 测试线程调用 `fetch_and_store_on_managed_runtime`。不覆盖公网证书链、目录签名、插件包下载或安装；回滚只涉及集成测试和脚本 |
| 30 | `RAMAG-PLUGIN-DOWNLOAD-HTTPS-INT-001` 插件包本地 HTTPS 集成验收 | 已完成：增加独立 Docker HTTPS fixture 和忽略测试，使用真实 `tar.gz` 响应验证下载器的成功、错误和临时文件清理路径 | 成功下载后目标文件摘要与 fixture 一致；404、302、超过 `64 MiB` 和摘要错误都返回确定错误；失败目标不产生归档或 `.part-*` 临时文件；成功文件在失败场景后保持不变；fixture 容器状态、端口、健康检查和清理均有脚本检查 | `./scripts/plugin-test/run-local-download-tests.sh` 通过，使用 `nginx:1.27-alpine`、自签名 `github.com` 证书、临时目录和命名后台容器；测试通过 `download_to_managed_runtime` 走专用 runtime。该证据不覆盖公网证书链、真实设置页点击或下载后安装；回滚只涉及下载器集成测试和脚本 |
| 32 | `RAMAG-UI-PLUGIN-CACHE-REFRESH-001` 目录刷新后恢复下载缓存 | 已完成：目录刷新成功后在后台按新目录重新核验 `downloads/index.json`，恢复摘要匹配的“已下载”候选；摘要变化、缺失文件和未知候选不恢复 | 刷新不会让有效缓存的安装按钮消失；同一插件 ID/版本但摘要变化时不显示安装按钮；缓存核验失败不阻止新目录展示；恢复不在 UI 更新线程执行 | `ramag-ui` 插件诊断定向测试 11 项、严格 Clippy、`ramag-bin` 组合检查、格式检查和差异检查通过；回滚只涉及目录刷新缓存恢复和对应测试 |
| 34 | `RAMAG-PLUGIN-PROCESS-GROUP-001` Unix 插件进程组清理 | 已完成：macOS/Linux 插件入口启动时创建独立 process group；协议错误、调用超时、握手失败和关闭超时先终止整个进程组，再回收直接子进程 | 插件入口创建的 Unix 子进程不会因宿主会话终止而继续运行；进程组不存在时清理操作安全结束；Windows Job Object 和沙箱保持独立未实现边界 | `ramag-app` 进程宿主定向测试 6 项、严格 Clippy、`ramag-bin` 组合检查、格式检查和差异检查通过；回滚只涉及 Unix process group、`libc` 依赖和对应测试 |
| 35 | 新需求和可复现缺陷 | 用户明确提出新功能，或真实窗口、Docker 集成或本地测试产生可复现缺陷时，先在本计划增加独立切片 | 范围、通过条件、验证命令和回滚边界已写入本计划，完成后再更新状态 | 只修改受影响文件和测试，保留无关工作区内容 |
| 36 | `RAMAG-PLUGIN-PROCESS-JOB-001` Windows Job Object 进程树清理 | 已实现：Windows 入口进程启动后创建 Job Object，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 并立即接管入口；协议错误、调用超时、握手失败和关闭超时先终止 Job Object，再回收直接子进程；创建或分配失败返回明确错误，不静默退化 | Windows 目标库和全目标编译必须通过；Windows 真实子进程验收需证明入口创建的子孙进程在宿主会话终止后退出；外部 Job 限制、嵌套 Job 和入口启动到接管之间的竞态单独记录，不扩展为沙箱或资源配额 | macOS Unix 定向测试 6 项、`ramag-app` 严格 Clippy、Windows MSVC 目标库编译检查、格式检查和差异检查通过；本机没有 Windows 运行环境，真实 Job Object 子孙进程验收未完成；回滚只涉及 Windows 依赖、Job Object 生命周期、对应测试和本节记录 |

上述顺序是当前唯一主线队列。普通开发不等待或监测 GitHub CI。只有发布 release 或用户明确要求时，才记录对应仓库、运行编号、提交和 job 结果。每完成一个独立切片，在本文件追加结果并更新表格状态，不创建并行计划或专项计划。

### 4.9 2026-11-07 UI 更新后的真实窗口验收

使用 Windows MSVC Debug 构建 `target/debug/ramag.exe`，通过 Computer Use 操作真实窗口。
验收使用隔离目录 `.ramag/ui-acceptance/sqlite-query-20261106/data` 和
`.ramag/ui-acceptance/sqlite-query-20261106/system`，未使用用户默认配置。窗口关闭后未保留
Ramag 进程。

- 首页在约 `1200x773` 窗口显示四列工具卡片。标题、描述、管理入口和拖拽把手保持同一视觉层级；卡片宽高与源码中的 `GOLDEN_RATIO` 计算一致，阴影、边框和悬停反馈可见。原有响应式网格未改变。
- SQLite `query_records` 结果页的列过滤和行过滤控件使用统一外框。两者之间没有红框所示的垂直分隔条，单行过滤控件内没有内部垂直滚动条。查询、排序、事务、DDL、比较和导出入口保持同一工具栏基线。
- 结果表头 `id integer` 可用鼠标拖选并通过 `Ctrl+C` 复制；系统剪贴板读回完整字符串 `id integer`。DDL 只读查看器标题 `main.query_records` 也可拖选并复制，剪贴板读回完整字符串。该结果覆盖表头和 DDL 标题的真实窗口复制路径。
- SQLite 的 `Virtual views` 红色提示仍表示驱动未实现 `list_virtual_views`。这是源码定义的“不适用”状态，不是本轮布局或样式回归；本轮未修改该功能。

本轮没有源码变更。Windows Debug 真实窗口验收通过；Linux 编译和 Linux/macOS 测试按本计划的 WSL 与 GitHub CI 边界处理，本轮不重复执行。

### 4.10 2026-11-07 SQLite 结构迁移修复和真实窗口回读验收

本轮使用提交 `d47d8b22` 的 Windows MSVC Debug 构建
`target\\x86_64-pc-windows-msvc\\debug\\ramag.exe`，并使用隔离目录
`.ramag/ui-acceptance/sqlite-query-20261106/data` 和
`.ramag/ui-acceptance/sqlite-query-20261106/system`。源表为 SQLite
`query_records`，包含 `sort_value` 索引；验收没有读取或修改用户数据库。

- 修复前的 SQLite 迁移预览使用 `CREATE INDEX "idx" ON "main"."table"`，真实窗口执行返回
  `near ".": syntax error`。SQLite 要求索引名可以带数据库名，但 `ON` 后的表名不能带同一数据库限定。
- `d47d8b22` 将 SQLite 普通索引和唯一索引生成为
  `CREATE INDEX "main"."idx" ON "table" (...)`，并保留带点表名的独立标识符转义；新增回归测试覆盖该 SQL。
- 同一 SQLite 文件中的源表和目标表不能复用索引名。为避免把源表索引删除或改名，真实窗口通过独立的
  `sqlite-query-target-20261107` SQLite 文件创建目标表，再从源连接跨连接比较。比较结果显示目标缺少
  `sort_value`、`amount`、`payload` 和 `query_records_sort_idx`；迁移预览显示 4 条语句、0 条删除或修改语句。
- 人工确认后迁移成功。审批记录显示目标连接执行 4 条 SQLite 语句，随后重新打开结构比较，列和索引均无差异，
  页面显示“结构已一致”。目标表、临时连接和目标数据库文件均已清理，应用进程已关闭。
- Windows 本地验证通过：`cargo fmt --all -- --check`、
  `cargo test --locked -p ramag-tool-dbclient --lib schema_migration -- --test-threads=1`（21 项）、
  `cargo clippy --locked -p ramag-tool-dbclient --lib --all-targets -- -D warnings`、
  `cargo test --locked -p ramag-infra-sqlite --lib sqlite_driver_round_trips_queries_and_metadata -- --test-threads=1`、
  `cargo build --locked --target x86_64-pc-windows-msvc -p ramag-bin`。
- Linux 只按约定使用 WSL `Ubuntu-24.04` 编译：`cargo build --locked --workspace --all-targets`，
  stable Rust `1.99.1`、`x86_64-unknown-linux-gnu`，耗时约 `5m42s`，exit code 为 `0`；Linux 测试不在本机执行。

本项关闭 `A-UI-REAL-1` 的适用组合缺口。SQLite 的 `Virtual views` 仍按驱动未实现能力标记为不适用；
MongoDB/Redis 的 SQL 事务、DDL 和结构迁移仍按源码能力标记为不适用。后续主线只保留可见反馈性能证据、
Linux/macOS 原生平台验收和触发式发布评估。

### 4.11 2026-11-07 Release UI 更新后的同步录像验收

本轮使用 Windows MSVC Release 构建和真实窗口。环境为 Windows 11 Pro for Workstations、NVIDIA GeForce RTX 3060、
`2560×1440`、系统报告刷新率 `59Hz`；Ramag 窗口区域约 `1445×934`，采集输出为 `960×620`。应用没有读取或修改
系统剪贴板、数据库、设置值或用户凭据。

- 录像 `.ramag/ui-acceptance/visible-feedback-20261107f.mp4` 通过 `ffprobe` 校验，时长 `44.9s`，窗口区域采集有效。
  同步操作记录保存在 `.ramag/ui-acceptance/visible-feedback-20261107f.events.json`，包含系统监控的 CPU、内存、GPU、
  磁盘、网络、功耗、温度、进程和概览标签切换，以及首页和系统监控之间的往返切换，共 `33` 次操作；最后保持空闲
  `10s`，随后关闭窗口。Computer Use 状态确认 Ramag 进程退出，临时 ffmpeg 进程已清理。
- Release `ui-profiling` 原始日志为 `.ramag/ui-acceptance/visible-feedback-20261107e.ramag.log`。日志记录
  `242` 个输入事件、`344` 个 draw、`342` 个同窗口 Presented 边界，`input_p95_us=347`、`draw_p95_us=10200`、
  `present_p95_us=1060`，`journal_lost=0`、`duration_samples_dropped=0`、`overflow=false`，没有跨窗口或无效呈现关系。
- 录像输出的 `avg_frame_rate=2890/449`，约 `6.44fps`，单帧间隔约 `155ms`。窗口画面差异原始数据保存在
  `.ramag/ui-acceptance/visible-feedback-20261107f-diffs.json`，按输入时间计算的 33 次首次变化估计和统计保存在
  `.ramag/ui-acceptance/visible-feedback-20261107f-latency.json`；估计中位数约 `296ms`、p95 约 `453ms`、最大约
  `484ms`。该估计受低帧率量化影响，不能作为精确的点击到可见反馈 p95，也不能替代显示合成器时间；它只能说明
  当前采集方案不能证明 `100ms` 条件已满足。
- 本轮没有发现可单独归因于动画时长的稳定瓶颈，没有修改动画或渲染代码。`RAMAG-UI-001AC-2` 继续保持进行中，
  下一步需要单独提高窗口录像帧率或接入显示合成器追踪，再判断是否需要让动画和页面更新更流畅。

补充尝试使用 NVIDIA NVENC 和 Windows Desktop Duplication 提高录像帧率。NVENC 录像
`.ramag/ui-acceptance/visible-feedback-20261107g.mp4` 通过 `ffprobe` 校验，时长约 `44.97s`、`575` 帧、
输出 `1280×828`、`avg_frame_rate=17250/1349`（约 `12.79fps`）。应用日志记录 `202` 个输入事件、`202` 个 draw、
`201` 个同窗口 Presented 边界，`input/draw/present p95` 为 `0.305/10.342/1.124ms`，无日志丢失或溢出。
由于采集启动晚于应用，录像与前 `22` 次操作重叠；这次硬件编码诊断的首次画面变化估计 p95 约 `247ms`，
仍不能满足或证明 `100ms` 条件。`visible-feedback-20261107g.events.json`、
`visible-feedback-20261107g-diffs.json` 和 `visible-feedback-20261107g-latency.json` 保存原始记录。

同一窗口区域尝试 `ddagrab`（Desktop Duplication API）时，ffmpeg 停在输入初始化且没有生成帧；
已确认进程路径后结束该临时采集进程，没有把这次失败当作应用故障。当前 Windows 会话没有可用的显示合成器
时间证据。随后尝试启动 Windows Performance Recorder 的 `DesktopComposition.light` 和 `GPU.light` 配置，
因系统策略错误 `0xc5585011` 拒绝启用性能追踪；`wpr -status` 确认没有残留录制。`RAMAG-UI-001AC-2`
继续保持进行中。

随后把 GDI/NVENC 窗口区域缩小为 `960x700` 再试，输入仍只有约 `13fps`；ffmpeg 在停止时未完成 MP4
索引，因此不保留为有效录像证据。该实验只说明当前会话的桌面采集瓶颈不是窗口区域大小，`RAMAG-UI-001AC-2`
仍需显示合成器追踪或可用的高帧率同步录像。

### 4.12 2026-11-07 Release UI 真实窗口回归验收

在同一台 Windows 开发机上重新启动 `target/x86_64-pc-windows-msvc/release/ramag.exe`，使用 Computer Use
获取真实窗口状态和截图。窗口可启动并显示首页工具卡片。首页卡片可进入系统监控，系统监控的概览、CPU、内存、
GPU、磁盘、网络、功耗、温度和进程共 `9` 个标签页均可切换。每个页面都显示对应的数据卡片或表格，数值和历史
图表在观察期间持续刷新；温度页对不可用的 CPU 传感器显示“不可用”，同时保留可用的 GPU 温度数据。返回首页后，
工具卡片网格和左侧导航保持可见，窗口可以正常关闭，Ramag 进程退出。

本次验收没有发现页面空白、标签页卡死、窗口布局跳变或退出失败。该结果证明当前发布构建的主要 UI 流程可用，
不等同于点击到可见反馈 `100ms` 的性能结论；`RAMAG-UI-001AC-2` 仍需显示合成器时间或更高帧率同步录像，
且不通过减少动画时长替代流畅度验收。

### 4.13 2026-11-07 `v0.9.2` 发布资产验证

Desktop Release run `42289929133` 已完成且结论为 `success`。Linux x86_64、macOS 和 Windows 打包任务，
以及 `Publish GitHub Release` 任务均成功。Release [v0.9.2](https://github.com/bruceblink/ramag-platform/releases/tag/v0.9.2)
为正式版本，发布时间为 `2026-11-06T21:30:44Z`；附注标签 `v0.9.2` 解析到提交 `37ae32b5`。

- 公开资产共 `11` 个：5 个安装包、4 个从 `v0.9.1` 生成的差分包、`SHA256SUMS.txt` 和 `update-manifest.json`。
- 下载全部 9 个二进制及差分资产后，按 `SHA256SUMS.txt` 逐项重算，`9/9` 个摘要匹配；没有缺失或多余的校验记录。
- `update-manifest.json` 的版本为 `0.9.2`、标签为 `v0.9.2`，包含 5 个安装包；清单中的名称和大小与 GitHub Release 资产一致。

本项关闭 `RAMAG-RELEASE-001` 的 `v0.9.2` 发布验证。后续发布仍需使用新的版本提交和标签；
不会移动或覆盖已公开标签。Linux/macOS 原生窗口、系统凭据和独立测试账户验收仍属于
`RAMAG-PLATFORM-001`，不由发布资产核对替代。

### 4.14 2026-11-07 跨平台系统凭据与原生窗口验证入口

为补充 `RAMAG-PLATFORM-001` 的系统凭据证据，新增被 `#[ignore]` 标记的
`crates/ramag-infra-storage/tests/keyring_round_trip.rs`。普通 `cargo test` 不运行该测试，
避免改动开发机用户的 Keychain 或 Secret Service；测试创建主密钥、读取并比较同一主密钥，
再删除条目并核对缺少主密钥时的停止错误。Linux Secret Service 在条目不存在时可能返回后端的“no result found”，
因此测试不再把空条目的前置删除作为前提，避免把服务的空状态误判为失败。

- Linux Desktop CI 安装 `dbus-x11` 和 `gnome-keyring`，在临时 `dbus-run-session` 中启动 Secret Service，
  使用临时随机口令运行 `gnome-keyring-daemon --components=secrets --daemonize --unlock` 解锁登录 keyring，
  再使用 `--ignored` 执行测试。随机口令只在该 shell 中使用，不写入日志或仓库。
- macOS Desktop CI 使用运行器的 Keychain，使用 `--ignored` 执行同一测试。
- `ramag-bin/examples/ui-preview.rs` 增加 `--smoke` 模式：创建真实窗口，保持 `2s` 后退出；Linux CI
  使用 Xvfb，macOS CI 使用运行器窗口服务，分别记录启动和退出结果。该模式不读取用户配置或系统凭据。
- Windows 本机适用验证已通过 `ramag-infra-storage` 的 `78` 项单元测试和严格 Clippy；本轮没有执行 Linux 测试，
  也没有修改正常启动或凭据存储实现。

该入口已经进入代码基线，但当前没有把普通开发的 GitHub CI 运行作为执行步骤。发布 release 或用户明确要求平台核对时，
再读取一条完整的 Desktop CI 运行并记录仓库、提交、运行编号、job 和失败阶段。Linux/macOS 系统凭据、原生窗口 Smoke、
退出重启回读和独立测试账户仍需按平台取得对应证据，CI 凭据测试和 Smoke 测试不能替代真实窗口步骤。

### 4.15 更新下载反馈打磨

更新页原先只显示下载字节数，没有给出百分比和视觉进度。已校验下载后的操作文案“安装安装包”也不够明确。
本切片只调整更新页的反馈层，不改变更新服务的下载、摘要校验、取消和安装调用。

- 当驱动提供总大小时，页面显示下载百分比和有界进度条。下载量超过总大小时按 `100%` 显示，避免进度条溢出。
- 总大小为零时保留“正在准备下载”，不显示虚假的百分比。
- 下载完成后按钮显示“安装更新”，保留已校验提示和打开目录入口。
- `360×640`、`1024×768` 和 `1440×900` 的更新页布局均保持在内容区域内。

验证命令为 `cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui --lib settings_view::update::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-ui --lib --all-targets -- -D warnings` 和 `git diff --check`。
回滚范围只包括 `crates/ramag-ui/src/settings_view/update.rs`、对应测试和本节记录。

2026-11-09 已完成本切片。下载总大小已知时显示有界百分比和进度条；总大小未知时继续显示准备状态；完成后按钮显示“安装更新”。
`cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui --lib settings_view::update::tests -- --test-threads=1`
和 `cargo clippy --locked -p ramag-ui --lib --all-targets -- -D warnings` 均通过，共 5 项更新页测试通过。

### 4.16 设置导航可访问性

设置页导航入口原先使用可点击容器，缺少稳定的按钮语义和键盘焦点反馈。该切片只补充通用交互语义，
不改变导航页面、布局尺寸或鼠标点击回调。

- 每个入口声明 `Button` 角色和对应页面标题，辅助技术可以区分“系统”“数据库”等入口。
- 入口加入键盘焦点顺序和焦点可见样式，桌面纵向导航和窄窗口横向导航都使用同一入口结构。
- 现有悬浮样式、选中背景、更新提示和 `settings-page-*` 调试标识保持不变。

验证命令为 `cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui --lib settings_view::pages::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-ui --lib --all-targets -- -D warnings` 和 `git diff --check`。
回滚范围只包括 `crates/ramag-ui/src/settings_view/pages.rs`、对应测试和本节记录。

2026-11-09 已完成本切片。设置导航入口现在声明 Button 角色、稳定的辅助技术标识和页面标题，支持 Tab 焦点顺序以及 Enter/Space 激活，
焦点状态使用现有主题色的半透明背景反馈。`cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui --lib settings_view::pages::tests -- --test-threads=1`
和 `cargo clippy --locked -p ramag-ui --lib --all-targets -- -D warnings` 均通过，共 4 项设置导航测试通过。

### 4.17 工具可见性开关可访问性

工具可见性设置页的 Home 和侧边栏开关原先没有包含工具名称的辅助技术标签，开关组和实际控件还复用了同一个调试标识。
本切片补充作用面和工具名称，并分离组标识与控件标识。

- Home 开关显示为“`<工具名> 的首页入口`”，侧边栏开关显示为“`<工具名> 的侧边栏入口`”。
- 组容器使用 `settings-tool-*-*-group` 标识，Switch 保留自身的 `settings-tool-home-*` 或 `settings-tool-sidebar-*` 标识。
- 现有开关状态、持久化和 Home/侧边栏独立切换行为保持不变。

验证命令为 `cargo fmt --all -- --check`、`cargo test --locked -p ramag-ui --lib tool_visibility::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-ui --lib --all-targets -- -D warnings` 和 `git diff --check`。
回滚范围只包括 `crates/ramag-ui/src/tool_visibility.rs`、对应测试和本节记录。

2026-11-12 已完成本切片。工具可见性测试 2 项通过，覆盖 Home/侧边栏独立切换及 `360×640`、`1024×768`、`1440×900` 布局边界。
严格 Clippy、格式检查和差异检查均通过。

### 4.18 插件存储失败隔离验收

插件宿主初始化前会读取普通设置和敏感设置。此前已有存储层单元测试，但没有验证宿主在读取或解封失败时是否阻止插件回调并移除工具入口。
本切片增加宿主级测试存储，覆盖普通设置读取失败和秘密解封失败。

- 两类失败都生成 `PluginLifecycleStage::Initialize` 诊断。
- 插件的 `initialize` 回调不执行，插件状态变为 `Failed`，注册表不再保留失败工具入口。
- 失败消息保留存储错误原因，后续插件继续按注册顺序处理。

验证命令为 `cargo fmt --all`、`cargo test --locked -p ramag-app --lib plugin_lifecycle::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --lib --all-targets -- -D warnings` 和 `git diff --check`。
回滚范围只包括 `crates/ramag-app/src/plugin_lifecycle_tests.rs` 与本节记录。

2026-11-12 已完成本切片。插件生命周期定向测试 15 项通过，包含设置快照读取失败和秘密解封失败的宿主级隔离验收。

### 4.19 插件开发手册基线同步

插件开发手册头部仍写入 `0.8.0`，而当前 workspace 和主线开发计划使用 `0.9.2`。这会让阅读者误判接口文档对应的代码版本。
本切片只更新手册适用基线和维护日期，保留静态插件范围、排除项和接口正文。

验证内容为 `Cargo.toml` 版本与手册版本一致、Markdown 结构检查、UTF-8/LF 检查和 `git diff --check`。
Rust 编译、UI 操作和 Docker 集成测试不适用。回滚范围只包括 `docs/05-plugin-development-guide.md` 的头部和本节记录。

2026-11-12 已完成本切片。手册适用基线已同步为 `0.9.2`。

### 4.20 可安装插件清单与本地状态

用户明确要求插件系统向 VS Code 和 Zed 的可安装模式演进。当前切片先建立包元数据和本地状态边界，后续再接入进程外运行时。

- `PluginPackageManifest` 保存插件 ID、SemVer 版本、宿主 API、`process` 运行时、相对入口、声明能力和 SHA-256 摘要。
- 清单校验拒绝不支持的格式/API、路径穿越、绝对路径、未知或重复能力、无效版本和非小写 64 位摘要。
- `PluginInstallationStore` 使用 `installed.json` 保存 `installed`、`enabled`、`disabled` 和 `quarantined` 状态，并限制记录数量、错误长度和文件大小。
- 状态文件使用同目录临时文件、完整写入、同步和替换；读取时重新校验所有清单。存储操作使用进程内互斥，避免并发状态更新丢失。
- 组合根在启动时读取状态并注入插件设置页。页面显示名称、ID、版本、API、运行时、状态和最近错误；设置页支持启用和禁用操作，操作通过 `PluginInstallationStore` 原子保存并立即回显。启用状态只表示本地状态，不启动外部进程。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-domain --lib traits::plugin::tests -- --test-threads=1`、
`cargo test --locked -p ramag-app --lib plugin_installation::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-domain --lib --all-targets -- -D warnings`、
`cargo clippy --locked -p ramag-app --lib --all-targets -- -D warnings`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics -- --test-threads=1`、
`cargo check --locked -p ramag-bin --all-targets`、
`cargo clippy --locked -p ramag-ui --lib --all-targets -- -D warnings`、
`cargo clippy --locked -p ramag-bin --bin ramag --all-targets -- -D warnings` 和 `git diff --check`。
回滚范围只包括插件包清单、安装状态存储、依赖锁文件、组合根、设置页和本节记录。

2026-11-13 已完成本切片。清单测试 12 项、安装状态测试 4 项、插件设置页测试 3 项通过，新增启用→禁用和状态文件回读验收；workspace 组合编译和严格 Clippy 通过。进程外 IPC、签名、沙箱、原子包安装、更新回滚、远程目录和真实插件窗口仍未验收。

### 4.21 进程外插件协议基础

本切片为下一阶段的独立插件进程建立纯协议边界。协议层只负责帧编解码和消息校验，不创建子进程、不执行入口、不读取凭据，也不负责沙箱。

- 使用 4 字节小端长度前缀；长度先校验，再按 `8 MiB` 上限分配帧内容。
- JSON 信封支持 `hello`、`ready`、`invoke`、`response` 和 `shutdown` 五类消息。
- 握手校验协议版本、插件 ID、宿主 API、能力集合和宿主帧上限；入口调用校验请求 ID、入口 ID 和 `4 MiB` 输入上限。
- 二进制输入和输出使用 base64；响应输出最多 `1 MiB`，失败摘要最多 `512` 字节，并要求成功标志与错误字段一致。
- 异步读写助手将截断、I/O 失败和协议错误转换为确定错误，后续进程宿主再增加超时、崩溃隔离和状态隔离。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_process_protocol::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --lib --all-targets -- -D warnings` 和 `git diff --check`。
回滚范围只包括 `crates/ramag-app/src/plugin_process_protocol.rs`、对应测试、依赖清单/锁文件和本节记录。

2026-11-13 已完成本切片。协议测试 8 项通过。真实子进程握手、超时、崩溃恢复、签名、沙箱、安装回滚和 Windows 进程树清理仍未验收。

### 4.22 进程外插件宿主最小闭环

本切片把协议层接入真实子进程。宿主只启动清单指定的包内入口，先完成握手，再允许入口调用；失败会结束当前会话，调用方可以根据错误更新安装状态。

- 包根必须是绝对普通目录，入口必须是包内普通文件；绝对路径、父目录、符号链接和包外路径都会被拒绝。
- 进程使用直接 `Command` 启动，清空继承环境，只保留 `PATH`、Windows `SystemRoot` 和协议所需的三个 `RAMAG_PLUGIN_*` 环境变量。
- 启动阶段使用握手超时；调用阶段串行读取响应并使用 `60s` 默认超时；超时、协议错误和请求 ID 不匹配会终止会话。
- 关闭使用 `1s` 默认等待，stderr 只保留 `16 KiB` 尾部，供诊断页面读取；正常禁用和失败隔离仍由上层安装状态存储负责。
- `ramag-plugin-helper` 仅用于跨平台真实进程验收，不是 Ramag 主程序入口。启动前会按安装器相同规则重新核对包摘要；签名验证、沙箱和进程组/Job Object 子孙进程清理仍未实现。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_process::tests -- --test-threads=1`、
`cargo test --locked -p ramag-app --test plugin_process -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings` 和 `git diff --check`。
回滚范围只包括 `crates/ramag-app/src/plugin_process.rs`、对应测试、协议辅助进程、Tokio 特性和本节记录。

2026-11-13 已完成本切片。进程宿主单元测试 4 项和真实辅助进程测试 1 项通过。第三方插件包、Windows 进程树清理和真实窗口验收仍未完成。

### 4.23 插件包目录安装

本切片把清单和进程宿主之间的包目录边界固定下来。输入是已经解包的本地目录，安装器不下载、不执行入口，也不把凭据传入插件。

- 清单文件固定为 `plugin.json`，大小限制为 `128 KiB`；清单先执行 API、版本、能力和入口校验。
- 包摘要使用稳定的相对文件路径和文件内容计算 SHA-256，清单文件不参与自身摘要；总文件内容限制为 `64 MiB`。
- 源目录、包目录和每个包内路径拒绝符号链接；源目录不能位于包目录内，入口必须是包内普通文件。
- 包版本目录先复制到临时目录，再备份旧版本并替换；状态记录写入失败时恢复旧版本目录。
- 当前输入是本地解包目录；受限本地 `tar.gz` 归档由 `RAMAG-PLUGIN-ARCHIVE-001` 负责。远程下载、签名验证、升级回滚记录和远程市场另行实现；卸载清理由 `RAMAG-PLUGIN-UNINSTALL-001` 负责。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_package_installer::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings` 和 `git diff --check`。
回滚范围只包括 `crates/ramag-app/src/plugin_package_installer.rs`、对应测试和本节记录。

2026-11-13 已完成本切片。包安装测试 4 项通过，覆盖有效安装、摘要不匹配、源目录边界和符号链接拒绝。

### 4.24 本地插件卸载与目录清理

本切片补齐本地安装生命周期的卸载动作。卸载只处理已经存在的安装状态和对应版本目录，不启动插件进程，不删除其他插件或其他版本。

- 卸载前重新读取并校验安装状态，根据插件 ID 和清单版本定位包目录；插件 ID 目录、版本目录和规范化路径必须位于包根目录内，符号链接和目录外路径均拒绝。
- 先把版本目录改名为同级临时备份，再调用 `PluginInstallationStore` 删除状态；状态删除失败时恢复原目录。
- 备份目录清理失败时尝试恢复目录和安装状态；成功后只删除空的插件 ID 目录，不影响其他版本。
- 安装与卸载共享进程内操作互斥，避免同一安装器实例并发替换或删除目录。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_package_installer::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings` 和 `git diff --check`。
测试覆盖有效卸载、未知插件、缺失包目录、符号链接拒绝、保留其他版本，以及设置页确认后状态和目录回读。

2026-11-13 在 macOS 隔离调试应用中完成真实窗口验收。使用隔离数据目录和插件状态目录预置 `demo.package` `1.0.0`；设置页显示插件名称、版本、API、运行时和“禁用/卸载”按钮。点击“禁用”后按钮变为“启用”，`installed.json` 回读为 `disabled`；再次启用后回读为 `enabled`。点击“卸载”先显示“取消/确认卸载”，确认后插件行消失，页面显示“暂无可安装插件”，状态文件变为空数组，版本目录已删除。测试插件入口未启动。远程目录、签名验证、沙箱和升级回滚记录仍未验收。

### 4.25 启动前包摘要核验

本切片修复安装后包文件被替换仍可能启动的问题。进程宿主在解析入口并创建子进程前，复用 `PluginPackageInstaller` 的稳定路径和内容摘要规则重新计算包摘要；摘要与 `PluginPackageManifest.package_sha256` 不一致时直接返回错误，不启动入口。

- 摘要计算继续排除 `plugin.json` 自身，保留相同的相对路径顺序、文件内容和 `64 MiB` 内容上限。
- 摘要计算失败或摘要不一致都发生在 `Command::spawn` 之前；不会创建插件进程，也不会进入握手。
- 一致摘要的真实辅助进程测试继续覆盖握手、回显调用和关闭；篡改后摘要测试确认入口不会被启动。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_process::tests -- --test-threads=1`、
`cargo test --locked -p ramag-app --test plugin_process -- --test-threads=1`、
`cargo test --locked -p ramag-app --lib plugin_package_installer::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings` 和 `git diff --check`。
2026-11-13 已完成本切片。进程宿主测试 5 项、真实辅助进程测试 1 项和包安装器测试 9 项通过。签名验证、沙箱、进程组/Job Object 和资源配额仍未实现。

### 4.26 插件设置页错误反馈

插件设置页原先把状态保存失败、卸载失败和安装记录中的最近错误显示为普通红色文本。错误原因缺少统一的视觉层级和稳定的测试选择器，窄窗口下也无法用相同结构检查错误反馈边界。

本切片复用设置页错误状态卡。状态卡包含错误图标、错误标题、边框和浅色背景；插件操作错误使用 `plugin-installations-error` 选择器，插件最近错误使用包含插件 ID 的选择器。错误反馈只改变显示，不改变状态存储、卸载和恢复逻辑。

验证覆盖明暗主题和 `360×520`、`1024×520`、`1440×520` 窗口。错误状态卡和最近错误卡均保持在“已安装插件”区域内，错误原因保持可读。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-ui --lib --all-targets -- -D warnings` 和 `git diff --check`。
2026-11-13 已完成本切片。插件诊断定向测试 5 项通过。真实窗口未重复操作错误反馈卡，当前证据为 headless GPUI 布局测试。

### 4.27 受限本地 `tar.gz` 包安装

此前安装器只接受已经解包的本地目录，用户不能直接把一个插件归档交给安装器。该切片增加 `PluginPackageInstaller::install_from_archive`，只处理本地 `tar.gz` 文件，不下载、不执行归档中的入口。

- 先读取归档快照并限制压缩后文件大小为 `64 MiB`，源路径必须是绝对路径、普通文件且不能是符号链接。
- 先扫描全部条目，再进行第二遍解包。条目路径必须是相对普通路径，拒绝父目录、根路径、重复路径、符号链接、硬链接、FIFO、设备文件和其他特殊类型。
- 条目数最多 `1024` 项，解包内容最多 `64 MiB`。扫描结果与第二遍解包结果必须一致，解包目录使用包目录同级的临时目录。
- 解包完成后复用目录安装器读取 `plugin.json`、核对包摘要、原子替换版本目录和写入 `installed.json`。临时目录由清理守卫回收；清理失败只记录诊断，不把已完成的安装伪报为失败。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_package_archive::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。有效归档安装、路径穿越、符号链接/硬链接/FIFO、重复路径、条目数量、解包大小、相对归档路径和归档符号链接测试通过。远程下载、签名验证和沙箱仍未完成。

### 4.28 设置页本地归档安装入口

本切片把 `PluginPackageInstaller::install_from_archive` 接入插件设置页。用户点击“安装本地归档”后选择 `.tar.gz` 或 `.tgz` 文件，文件选择和安装任务在后台执行，设置页保持可响应。

- 安装开始后按钮显示“安装中…”并禁用，避免同一页面重复提交归档。
- 用户取消文件选择时不修改现有安装状态；归档安装成功后按插件 ID 替换已有行或追加新行，并复用安装器已经写入的 `installed.json`。
- 归档校验或状态保存失败时保留当前插件列表，在页面显示统一的“插件操作失败”状态卡和错误原因。
- 安装按钮使用稳定选择器 `plugin-installation-install-archive`，便于布局和辅助测试验证。

验证覆盖明暗主题和 `360×520`、`1024×520`、`1440×520` 窗口，按钮保持在“已安装插件”区域内。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-ui --lib --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。插件诊断定向测试 5 项通过；真实系统文件选择对话框未自动化操作，当前 UI 证据为 headless GPUI 布局测试。

### 4.29 本地插件目录清单解析

本切片为后续插件市场和远程分发保留一个安全的元数据边界。`PluginPackageCatalog::parse_json` 只读取本地 JSON 字节并返回已校验的候选包，不执行网络请求、下载、解包、安装或权限授予。

- 清单格式版本固定为 `1`，JSON 最大 `1 MiB`，最多 `256` 个包条目。
- 每个条目校验插件 ID、显示名称、SemVer 版本、宿主 API 兼容性和 `process` 运行时。
- 下载地址必须是无用户信息、无片段的 HTTPS 地址；地址长度最多 `2048` 字节。
- 归档 `archive_sha256` 必须是 `64` 个小写十六进制字符；能力必须来自已知集合且不能重复。包内 `PluginPackageManifest.package_sha256` 继续由安装器按解包内容核验。
- 同一插件 ID 和版本只能出现一次。结构体使用 `deny_unknown_fields` 拒绝未知字段，目录清单不能借此传入执行参数。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_package_catalog::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。有效清单、重复条目、格式版本、HTTPS 地址、SemVer、归档摘要、API、能力和资源上限测试通过。远程目录获取和签名仍未完成；本地候选展示由 `RAMAG-UI-PLUGIN-CATALOG-001` 负责。

### 4.30 设置页本地目录候选展示

本切片把插件状态目录中的 `catalog.json` 接入应用组合根和插件设置页。启动时读取并校验清单；损坏、超量或不兼容的清单只记录警告并显示空目录，不阻止内置工具和安装状态加载。

- 设置页新增“可安装插件目录”区域。每个候选显示名称、插件 ID、版本、API、进程运行时、能力和下载地址，并使用 `plugin-package-catalog` 与 `plugin-package-catalog-<id>-<version>` 选择器。
- 空目录显示下一步说明，明确当前不会联网、下载或执行代码；候选展示不改变 `installed.json`，也不授予权限。
- 目录候选由启动组合根注入，设置页只负责渲染。远程请求、归档下载、签名验证和安装动作保持独立边界。

验证覆盖明暗主题和 `360×520`、`640×520`、`1024×520`、`1440×520` 窗口。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-ui --lib --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。插件诊断定向测试 5 项通过；真实窗口读取 `catalog.json` 和目录候选展示未自动化操作，当前 UI 证据为 headless GPUI 布局测试。

### 4.31 有界 HTTPS 插件包下载

本切片增加 `PluginPackageDownloader`，把已经通过 `PluginPackageCatalogEntry::validate` 的候选包下载到新的本地缓存文件。目录字段 `archive_sha256` 表示整个 `.tar.gz` 归档的摘要；包内 `PluginPackageManifest.package_sha256` 继续表示解包文件内容摘要。下载器只负责接收、限制、归档摘要核验和原子落盘，不调用安装器，不执行包内容，也不改变安装状态。

- 生产构造器 `PluginPackageDownloader::new` 使用 HTTPS 目录条目、`HTTP 200` 和零重定向策略；`with_client` 仅用于注入客户端，调用方必须自行负责其重定向和 TLS 配置。
- `Content-Length` 和流式累计大小都受 `MAX_PLUGIN_DOWNLOAD_BYTES = 64 MiB` 限制。目标路径必须是绝对路径，最终目标父目录不能是符号链接或非目录，目标文件必须不存在。
- 下载先写同目录临时文件，完成 `flush`、`sync_all` 并核对小写 SHA-256 后再通过不覆盖的原子发布完成落盘。摘要不匹配、状态错误、超量或写入失败会清理本次已创建的临时文件。
- workspace 的 `reqwest` 使用 `rustls-no-provider`。应用启动层必须在创建下载器前安装 TLS provider；下载模块不修改全局 TLS 状态。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_package_downloader::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。下载器默认测试 3 项、Docker HTTPS 集成测试 1 项、严格 Clippy、组合检查、格式检查和差异检查通过；真实 `tar.gz` 成功落盘、SHA-256、HTTP 状态、超量、摘要错误和临时文件清理已由 4.41 补充验收。真实设置页点击、下载后安装、签名验证和沙箱仍需独立切片。

### 4.32 设置页目录候选下载入口

本切片把 `PluginPackageDownloader` 接入插件设置页的本地目录候选。用户可以从候选行启动下载，应用把已核验摘要的归档写入插件状态目录下的独立 `downloads` 子目录；下载完成后显示安装动作，安装仍由独立的目录清单核对流程负责。

- 每个候选使用 `plugin-package-catalog-download-<id>-<version>` 稳定选择器，并显示“待下载”“下载中”或“已下载”。同一设置页同时只运行一个下载任务，下载中按钮禁用。
- 下载任务在 GPUI 后台执行器中提交到下载器专用 Tokio runtime，避免直接在 UI 执行器中驱动 Tokio 文件和 HTTP future。目标路径由应用生成，设置页不接受用户任意写入路径。
- 下载成功后保留已校验缓存路径和“已下载”状态，并显示“安装”按钮。下载失败保留候选行，复用统一错误状态卡显示原因；下载本身不改变目录候选和 `installed.json`。
- 下载和安装、卸载互斥，避免并行写入包目录。安装按钮由 `RAMAG-PLUGIN-CATALOG-INSTALL-001` 负责清单身份核对、状态刷新和缓存清理。

验证覆盖明暗主题和 `360px`、`1024px`、`1440px` 窗口。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics::tests -- --test-threads=1`、
`cargo test --locked -p ramag-app --lib -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo clippy --locked -p ramag-ui --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。插件诊断定向测试 8 项、应用层全量测试 329 项、严格 Clippy、组合检查、格式检查和差异检查通过。真实窗口点击和真实 HTTPS 服务仍未验收；下载后安装由 4.33 切片覆盖。

### 4.33 下载缓存安全安装

本切片把设置页中已完成归档摘要核验的缓存包接入安装器。安装动作重新读取缓存文件，核对目录记录的 `archive_sha256`，再在临时解包目录中核对包内 `plugin.json` 的身份字段，最后复用现有目录摘要、入口解析、原子替换和状态保存流程。

- 目录记录和包内清单必须逐项匹配 `id`、名称、版本、宿主 API、运行时和能力集合。能力按集合比较，声明顺序变化不改变结果。目录记录没有入口字段，入口路径继续由包内清单和现有入口解析器校验。
- 归档摘要在安装按钮触发时重新计算，不能只信 UI 内存中的下载路径或下载完成状态。摘要不匹配或清单身份不匹配时，在写入 `packages` 和 `installed.json` 前返回错误。
- 安装成功后设置页按插件 ID 更新安装状态，候选显示“已安装”，并删除本次缓存归档。缓存清理失败不回滚已完成安装，只记录带路径的诊断；安装失败保留缓存，允许用户重试。
- 下载缓存文件写入插件状态目录，但“已下载”路径映射在本切片仍只在本次应用进程内恢复；重启后的缓存扫描和状态恢复由 4.34 切片完成。
- 目录字段使用 `archive_sha256`。解析器继续接受历史 `package_sha256` 字段作为兼容别名；包内 `PluginPackageManifest.package_sha256` 保持解包内容摘要语义，两个摘要不能互换。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_package_archive::tests -- --test-threads=1`、
`cargo test --locked -p ramag-app --lib plugin_package_catalog::tests -- --test-threads=1`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics::tests -- --test-threads=1`、
`cargo test --locked -p ramag-app --lib -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo clippy --locked -p ramag-ui --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。归档安全安装定向测试 6 项、目录清单测试 6 项、插件诊断测试 8 项、应用层全量测试 329 项、严格 Clippy、组合检查、格式检查和差异检查通过。真实 HTTPS、真实窗口点击和跨进程安装仍未验收。

### 4.34 下载缓存跨重启恢复

本切片补齐下载归档的进程外缓存状态。此前归档文件会写入插件状态目录，但设置页只在当前进程内存中保存“已下载”路径；应用重启或重新创建设置页后，用户无法继续安装已经完成摘要核验的缓存。

- `PluginPackageCache` 在 `downloads/index.json` 中保存插件 ID、版本、归档摘要和单层文件名。索引写入使用同目录临时文件、完整写入、同步和原子替换。
- 启动恢复只接受绝对且非符号链接的缓存根目录、受限索引、当前本地目录候选和普通归档文件。每个候选重新检查文件大小和归档 SHA-256；单次恢复重新读取的归档总量最多 `256 MiB`。摘要不匹配、路径越界、符号链接、目录、未知候选和重复记录都不注入设置页。
- 缓存恢复失败只写入带路径的警告，不阻止内置工具和安装状态加载。恢复结果只改变设置页的“安装”动作，不写入 `packages` 或 `installed.json`。
- 安装成功后同步移除索引记录和归档。旧版本使用 PID/序列的随机归档文件名，没有索引记录时不迁移；这些文件不会被当作可安装缓存。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_package_cache::tests -- --test-threads=1`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics::tests -- --test-threads=1`、
`cargo check --locked -p ramag-bin --all-targets`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo clippy --locked -p ramag-ui --all-targets -- -D warnings` 和 `git diff --check`。
2026-11-13 已完成本切片。缓存定向测试 8 项、插件诊断测试 8 项、严格 Clippy、组合检查、格式检查和差异检查通过。真实窗口恢复和跨进程安装仍未验收。

### 4.35 插件目录候选窄窗口与长文本布局

本切片收紧插件目录候选的响应式布局。此前候选行的标题、状态、动作按钮和下载地址共用一行，长插件名称或长 URL 可能把动作区域推到目录边界之外。

- 候选行头部和动作区域允许换行，标题栈继续使用可收缩宽度。
- 能力文本使用可换行的全宽区域。下载地址使用 `plugin-package-catalog-url-<id>-<version>` 稳定选择器、单行省略和隐藏溢出。
- 状态圆点和状态文字按待下载、下载中、安装中、已下载和已安装区分弱提示、警告和成功颜色；状态逻辑和动作互斥条件不变。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-ui --all-targets -- -D warnings` 和 `git diff --check`。
2026-11-13 已完成本切片。插件诊断测试 9 项、严格 Clippy、格式检查和差异检查通过。真实窗口点击仍未验收。

### 4.36 远程插件目录获取与本地保存

本切片增加远程插件目录的显式获取边界。应用启动不联网，现有组合根继续从插件状态目录读取本地 `catalog.json`；后续设置页手动刷新可以复用本切片的 API。

- `PluginPackageCatalogFetcher` 使用 HTTPS、10 秒连接超时、20 秒总请求超时、禁止重定向和固定 User-Agent。源地址拒绝用户信息、查询参数和片段。
- 响应必须是 HTTP 200。`Content-Length` 和流式累计读取均受 `MAX_PLUGIN_CATALOG_MANIFEST_BYTES = 1 MiB` 限制；响应正文不进入错误信息。
- 获取完成后复用 `PluginPackageCatalog::parse_json` 校验格式、条目和能力。解析失败、网络失败、HTTP 错误、重定向或超量都不会写入本地清单。
- `PluginPackageCatalogStore` 将已解析清单写入同目录临时文件，执行完整写入、`flush`、`sync_all` 后原子替换 `catalog.json`。根目录必须是绝对的普通目录；保存失败保留旧文件。
- 远程清单只提供候选元数据，不表示签名、来源可信或安装许可；签名、来源 URL 设置、手动刷新 UI、自动安装和沙箱继续由独立切片负责。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_package_catalog_fetcher::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。fetcher/store 定向测试 6 项、应用层严格 Clippy、格式检查和差异检查通过。真实 HTTPS 服务、更新页目录源设置和手动刷新仍未验收；本切片未改变启动联网行为。

### 4.37 插件目录源设置与手动刷新

本切片把远程目录获取接入插件设置页。应用启动仍只读取本地 `catalog.json`；用户需要在插件页输入目录源并主动点击“刷新目录”，避免网络请求阻塞启动。

- 设置页恢复 `plugin_catalog_source_url` 偏好，并显示有界目录源输入框和稳定选择器 `plugin-package-catalog-source-input`、`plugin-package-catalog-refresh`。
- 只有组合根同时注入 `PluginPackageCatalogFetcher` 和 `PluginPackageCatalogStore` 时刷新按钮可用。输入为空、获取器缺失或存储器缺失时只显示本地错误，不发起请求。
- 刷新在后台任务中执行。获取和解析成功后更新候选并保存 URL 偏好；任何请求、状态、超量、解析或保存错误都保留旧候选和旧 `catalog.json`。
- 预览窗口未配置获取器和存储器，刷新按钮保持禁用；真实窗口检查覆盖输入可编辑、按钮状态、空目录和说明布局。真实 HTTPS 点击刷新继续保持未完成。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-ui --all-targets -- -D warnings`、
`cargo clippy --locked -p ramag-bin --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。插件诊断测试 10 项、真实 macOS 预览窗口、严格 Clippy、组合检查、格式检查和差异检查通过；真实 HTTPS 点击刷新和第三方来源信任仍未验收。

### 4.38 目录刷新 runtime 崩溃修复

真实生产调试包第一次点击“刷新目录”时进程退出。macOS 崩溃报告显示，GPUI 后台执行器直接驱动 `reqwest`，在 `tokio::time::timeout` 内调用 `Handle::current()`，因没有 Tokio reactor 触发 `SIGABRT`。

- `PluginPackageCatalogFetcher::fetch_and_store_on_managed_runtime` 在后台线程同步进入固定大小的专用 Tokio runtime 执行获取和保存，并把 runtime 初始化失败和请求失败转换为可显示的错误。
- 插件设置页刷新任务改用该入口。网络请求仍只在用户点击后开始，启动流程不联网；失败仍保留旧候选和旧 `catalog.json`。
- 新增 runtime 包装错误传播和真实 `reqwest` 请求上下文单测；第一次真实窗口点击提供了修复前崩溃证据，修复后的真实 HTTPS 点击返回 HTTP 404 错误卡且进程保持运行。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_package_catalog_fetcher::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo clippy --locked -p ramag-ui --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets`、`cargo build --locked -p ramag-bin` 和 `git diff --check`。
2026-11-13 已完成本切片。目录获取器定向测试 8 项、真实 macOS 生产调试窗口、严格 Clippy、组合检查、标准构建、格式检查和差异检查通过；第三方来源签名和成功目录仍需独立验收。

### 4.39 macOS `__eh_frame` 链接器警告修复

大型 GPUI 调试二进制在 macOS `ld64` 链接阶段输出 `__eh_frame section too large (max 16MB) to encode dwarf unwind offsets in compact unwind table`。该提示来自紧凑展开表容量，不是 Rust 源码错误。

- `.cargo/config.toml` 为 `aarch64-apple-darwin` 和 `x86_64-apple-darwin` 设置 `-C link-arg=-Wl,-no_compact_unwind`。
- 该参数只改变 macOS 链接器的展开表编码路径，保留 DWARF 展开信息；Windows 和 Linux 目标不继承该配置。
- 标准 `cargo build --locked -p ramag-bin` 和 `./scripts/build-dmg.sh --debug` 均不再输出该 `__eh_frame` 警告。构建仍可能显示依赖包的 future-incompatibility 提示，该提示不属于本切片。

验证命令为 `cargo build --locked -p ramag-bin`、`./scripts/build-dmg.sh --debug`、
`cargo fmt --all -- --check` 和 `git diff --check`。
2026-11-13 已完成本切片。macOS 标准构建和 DMG 调试构建通过，目标链接器警告已消失。

### 4.40 目录源本地 HTTPS 集成验收

本切片补充远程目录获取器缺少真实网络成功路径的验收证据。测试脚本在本机 Docker 中启动 `nginx:1.27-alpine`，使用临时目录生成自签名 `github.com` 证书、目录响应和有界错误响应。测试客户端只在该集成测试中允许自签证书，并把 `github.com` 解析到临时回环端口；生产获取器的 HTTPS 证书校验策略不变。

- `/plugins/catalog.json` 返回有效目录。测试通过 `fetch_and_store_on_managed_runtime` 验证生产后台入口，确认目录解析成功、原子写入完成，随后从 `PluginPackageCatalogStore` 读取到相同目录。
- `/plugins/not-found.json` 返回 `404`，`/plugins/redirect.json` 返回 `302`，`/plugins/oversized.json` 返回超过 `1 MiB` 的静态响应。三种失败都不会替换已保存的 `catalog.json`。
- 脚本检查容器状态为 `running`、`443/tcp` 映射到临时回环端口，并通过 `curl` 健康检查确认实际 HTTPS 连接。退出时删除命名容器和临时目录；脚本不使用一次性容器模式。

验证命令为 `bash -n scripts/plugin-test/run-local-catalog-tests.sh`、
`./scripts/plugin-test/run-local-catalog-tests.sh`、
`cargo test --locked -p ramag-app --lib plugin_package_catalog_fetcher::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets`、
`cargo fmt --all -- --check` 和 `git diff --check`。
2026-11-13 已完成本切片。默认 fetcher/store 测试 8 项通过，Docker fixture 集成测试 1 项通过；容器状态、端口映射、健康检查和退出清理均通过。该证据不覆盖公网证书链、目录签名、插件包下载、安装、沙箱或进程树清理。

### 4.41 插件包本地 HTTPS 集成验收

本切片补充 `PluginPackageDownloader` 缺少真实归档响应的验收证据。脚本在本机 Docker 中启动 `nginx:1.27-alpine`，使用临时目录生成自签名 `github.com` 证书、有效 `tar.gz`、超过 `64 MiB` 的响应和 HTTP 错误路由。测试客户端只在该集成测试中允许自签证书，并将 `github.com` 解析到临时回环端口；生产 `PluginPackageDownloader::new` 的证书校验策略不变。

- 有效 `remote-fixture.tar.gz` 通过 `download_to_managed_runtime` 下载到新目标，测试读取目标文件并核对 fixture SHA-256。
- `404`、`302`、`Content-Length` 超过 `MAX_PLUGIN_DOWNLOAD_BYTES` 和错误摘要分别返回 `HttpStatus`、`TooLarge` 或 `DigestMismatch`；失败目标不产生归档或 `.part-*` 临时文件。
- 成功归档在后续失败场景后仍可读取。脚本检查 fixture 容器状态为 `running`、`443/tcp` 映射到临时回环端口，并通过 `curl` 健康检查实际 HTTPS 连接；退出时删除命名容器和临时目录。

验证命令为 `bash -n scripts/plugin-test/run-local-download-tests.sh`、
`./scripts/plugin-test/run-local-download-tests.sh`、
`cargo test --locked -p ramag-app --lib plugin_package_downloader::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets`、
`cargo fmt --all -- --check` 和 `git diff --check`。
2026-11-13 已完成本切片。下载器默认测试 3 项通过，Docker fixture 集成测试 1 项通过；真实归档成功、HTTP 错误、超量、摘要错误和临时文件清理均通过。该证据不覆盖公网证书链、真实设置页点击、下载后安装、签名、沙箱或进程树清理。

### 4.42 目录刷新后恢复下载缓存

本切片修复目录刷新成功后已下载缓存从设置页消失的问题。刷新会先获取并校验新目录，再在后台使用 `PluginPackageCache::restore` 重新核验 `downloads/index.json` 和归档摘要，最后同时更新目录候选和“已下载”状态。

- 缓存恢复只接受与新目录中插件 ID、版本和 `archive_sha256` 都匹配的普通归档文件。摘要变化、文件缺失、路径异常和未知候选不会恢复。
- 缓存恢复失败只记录警告并显示新目录，不阻断目录刷新，也不在 UI 更新线程执行有界文件读取和 SHA-256 计算。
- 新增 headless GPUI 测试验证有效缓存仍显示安装按钮，并验证同一 ID/版本摘要变化后不会恢复旧缓存。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-ui --lib plugin_diagnostics::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-ui --all-targets -- -D warnings`、
`cargo clippy --locked -p ramag-bin --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。插件诊断定向测试 11 项、严格 Clippy、组合检查、格式检查和差异检查通过；真实窗口目录刷新后的缓存恢复仍需单独 UI 操作验收。

### 4.43 Unix 插件进程组清理

本切片补充 macOS/Linux 进程外插件宿主的子进程清理边界。启动入口时使用 `process_group(0)` 创建独立 Unix process group；握手失败、协议错误、入口调用超时和关闭超时时向负的 PGID 发送 `SIGKILL`，随后继续回收直接子进程。这样可以覆盖入口启动的子进程，避免只终止入口进程后留下后台进程。

- 进程组 ID 只来自本次宿主创建的子进程。ID 无效或进程组已经不存在时，清理函数返回失败状态但不阻断后续直接子进程回收。
- Windows 不使用 Unix `libc::kill`；Windows Job Object、沙箱和资源配额仍需单独设计和验收。
- 新增真实 Unix `/bin/sh` 子进程测试。测试让 shell 创建 `sleep` 子进程，再终止进程组并确认 shell 以非成功状态结束。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_process::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo check --locked -p ramag-bin --all-targets` 和 `git diff --check`。
2026-11-13 已完成本切片。Unix 进程宿主定向测试 6 项、严格 Clippy、组合检查、格式检查和差异检查通过；Windows Job Object、沙箱、签名和资源配额仍未实现。

### 4.44 Windows Job Object 进程树清理

本切片补充 Windows 进程外插件宿主的进程树清理实现。入口进程创建后立即创建匿名 Job Object，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`，并通过 `Child::raw_handle()` 把入口加入 Job。协议错误、入口调用超时、握手失败和关闭超时时调用 `TerminateJobObject`，再执行直接子进程终止和等待；Job handle 使用 RAII 关闭，避免会话异常退出时遗留进程。

- Job 创建、配置或入口分配失败会在启动阶段返回明确的 `PluginProcessError::JobObject`，不会静默退化为只终止直接入口。
- Windows 目标使用 `windows 0.57` 的 `Win32_Foundation` 和 `Win32_System_JobObjects` 功能；`ramag-infra-system` 保持为跨平台直接依赖，避免 Windows 目标在资源采样代码处缺少依赖。
- Job Object 的真实子孙进程清理需要 Windows 运行环境。当前 macOS 只完成 Windows 目标库和全目标代码检查，不能把该检查当作 Windows 运行时验收。
- 入口启动到 Job 接管之间仍存在 Windows 进程创建竞态；本切片不引入自定义 `CreateProcessW`/挂起恢复流程，也不把 Job Object 当作沙箱或资源配额实现。

验证命令为 `cargo fmt --all -- --check`、
`cargo test --locked -p ramag-app --lib plugin_process::tests -- --test-threads=1`、
`cargo clippy --locked -p ramag-app --all-targets -- -D warnings`、
`cargo check --locked -p ramag-app --target x86_64-pc-windows-msvc --lib` 和 `git diff --check`。
2026-11-13 已完成代码实现和跨目标静态检查；Windows 真实 Job Object 子孙进程验收仍需 Windows 运行环境，签名、沙箱和资源配额仍未实现。

## 5. 条件限制与未纳入执行的事项

以下内容保留其未验证状态，不因合并文档自动启动新功能。

| 事项 | 处理边界 |
|---|---|
| 平台与硬件 | macOS 网络自动选择、Linux/macOS 原生窗口与秘密存储、GPU/PawnIO 不同硬件、进程授权、托盘恢复和跨 DPI/多显示器，需要对应平台或设备证据；Windows 开发机结果不能代替 |
| 监控差异 | 单位/量程/图例、失败来源、键盘焦点、配置损坏恢复和后台历史连续性按实际缺陷复核；新增温和结束、托盘动态图、偏好同步等能力先取得明确范围 |
| 外部服务 | COS/OSS 云端流程、真实 Kubernetes、Kafka TLS/SASL/ACL 服务兼容及生产 exporter 安全配置，须具备对应测试环境；RustFS、明文 Broker 或代码检查不替代这些结果 |
| 更新库跨程序复用 | [架构说明](02-architecture.md#跨程序复用评估)已评估提取边界与现有方案；尚未安排提取或发布公共库。Ramag 根许可证为 `GPL-3.0-only`，与 `it-tools-template` 保持现有独立边界 |
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
- Linux 本机仅用 WSL 编译；Linux/macOS 测试由 `.github/workflows/desktop-ci.yml` 执行。普通开发不读取 GitHub CI 运行结果。
  发布 release 或用户明确要求时，再记录仓库、提交、运行编号、job、失败阶段和原生验收范围。三平台质量检查、发布打包和原生验收分别记录。
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
