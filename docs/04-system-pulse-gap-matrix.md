# System Pulse 优点吸收与差距矩阵

> 状态：完整源码迁入方案已于 2026-10-02 确认并正在实施；以下旧视图差距记录保留供追溯，不作为迁入后的完成证据
> 日期：2026-10-02
> 参考提交：`f1be5d51d24c21fa8c740be79200bdda3df3a00c`
> 专项设计：[`03-system-pulse-adoption-plan.md`](03-system-pulse-adoption-plan.md)
> 验收标准：[`07-ui-acceptance-standard.md`](07-ui-acceptance-standard.md)

## 术语与命名

| 规范中文名 | English / Acronym | 职责边界 | 不代表什么 |
|---|---|---|---|
| 系统采集器 | System Collector | 后台读取设备、传感器和进程并产生状态化快照 | 不代表监控页面已展示全部数据 |
| 系统监控视图 | System Monitor View | Ramag 中的十个固定监控页面及其操作 | 不代表嵌入 System Pulse 窗口 |
| 温和结束 | Graceful Close | 请求目标进程正常退出，允许拒绝或保存提示 | 不代表进程已经退出或强制终止 |
| 强制退出 | Force Quit | 核对稳定身份后强制终止一个目标进程 | 不代表终止其子进程或保留未保存数据 |
| 监控预设 | Monitor Preset | 有界保存采样、设备和传感器展示偏好 | 不代表连接、凭据或采样历史备份 |
| 差距切片 | Gap Slice | 一个可独立实现、验证、提交和回滚的差距项 | 不代表全应用完成 |

## 1. 范围与依据

用户要求逐项对齐 System Pulse 的实际运行功能和 UI，功能完善和已知 UI/功能 bug 优先，键盘增强、过渡动画及特效后置。本矩阵以本机运行的固定提交作为主要对照：先操作参考程序，再在 Ramag 中复现相同流程，核对数据来源、状态、布局和操作结果。源码、用户指南和历史截图辅助解释差异；仅有当前项目截图、源码或 headless 结果不能证明已与参考程序对齐。

参考源码在本机 `F:/project/system-pulse`，已用 `git rev-parse HEAD` 核对提交。2026-10-01 在该目录使用 `cargo run --locked` 编译并启动真实 Windows 程序；`SYSTEM_PULSE_STATE_DIR` 指向独立的临时配置目录，不读取或修改用户已有设置。参考依据包含真实窗口操作、[上游用户指南](https://github.com/eas4ai/system-pulse/blob/f1be5d51d24c21fa8c740be79200bdda3df3a00c/docs/user-guide.md)、`src/process_panel.rs`、`src/screen_summary.rs`、`src/settings.rs` 和 `src/workspace.rs`。保留 Ramag 壳层与主题约定，不复制参考字体、图片或窗口资源。

“充分吸收”逐项记录已接入、存在缺口、待验证或后置，不用总测试数或页面名称数量计算完成比例。继续沿用 Ramag 原生 GPUI、工具注册、全局设置和后台任务生命周期。上游功能与 Ramag 多工具工作台存在职责差异时，记录对应实现及理由，而非启动另一个监控应用。

## 2. 差距与验收

| 优点或能力 | Ramag 当前证据与差距 | 优先级与独立验收 |
|---|---|---|
| 十页固定信息架构 | Summary、CPU、Memory、GPU、Disks、Network、Energy、Thermals、Processes、Settings 已接入；硬件缺失仍保留页面 | 已接入；逐页检查正常、空、失败、不可用与过期，不能用导航存在代替数据正确 |
| 状态化真实采样 | `ramag-infra-system` 已纳入进程读写速率、线程数、用户读取失败原因、设备、诊断和平台后端；UI 尚未充分呈现进程字段 | P1；复用现有快照，不新建第二个采集器；不可用/失败值保留原因和单位 |
| 安全进程操作 | 已确认、拒绝自进程、核对 PID/精确启动身份/名称；当前强制操作已明确为“强制退出”，并显示未保存数据及子进程边界。温和结束尚未提供 | 强制风险表达已收口；温和结束按平台能力独立实现。确认/取消、身份变化、失败与退出回读分别验证，不用 UI 文案验收代表平台操作能力新增 |
| 进程表与详情 | `02A` 已补齐七列双方向排序、读写单位、失败原因提示、紧凑行与横向滚动；`02B` 已接入完整身份绑定详情、退出/PID 复用/过期提示。搜索名称/PID/用户，最多显示有界行数；键盘行导航仍缺失 | P1；`02C` 补键盘行导航；字段/交互分别验收 |
| Summary 信息组合 | 当前 CPU/内存读数和趋势、Top 进程已接入；GPU/磁盘/网络主要为设备数量入口，Summary 缺少 Energy/Thermals 入口及这些子系统趋势，也缺少上游 CPU 频率/温度、逐逻辑处理器趋势和内存可用/缓存/swap 数值组合 | P1；逐子系统补所选来源的 GPU/磁盘读写/网络 RX/TX/能耗/温度趋势和读数。1440x900 与 1024x768 首屏组合及 360x640 滚动验证，图表保留缺口和时间；Energy 使用命名传感器，不能加总包/组件功率 |
| 物理单位与曲线量程 | 已有物理单位、时间图表和缺口处理；缺少上游组合量表、悬停传感器/量程说明及图例的逐页核对 | P1 图例/单位/量程先核对，量表表现后续深化；CPU 进程可超过单核 100%，能耗不能推算系统总功率 |
| 网络默认接口选择 | Linux 快照及 UI 已使用 main-table 默认路由选择，手动选择优先；连接归属字段表达接口地址/TCP 表关联，不代表跨平台默认路由。Windows/macOS 自动选择待核对 | P1 核验；Linux IPv4/IPv6 路由、无默认路由、手动优先、设备移除/恢复均有断言；其他平台独立记录，不静默替换已保存设备 |
| Energy 单传感器选择 | Energy 页按稳定传感器 ID 选择一个主功率来源，显示选中读数、量表、历史图、来源/范围/单位，并保留全部 measured channels；已保存 ID 缺失时显示不可用原因，不静默切换，也不把包/组件功率相加 | `A-PULSE-GAP-06-Energy` 已完成；真实值随采样变化不作为跨窗口精度比较 |
| 温度重点信息 | Thermals 页按稳定传感器 ID 选择主温度来源，突出最热当前传感器，显示动态摄氏量程、历史图、全部温度卡片和 Windows CPU 温度授权入口；保存 ID 缺失、过期、失败和不可用状态均保留原因 | `A-PULSE-GAP-06-Thermals` 已完成；Headless 覆盖多状态和多传感器规则，Computer Use 覆盖参考/Ramag 同流程，其他硬件平台与授权成功路径另行验证 |
| 设备和传感器偏好 | 已持久化设备选择与隐藏传感器；上游不可用传感器自动隐藏，Ramag 原计划保留原因 | 保留 Ramag 的明确不可用原因；可增加“隐藏不可用”偏好，默认不能造成导航跳动；失败/过期仍可辨识 |
| 键盘与屏幕恢复 | 设备偏好已保存，固定页签可点击；尚未证明方向键/Home/End、Ctrl+Tab 循环及监控上次页面恢复 | P1；真实键盘和 headless 命中测试，焦点可见且不困在表格，恢复不改变业务选择 |
| 命名监控预设 | `MonitorPresetLibrary` 有版本、名称/数量/字节上限；支持 Default、Minimal、GPU Focus、Developer 内置入口，以及命名预设创建/应用/重命名/覆盖/删除 | 已实现并通过目标测试与真实窗口流程；固定十页结构不写入快照，连接、凭据、历史样本和不可迁移布局仍排除 |
| 保存与恢复反馈 | 系统/主题/监控采样保存中、已保存、失败原因和重试已通过本机验收；其他工具保存流程及损坏配置恢复仍待专项核对。上游显式归档损坏配置后恢复 | 系统/主题/采样已接入；P1 核对其他工具和 redb 恢复边界，损坏内容不被普通退出静默覆盖；记录原因和恢复动作 |
| 进程授权和结果边界 | 采集器已保留平台认证/操作后端；当前工具强制退出链路需逐平台核对权限失败、授权互斥、超时及退出回读，不能由后端源码存在推断完整 UI 已接入 | P1；仅操作测试拥有的子进程，确认锁定完整身份；普通权限、拒绝/取消、授权后身份改变、受保护目标、超时结果不确定分别验证。系统认证输入由用户完成，主窗口权限不提升 |
| 后台监控与历史连续性 | 当前监控实例拥有采样器及历史，工具视图重建可能重置；上游窗口关闭后托盘可继续监控 | P2；先核对 Ramag 壳层是否保留工具实体，按真实生命周期设计服务所有权，历史/任务有界，退出全部释放 |
| 托盘图与宿主恢复 | Ramag 已有托盘驻留偏好，上游托盘有 CPU 图及宿主丢失后的窗口恢复 | P2；与应用级托盘合并，正常/不可用提示、无宿主退出和恢复分别验证，不能重复创建托盘 |
| 标题、分区与领域配色 | Shell/首页已有切片证据；设置仍使用整段外框，监控有重复层级、较大空白、磁盘/能耗领域颜色差异 | P1 修布局和可达性，视觉深化单独验收；对照明暗主题、三个尺寸和最大字号截图，不以改边框宣布全盘完成 |
| 正常交互动效 | GPUI Kit 已支持系统偏好及可中断过渡；已有应用优化草稿 | P2 后置；正常动画保留，辅助功能明确请求时减少；不延迟事件、不排队、不持续空闲重绘 |
| 可复现验证与发布 | 来源、GPL-3.0-or-later 和原生模块归属已保留；当前不执行发行包、签名和 GitHub CI 检查 | 本轮本机 fmt、workspace Clippy、目标/UI 测试；GPU、PawnIO 实机、macOS 和正式签名包分别列未验证，不能引用上游结果替代 |

## 3. 执行顺序

1. `A-QUALITY-SETTINGS-SAVE-001`：收口保存失败原因、重试与实际回读，保持界面可操作。
2. `A-PULSE-GAP-01`：明确当前进程操作是强制退出，修复风险文字和紧凑操作区。
3. `A-PULSE-GAP-02A/B`：进程表多列排序、访问失败原因与选中进程详情；已完成。`02C` 键盘操作按用户要求后置。
4. `A-PULSE-GAP-03`：Summary 子系统真实读数与趋势、CPU/内存信息组合及布局，逐子系统验证来源时间、范围和缺口；当前主线。
5. `A-PULSE-GAP-04`：自动/手动设备选择优先和上次页面恢复；页签键盘导航后置。
6. `A-PULSE-GAP-05`：监控预设管理与配置恢复；已完成固定监控工作台快照、内置入口和独立写操作确认；完整跨页面工作区布局仍不进入 Ramag 预设。
7. `A-PULSE-GAP-06`：Energy/Thermals 单传感器选择、最热当前传感器、动态摄氏量程和授权结果；Energy 与 Thermals 已分别提交并完成本机验收。
8. `A-PULSE-GAP-07`：服务和历史生命周期、托盘图及宿主恢复。
9. `A-PULSE-GAP-08`：剩余视觉细节与量表；功能/UI 数据视图对齐之后再实施 `02C` 键盘、页签快捷键、过渡动效和发布性能专项。

每个独立功能先写问题、设计与验收，再实现和运行匹配本机检查，通过后独立提交并立即推送 `main`。涉及数据库或协议服务时才使用本机 Docker；纯监控、UI 和本地偏好切片记录 Docker 不适用。Computer Use 证据、headless 证据、真实硬件准确性和发布性能各自注明范围。

## 4. 本机验证入口

所有切片从 workspace 根目录先运行对应目标测试，再依次运行 `cargo fmt --all -- --check`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、`powershell -NoProfile -File scripts/windows/check-source-size.ps1`、`git diff --check` 和修改文本 LF 检查。影响公共 UI 或存储时运行 `cargo test --locked --workspace -- --test-threads=1`；纯监控切片至少运行 `cargo test --locked -p ramag-infra-system -p ramag-tool-system -p ramag-ui --lib -- --test-threads=1`。环境变量控制或忽略的外部服务测试不计为已运行集成证据。

| 差距切片 | 测试入口与必须新增的断言 | 原生证据及限制 |
|---|---|---|
| 设置保存 | `ramag-ui::preferences::tests`、`preferences::status_tests`；实际 redb 值、最新 revision、失败草稿重试、慢写入可操作、三尺寸状态与重试区域 | 隔离设置预览点击主题/字号/刷新并切页；失败/延迟注入保持 headless 证据，不冒充真实磁盘故障 |
| 进程风险与详情 | `ramag-tool-system::view::render_test` 和进程排序单元测试；确认/取消不改变目标、身份变更拒绝、各物理字段排序、缺失值和长名称、详情稳定身份 | 仅测试拥有的子进程；新增温和结束/授权按平台单独回读，不操作用户进程 |
| Summary 趋势 | `ramag-tool-system::view::render_test`；使用快照/历史测试数据构造当前、预热、失败、不可用、过期、时间缺口与空设备，三尺寸检查每个子系统图例/单位/来源时间/主要区域可达 | 实际启动 System Pulse，操作设备切换、分页往返和采样刷新，再在同主题、同尺寸 Ramag 窗口执行同一流程；分别记录参考结果、Ramag 结果和剩余差距，测试数据不能证明硬件精度 |
| 网络选择与恢复 | `ramag-infra-system::host::network_route::tests` 与监控选择测试；IPv4 优先、IPv6 回退、物理接口回退、手动优先、设备消失保留 ID 和原因 | Linux 路由本机验证，Windows/macOS 自动选择另列；未运行的平台保持未验证 |
| Energy/Thermals | 监控页面测试；选择稳定 ID、保存后重建、仅当前温度取最大、选中不可用保留原因、不同能耗范围不加总 | PawnIO、EMI、GPU/Apple 等实际硬件分别记录；没有硬件不能宣称精度通过 |
| 预设/配置恢复/生命周期 | `ramag-ui` 预设库与视图测试；创建/应用/重命名/覆盖/删除确认及取消、名称/数量/字节上限、损坏数据拒绝覆盖 | Computer Use 已在 System Pulse Settings 与 Ramag“设置 -> 系统监控”完成同流程；隔离预览不证明生产 redb 重开回读，托盘宿主恢复仍需真实窗口证据 |

新增测试入口以最终函数名写入对应验收记录，记录完整命令、退出码、操作、结果与未完成项。图表和表格测试必须断言来源状态与真实控件行为，不能仅数元素或比较实现常量。每项的 motion、发布性能、跨平台与硬件状态单列，避免与本机功能通过混用。

## 5. 当前进程风险修复设计

`A-PULSE-GAP-01` 只明确现有强制操作，不新增温和结束或权限能力。桌面进程行使用“强制退出”；窄窗口使用固定尺寸的退出图标并提供“强制退出此进程”提示。确认标题、正文、确认按钮和失败提示使用同一名称；正文写明可能丢失未保存数据、只影响目标进程且不包括子进程。捕获 PID、精确启动身份和名称后仍由已有后台路径重新核对，不复用列表位置。

确认正文在长名称或 `360x240`/18px 下可滚动，标题和取消/确认保留可达；默认焦点与 Escape/Tab 取消流程继续保留。测试使用既有监控渲染入口，覆盖明暗三尺寸和两种字号、长名称确认边界、取消不启动终止，以及原进程身份/失败测试。真实窗口仅检查文案、布局、打开确认和取消，不执行用户进程操作；Docker 不适用。通过本机目标测试、fmt、workspace all-target Clippy、源码尺寸及差异/LF 检查后独立提交。

### 2026-10-01 进程风险修复验收

`A-PULSE-GAP-01` 统一进程行、确认标题/按钮和结果提示为“强制退出”。紧凑行采用带风险提示的 CircleX 图标；桌面行保留明确文字，操作区固定且不溢出。风险正文先显示未保存数据与单进程范围，再显示捕获的目标和 PID；正文滚动在采样重绘中保持，新确认从顶部开始。打开后容器焦点上的 Enter 不执行操作，Tab 可进入取消/确认，Escape 和取消不启动终止任务。

本机 `ramag-tool-system` 的 17 项测试通过。`view::render::tests::termination_tests` 覆盖明暗主题 × 标准/大字号 × 三种页面尺寸，追加 `360x240` 长英文/中文确认框，检查标题/正文/操作边界、真实滚动偏移和取消后的状态。采集器/UI 联合目标测试为 111 项采集器通过、1 项忽略、17 项监控和 124 项公共 UI 通过，日志 `target/pulse-force-quit-tests-20261001.log`；忽略项不计为通过。`cargo build --locked -p ramag-bin` 与隔离预览构建通过。

Computer Use 在更新后的 `1024x768` 亮色实际窗口进入 Processes，按 PID 筛选本轮专属 PowerShell 等待进程，打开确认并核对风险正文和目标，再点击取消；回读确认该测试进程仍在运行。预览正常关闭，测试子进程已清理。原生证据只覆盖标签、正文、确认打开和取消；窄窗口/大字号/长名称滚动是 headless 证据，不代表原生强制终止或授权流程完成。Docker 服务、镜像、端口、启动和清理均不适用，GitHub CI 未检查。

最终本机 `cargo test --locked --workspace --quiet -- --test-threads=1` 通过，完整日志 `target/pulse-gap-workspace-tests-20261001.log`；补充初始 Enter 不提交的目标回归通过。最终 fmt、workspace all-target Clippy `-D warnings`、源码尺寸、差异与 LF 检查通过。环境变量控制或忽略的服务测试不计为本机 Docker 集成证据。

## 6. 进程表补齐设计与验收

`A-PULSE-GAP-02A` 补齐 PID、名称、用户、CPU、内存、读取速率和写入速率的排序及展示。每个排序键使用稳定 ID；再次选择同键反转方向，首次选择 PID/文本升序、资源数值降序。不可用、失败、预热、过期和非有限值始终排在当前值之后，同值按 PID 和精确启动身份确定顺序；真实零保持可排序，CPU 保留单核口径并允许大于 100%。搜索按名称/PID/用户，不读取命令行或环境。

桌面使用对齐的七列和独立操作列，中等宽度支持真实横向滚动，表头可点击排序。`360x640` 使用两层紧凑行，保留名称/PID/用户、CPU/内存/读取/写入单位和退出图标；排序字段从菜单选择，方向使用箭头图标及提示。长文本和失败原因可查看全文，不把缺失值显示成零。所有模式保留危险操作确认。

`A-PULSE-GAP-02B` 再补选中进程详情，绑定 PID 与精确启动身份；排序、过滤与刷新不会把选择转给另一进程。详情展示完整名称/用户、单位、线程数和字段读取原因。退出或 PID 复用时清除旧指标并显示原因；这项独立验证和提交，键盘行导航作为 `02C` 单独接入。

`02A` 验收：排序单元测试覆盖七键、双方向、默认方向、零/缺失/失败/过期/非有限值、稳定同值及中文搜索；headless 点击全部表头及紧凑菜单，实际断言排序结果、单元格边界、横向滚动、状态/单位和确认目标。覆盖明暗主题 × 16px/18px × `360x640`/`1024x768`/`1440x900`，另测 640px 中等宽度。Computer Use 使用隔离预览搜索和排序，仅打开确认/取消测试专属目标。目标/UI 测试、fmt、workspace all-target Clippy、源码尺寸和差异/LF 通过后独立提交推送 `main`，Docker 不适用，不检查 GitHub CI。

### 2026-10-01 A-PULSE-GAP-02A 验收

代码已补齐七列桌面表、固定操作列、稳定方向图标、紧凑 2×2 指标行和中等窗口横向滚动；`MonitorSnapshot::sorted_processes` 统一过滤与排序，缺失、失败、预热、不可用、过期和非有限读数保留状态/单位并排在当前有效值之后。排序单元测试与监控渲染测试共 30 项通过；联合采集器 111 项（1 项忽略）和公共 UI 124 项通过，完整日志 `target/pulse-process-table-final-tests-20261001.log`。`cargo build --locked -p ramag-bin`、隔离预览构建、fmt、workspace all-target Clippy、源码尺寸、差异和 LF 检查通过。

Computer Use 在 `1024x768` 亮色窗口完成 Processes 导航、PID 升序/降序、PID 搜索、测试子进程强制退出确认与取消，回读目标仍在运行；在 `360x640` 暗色窗口核对紧凑身份区、CPU/内存/读取/写入单位、排序菜单和方向提示。Headless 证据覆盖其余尺寸、字号、主题、字段顺序、滚动和实际排序结果。原生证据未执行终止操作，也不覆盖进程详情、键盘行导航、跨平台/硬件或发布性能；Docker 不适用，GitHub CI 未检查。下一项为 `A-PULSE-GAP-02B` 进程详情。

最终 `cargo test --locked --workspace --quiet -- --test-threads=1` 通过，日志 `target/pulse-process-table-workspace-tests-20261001.log`；环境变量控制或忽略的服务测试不计为 Docker 集成证据。亮色预览通过 Alt+F4 正常关闭；暗色预览与最初无窗口的隐藏预览由本轮进程清理结束，测试子进程及本轮剩余临时 redb 已清理。没有修改用户存储、连接或系统偏好。

## 7. 选中进程详情设计与验收

`A-PULSE-GAP-02B` 沿用第 6 节已确认的进程详情范围。点击进程行选中完整身份（PID 与精确启动时间），详情区放在排序工具栏与列表之间，标题和关闭图标固定，正文高度有界且可滚动。只保存一项身份和名称，不复制旧指标或读取命令行、环境、凭据。选择、详情和强制退出各自独立；点击退出按钮只打开原有身份确认，不能因事件冒泡切换详情目标。

每次渲染从未过滤的最新快照查找完整身份，排序、搜索、行数上限和切页往返均不改变目标。详情显示完整名称、PID、用户、CPU、内存、读取速率、写入速率、线程数、各字段状态/原因与快照采样时间。不存在精确身份时移除所有旧指标，显示“本次快照未包含此进程，可能已退出或无法读取”；同 PID 的启动身份改变时显示 PID 已被复用，不展示新进程值。过期快照保留明确过期状态而非当前读数，字段为空或非有限值不显示为零。关闭只清除详情选择，不发送进程信号。

验收覆盖明暗主题 × `360x640`/`1024x768`/`1440x900` × 16px/18px：实际行点击、选中反馈、排序/过滤/刷新/切页后身份保持、线程数与单位、字段失败原因、长英文/中文名称和用户、正文滚动与固定关闭入口；缺失、PID 复用、过期和关闭分别断言。Computer Use 在隔离预览中选择本轮专属子进程、查看详情、过滤隐藏目标、取消危险确认和关闭详情；从外部结束测试子进程后回读缺失状态。Docker 服务、镜像、端口、启动和清理均不适用；本轮不检查 GitHub CI，键盘行导航仍在 `02C`。

### 2026-10-01 A-PULSE-GAP-02B 验收

进程详情与选择实现完成。`process_detail_tests` 的 4 项 headless 交互回归覆盖真实行点击与关闭、排序/过滤/切页保持身份、其他行退出按钮不切换目标、最新快照更新/退出缺失/PID 复用/过期、长名称与字段失败原因、明暗三尺寸及两种字号正文滚动；精确启动身份解析和详情格式测试另有 3 项。监控 39 项、采集器 111 项（1 项忽略）和公共 UI 124 项通过，日志 `target/pulse-details-axis-final-tests-20261001.log`。

Computer Use 已在隔离亮色 `1024x768` 预览中选择本轮专属子进程、检查身份和实时字段、隐藏列表目标后保持详情、取消危险确认及关闭详情；结束该测试子进程后观察详情移除旧指标并显示缺失原因。PID 复用、过期和原生长文字滚动的其余矩阵由 headless 覆盖，不描述为原生故障注入。仅操作本轮测试目标，没有改动用户存储或发送用户进程信号；Docker 与 GitHub CI 不适用或未检查，发布性能和键盘操作继续保持未完成。

最终 workspace 回归通过，日志 `target/pulse-details-axis-transparent-window-workspace-tests-20261001.log`；fmt、workspace all-target Clippy `-D warnings`、源码尺寸、差异与 LF 检查通过。环境变量控制或忽略的服务测试不计为本机 Docker 集成证据。按用户后续指示，下一项进入 `A-PULSE-GAP-03` 的 Summary 功能与 UI 对齐。

## 8. Summary 设备活动趋势设计

2026-10-01 用户明确功能与 UI 视图对齐为当前主线，键盘导航属于后置增强。实际运行固定提交的 System Pulse 后，`A-PULSE-GAP-03A` 先将网络与磁盘的设备数量入口改为所选设备的真实活动区：设备名称、磁盘容量、双向速率图例和两条共用时间/量程的趋势曲线，并保留进入详细页面的图标入口。此切片只验收磁盘/网络活动能力；Summary 的整体组合布局及其余子系统仍未对齐。

Summary 与详细页面使用同一设备选择规则：保存的手动 ID 优先，网络其次使用快照推荐接口，最后使用首个匹配设备。已保存设备消失时显示该 ID 和不可用原因，不能静默换到其他设备。磁盘使用所选卷的读取/写入速率，网络使用所选接口的 RX/TX；只从同一 monitor_id、Rate 类型和 B/s 单位的描述读取历史，不能混用累计计数或跨设备加总。真实零保留，预热/失败/不可用/过期/无效值显示对应状态与原因，曲线断开缺失和采样中断。两条曲线使用共同来源时间范围，单位和量程一致，最多保留既有有界历史。

验收包含两设备明显不同的读数、手动/推荐/缺失 ID、历史时间不齐、当前零、各状态、非有限值、长设备名称和原因、明暗三尺寸及大字号。真实窗口核对设备名称、图例、图表和进入页面后设备一致；不把本机读数展示当成网卡/磁盘硬件精度证明。Docker 服务、镜像、端口不适用。后续分别补 CPU/内存紧凑组合、GPU、Energy/Thermals 的真实读数和趋势；键盘、动效与发布性能后置。

### 真实参考窗口观察与后续设计

Computer Use 在 `1282x912` 暗色 System Pulse 窗口观察到：首排依次是 CPU/Clock/Temp/GPU 四个小型竖向量表、CPU overview 和 Top CPU processes；第二排为全宽 Memory utilization，包含已用/总量、趋势、RAM available 和 Swap used。下面五个子系统为 Disks、Network、Energy、GPU、Thermals，当前宽度分成三项和两项。宽屏的一排五项及窄屏行为需另外实际核对，不能由源码推断为已验收。

参考磁盘页通过菜单从 C 盘切到 F 盘，容量从约 `162.8 / 299.8 GiB` 变为 `390.9 / 465.8 GiB`；返回 Summary 后保留 F 盘，活动图和容量对应所选盘。磁盘读取使用绿色、写入使用黄色，网络接收使用蓝色、发送使用黄色，两条方向曲线共用量程。实时 CPU、磁盘和网络读数会随采样变化，双窗口验收比较来源、单位、状态及行为，不要求不同采样时刻数值完全相等。

下一项 `03B` 按上述实际组合调整 CPU/内存区域：四个小型量表使用主机 CPU 利用率、CPU 频率、当前最高温度和所选 GPU 利用率；CPU overview 显示主机归一化读数、历史和逻辑处理器数量；Top 进程保持单核 CPU 口径；内存全宽显示已用/总量、可用 RAM 和 swap。每个量表使用自己的物理单位和来源，不把核心数量替代四个量表；缺失、失败和过期值保留状态。Energy/Thermals/GPU 活动及详细分页的剩余差距分别验收。

### 2026-10-01 `A-PULSE-GAP-03B` 实际布局对齐

参考 System Pulse 的真实 `1282x912` 窗口和可访问树，Ramag Summary 首屏已重排为：四个领域色量表、CPU overview 历史图、Top CPU processes；第二排为全宽 Memory utilization，显示已用/总量趋势及 Available、Cache、Swap used；磁盘和网络活动卡继续位于下方。CPU、频率、温度、GPU、内存和状态文本均从现有传感器描述与快照读取，不用零填充不可用值。

Ramag 隔离预览重新构建后在真实窗口回读：CPU、Clock、GPU 量表、CPU overview、Top CPU processes 和全宽 Memory utilization 均可见，窗口继续滚动到磁盘/网络活动区域；无 CPU 温度传感器时显示“不可用”，缓存字段未暴露时显示紧凑的“不可用”状态，完整采集器原因通过悬浮提示保留，避免挤压 Swap 标签。主窗口恢复下限和原生最小尺寸统一为 `960x640`，与 System Pulse 的窗口约束一致；组件 headless 仍覆盖 `360x640`，用于验证内容在窄视口中滚动可达，而不是把真实桌面窗口压缩到不可读尺寸。

本切片完成 Summary 的首屏布局与数据组合对齐，不代表 Energy/Thermals/GPU 详细趋势、宽屏五卡同排或跨平台硬件精度已完成。下一切片先按同一真实窗口流程补齐剩余子系统。

### 2026-10-02 `A-PULSE-GAP-03C` GPU Summary 卡片设计确认

System Pulse 的 Summary 在 GPU 子系统区域显示当前选定 GPU 的利用率和历史趋势，图表固定使用 `0–100%` 百分比范围，并保留进入 GPU 详细页的操作。Ramag 当前只显示 GPU 设备数量，不能在 Summary 直接判断利用率或时间变化。本切片只补 GPU Summary 卡片，不改 GPU 详细页、采集器或设备选择设置。

实现沿用 Summary 已有的稳定 GPU 选择规则：保存的 GPU ID 优先，没有保存值时选首个 GPU；保存的设备消失时继续显示该 ID 和不可用原因，不能静默切换。利用率只读取所选 monitor 的 summary sensor，要求百分比单位和有效的 `0..=100` 值；真实零值保留，预热、失败、过期、缺失和无效值保留状态及原因。历史点沿用采集时间和缺口规则，卡片使用 GPU 领域色、当前读数、`0 %` 与最大值标签，并保留进入详细页的图标按钮。

验收覆盖两个不同 GPU 的选择与回退、有效零值、预热/失败/过期/缺失/非有限和越界值、三种窗口尺寸、明暗主题及 16px/18px 字号。目标测试、workspace fmt、Clippy、源码尺寸、差异/LF 检查通过后，再用 Computer Use 先观察 System Pulse Summary 的 GPU 卡片，再在同尺寸 Ramag 窗口执行同一页签和详细页导航流程。硬件精度、跨平台驱动和 GPU 详细页布局另行记录；Docker 不适用。

### 2026-10-02 `A-PULSE-GAP-03C` GPU Summary 真实窗口验收

Computer Use 先启动并确认只有一个 System Pulse 参考窗口（`1282x912`），Summary 可访问树回读 GPU 读数 `30.0 %`、GPU 历史图 `0.0–100.0 %` 和 GPU 页签。随后启动当前 `ui_preview` 构建并确认只有一个 Ramag 监控窗口（`1271x944`）；同一流程回读 Summary 页签，点击 GPU 页签后出现 `NVIDIA GeForce RTX 3060` 设备入口，再返回 Summary 并滚动到下方子系统区域观察 GPU 卡片。两窗口均由 Computer Use 捕获真实窗口状态，未使用旧安装程序或第二个并行实例。

Ramag 的 GPU Summary 卡片现在显示所选设备、利用率百分比、`0–100%` 纵轴趋势、过期/失败/不可用原因和进入 GPU 详细页的箭头；稳定设备选择、有效零值、越界值、过期值和缺失 ID 由 `summary_gpu_tests` 的 headless 回归覆盖。`ramag-tool-system` 全量 58 项、fmt、workspace Clippy、源码尺寸、`git diff --check` 和 UI preview 构建通过。实际 GPU 硬件精度、跨平台驱动和详细页卡片排布仍未在本切片宣称完成；Docker 不适用。

### 2026-10-01 `A-PULSE-GAP-03A` 真实运行验收

参考程序以固定提交 `f1be5d51d24c21fa8c740be79200bdda3df3a00c` 在 `F:/project/system-pulse` 本机编译运行，窗口为 `1282x912` 暗色主题。Computer Use 操作磁盘下拉框从 `C:\` 切到 `F:\`，回到 Summary 后仍显示 `F:\`，容量和读写曲线随所选文件系统更新；网络页下拉框显示多个接口，双向曲线和累计值分别保留。参考程序 Summary 在同一窗口观察到磁盘、网络、能耗、GPU、温度子系统组合。

Ramag 隔离预览使用 `cargo build --locked -p ramag-bin --example ui-preview` 构建并运行 `system dark 1282 912`，在真实窗口回读 Summary：磁盘卡显示所选 `C:\`、容量、读取/写入单位和双向趋势，网络卡显示所选接口、接收/发送单位和双向趋势，GPU 设备入口可达；活动卡箭头可进入对应详细页。代码测试构造两个不同设备、保存设备优先、网络默认接口、缺失设备、零值、失败/过期、非有限值、时间缺口和窄窗口滚动导航，`ramag-tool-system` 42 项与 `ramag-ui` 126 项通过。

本切片的真实窗口证据只证明活动来源、单位、选择保持和页面操作，不证明磁盘/网卡硬件精度，也不证明参考程序与 Ramag 在不同采样时刻的数值相等。CPU/内存 Summary 组合已由 `03B` 覆盖；Energy/Thermals/GPU 趋势和 1440 宽屏五卡布局仍属于后续未完成项。

### 2026-10-01 十页真实窗口逐页对比

本次验收使用 Computer Use 分别操作两个真实窗口，而不是只读取截图或 headless 输出：System Pulse 固定提交 `f1be5d51d24c21fa8c740be79200bdda3df3a00c`，Ramag 使用当前隔离 `ui-preview` 构建；两者均在暗色主题、约 `1282x912` 窗口中先观察参考页，再执行对应的 Ramag 页操作。进程页额外在两边输入相同的 `system-pulse` 搜索条件并回读过滤结果。表中的“通过”只表示本行列出的功能和状态已被真实窗口复现，不表示整页视觉已经完成。

| 页签 | System Pulse 真实观察 | Ramag 真实观察 | 本轮结论 |
|---|---|---|---|
| Summary | 首排为 CPU/Clock/Temp/GPU 量表、CPU overview、Top CPU processes；第二排为全宽 Memory utilization；下方显示 Disks、Network、Energy、GPU、Thermals 活动卡 | 已按同样的信息顺序显示 CPU/Clock/Temp/GPU、CPU overview、Top CPU processes、全宽 Memory，以及磁盘/网络活动卡；缺失温度和缓存字段保留不可用原因 | `03A/03B` 功能和首屏组合通过；缓存字段的长原因文本、宽屏五卡同排仍是视觉差距 |
| CPU | Overall meter、24 个逻辑处理器图表、Uptime、进程数和 CPU package power | CPU 利用率、频率、进程/线程、逐核心传感器卡和历史图 | 功能来源覆盖；图表网格和底部摘要布局仍未一一对齐 |
| Memory | Overall meter、全宽 Memory utilization、RAM used/total/available/free 与 Swap used/total | RAM used/total/available/free、Swap、cache/buffers/other 和 page faults 卡片，带不可用原因 | 数据字段覆盖；参考的单图加底部六项布局仍是后续 UI 差距 |
| GPU | 选定 NVIDIA GeForce RTX 3060，使用历史图、VRAM 量表、共享内存、温度、功率、时钟和风扇字段 | 同一 GPU 的 usage、VRAM、共享内存、温度、功率、时钟和风扇卡片 | 传感器功能通过；选中标题、量表与卡片布局仍需统一 |
| Disks | 文件系统下拉选择器、Filesystem used 量表、读写合并趋势和容量说明；Computer Use 已验证 `C:\` 切换 `F:\` 后保持 | 磁盘页签选择器、Filesystem used/read/write/IOPS/latency 卡片；Summary 保持所选磁盘 | 选择、容量和读写来源通过；下拉选择器与合并趋势布局待对齐 |
| Network | 接口下拉选择器、RX/TX 合并趋势、累计 RX/TX | 接口页签、RX/RX total/TX/TX total/TCP 卡片；Summary 保持所选接口 | 接口和双向数据通过；选择器、趋势和累计值位置待对齐 |
| Energy | 通过右上设备选择器切换到 `CPU · CPU package power`；主读数、量表和历史图使用该传感器，下面保留 CPU package 与 NVIDIA GPU measured power channels | 通过 Energy 页下拉菜单在 GPU power 与 CPU package power 间切换；主读数、量表、历史图、来源/范围/单位和 measured channels 均随稳定 ID 更新 | `A-PULSE-GAP-06-Energy` 真实窗口功能通过；Ramag 保留工作台壳层和紧凑标题，精确字体/卡片排布留在视觉细节切片 |
| Thermals | 选定 GPU temperature，突出最热当前传感器、温度量表、历史图和传感器卡；提供 CPU temperature 授权按钮 | CPU temperature、GPU temperature、CPU package temperature 三张卡；选择器、动态摄氏量程、不可用字段原因和授权入口均可见 | `A-PULSE-GAP-06-Thermals` 真实窗口功能通过；不同平台硬件精度和授权成功路径另行验证 |
| Processes | 搜索框、End task/Force quit、PID/Name/CPU(one core)/Memory/Read I/O/Write I/O/User 列；搜索 `system-pulse` 后保留两个进程 | 搜索框、按 CPU 排序选择器、PID/Name/User/CPU/Memory/Read/Write/操作列；相同搜索后保留两个进程 | 搜索和字段功能通过；操作列更丰富，排序和列顺序仍有视觉/交互差异；键盘行导航按要求后置 |
| Settings | Appearance（主题、界面字体、数字字体）、Sampling（0.5/1/2/5s）和 Presets（内置预设、命名输入、保存） | 主题、字号、滚动条、界面字体、数值字体、0.5/1/2/5s 采样、采集状态和固定监控预设管理 | `A-PULSE-GAP-05`、`09`、`10` 已通过目标测试和限定真实窗口对照；跨页面可停靠布局仍不适用 |

本轮十页验收的结果是：`03A/03B` 的 Summary 交付范围、`06-Energy` 的主传感器选择、`06-Thermals` 的最热/选中温度组合、`09-Settings-Appearance` 的字体选择、`10-Settings-Sampling` 的四档采样和 `05` 的固定监控预设管理已通过目标测试与限定真实窗口对照，CPU、Memory、GPU、Disks、Network、Processes、Thermals 的主要数据来源和状态可见；各详细页的精确布局、跨平台硬件精度和键盘增强仍按矩阵单独记录。后续每个切片继续执行“参考窗口操作 -> Ramag 同流程 -> 记录结果”的顺序，不能以静态截图、旧测试或单纯编译成功替代该证据。

### 2026-10-02 `A-PULSE-GAP-06-Energy` 单传感器选择验收

本切片先在本机 `F:/project/system-pulse` 的真实窗口打开 Energy 页，使用 Computer Use 打开右上设备选择器，选择 `CPU · CPU package power`；参考窗口回读主读数 `29.3 W`、主图量程 `0.0–42.1 W`，对应 CPU package measured channel 同步显示 `29.3 W`，GPU channel 保留独立的 `14.6 W`。采样值随时间变化，数值相等只作为同一帧的内部一致性检查，不作为两个进程跨时刻的硬件精度比较。

Ramag 使用刚构建的 `cargo build --locked -p ramag-bin --example ui-preview` 隔离窗口，在同一 Energy 页通过 Computer Use 依次选择 GPU power、再选择 CPU package power；窗口回读选中名称、来源/范围/单位、黄色量表、历史图和 measured channels，选择按钮在两次操作后恢复为 `CPU package power`。Ramag 的工作台壳层和紧凑标题保持不变，主内容组合与参考程序对应；实测值因两个窗口的采样时间不同而变化。

实现以稳定传感器 ID 保存 `energy` 选择，CPU package power 是无偏好时的默认来源；保存 ID 消失时保留不可用原因，不自动改选其他传感器。采集快照过期时主值和量表停止显示为实时数据，历史曲线继续保留上下文；零值仍按有效 `0 W` 显示。

验证命令：`cargo test --locked -p ramag-tool-system --lib -- --test-threads=1`（45 项通过）、`cargo build --locked -p ramag-bin --example ui-preview`（通过）、`cargo fmt --all -- --check`、workspace Clippy、源码尺寸、`git diff --check` 和修改文本 LF 检查。纯本地监控/UI 切片不使用 Docker；没有宣称 GPU/温度/跨平台硬件精度完成。下一项进入 Settings 完整工作区预设职责拆分，Thermals 单传感器另行提交。

### 2026-10-02 `A-PULSE-GAP-09-Settings-Appearance` 真实窗口验收

System Pulse 使用固定提交 `f1be5d51d24c21fa8c740be79200bdda3df3a00c` 的真实窗口，在 Settings -> Appearance 观察 Theme、Interface font、Numeric font 和 Typography preview；选择 `IBM Plex Sans` 与 `IBM Plex Mono` 后，参考窗口即时更新按钮状态和预览。Ramag 使用刚构建的 `cargo build --locked -p ramag-bin --example ui-preview` 隔离窗口，打开系统设置后通过 Computer Use 选择同样的两种字体，回读“系统设置：已保存”、按钮选中态和数字预览变化，再切换浅色主题，字体选择保持不变。两边均未把采样或预设操作混入本切片；Ramag 的中文工作台壳层和响应式设置卡保持现有产品约定。

Headless 覆盖 `360x640`、`1024x768`、`1440x900` 的设置控件边界、明暗主题、序列化默认值、未知字体拒绝、主题切换保持和保存状态；真实窗口证据覆盖 1024x768 Ramag 与参考 Settings 页面。Ramag 预览进程已停止，未读取用户连接或凭据；System Pulse 参考进程保持运行。下一项进入 Settings 完整工作区预设职责拆分，Thermals 单传感器继续独立验收。

### 2026-10-02 `A-PULSE-GAP-10-Settings-Sampling` 真实窗口验收

System Pulse 参考窗口在 Settings -> Sampling 通过 Computer Use 选择 `0.5 s`，回读大号周期值 `0.5`、选中按钮和底部状态 `0.5 s update`。Ramag 隔离预览先进入系统监控 Settings，选择 `0.5s`，回读右上角 `刷新 0.5s`，再返回 Summary，选中页签和实时读数继续更新，证明设置页和监控页共用同一周期状态。

实现新增有界 `HalfSecond` 枚举，贯通 `MonitorSettings` JSON、全局设置页、系统监控 Settings、采样服务和启动恢复；旧的 1/2/5 秒 JSON 保持兼容，采样切换不修改主题、字体、设备选择、传感器显隐或历史样本。Headless 验证为 `ramag-ui` 127 项、`ramag-tool-system` 47 项，另通过 `cargo build --locked -p ramag-bin --example ui-preview`；本地 UI 切片不使用 Docker。Ramag 隔离预览已停止，System Pulse 参考进程保持运行。下一项只处理完整工作区预设的快照字段与损坏数据保护，不能把连接或凭据写入预设。

### 2026-10-02 `A-PULSE-GAP-05` 固定监控预设真实窗口验收

System Pulse 参考窗口在 Settings -> Presets 显示 Default、Minimal、GPU Focus、Developer 四个内置入口，以及命名输入和保存当前工作区按钮。Ramag 使用刚构建的隔离预览，在“设置 -> 系统监控”回读同名四个内置按钮；点击 Developer 后刷新频率切换为 `0.5 秒` 并显示“已应用内置预设”，点击 Default 后恢复 `1 秒`。Ramag 的固定十页结构没有可安全迁移的停靠布局，因此内置入口只改变采样或清空默认展示偏好，并明确提示固定页面结构保持不变。

随后在两边按同一写操作顺序核对命名预设：Ramag 输入 `Work` 保存，回读列表行和绿色保存状态；点击覆盖后出现确认区域，取消不改变行；点击删除后出现确认区域，取消保留行，确认删除后列表清空并显示结果状态。命名预设使用版本化 `MonitorPresetLibrary`，限制名称、数量和序列化字节数，内置名称不能被自定义预设占用；应用预设只写采样和设备/传感器展示偏好，不写连接、凭据、历史样本或全局外观。

Headless 覆盖库解析/序列化、未知版本、内置名称冲突、创建/应用/覆盖/重命名/删除和确认取消；`cargo test --locked -p ramag-ui monitor_preset --lib -- --test-threads=1` 为 4 项通过，`cargo build --locked -p ramag-bin --example ui-preview` 通过。Computer Use 证据覆盖 System Pulse `1282x912` Settings 与 Ramag `1024x768` 隔离窗口的入口、采样状态、列表和确认流程；隔离预览不证明生产 redb 重启后的实际回读，Ramag 固定页面不能宣称等价于 System Pulse 的可停靠工作区布局。纯本地 UI/偏好切片不使用 Docker；System Pulse 参考进程继续运行。

### 2026-10-02 `A-PULSE-GAP-06-Thermals` 温度选择和实时范围验收

System Pulse 使用固定提交 `f1be5d51d24c21fa8c740be79200bdda3df3a00c` 的 `1282x912` 暗色真实窗口。Computer Use 在 Thermals 页回读 `NVIDIA GeForce RTX 3060 · GPU temperature` 选择器、`Hottest current sensor` 独立读数、温度量表、选中传感器历史图、`Temperature sensors` 卡片和 `Enable CPU temperatures...` 授权入口；温度纵轴按当前历史样本显示约 `40.0–42.0 °C`，没有把摄氏温度固定成 `0–100`。

Ramag 使用 `cargo build --locked -p ramag-bin --example ui-preview` 后的隔离真实窗口，按同一顺序进入 Thermals。页面现在显示设备名和传感器名的选择器，菜单带勾选状态和 `320px` 滚动上限；切换到 `CPU · CPU temperature` 后主传感器保持该稳定 ID 并显示“不可用”及采集原因，同时最热摘要仍显示 GPU 当前温度。点击一次 CPU 温度授权按钮后回读“已请求启用 CPU 温度采集”和按钮状态，没有出现重复帮助程序窗口；CPU 温度仍因本机硬件未暴露而保持不可用。温度量表、主历史图和传感器卡片使用同一有限历史范围，当前零值、负值、非有限值、过期和失败读数不会伪装成实时数值。

实现以 `selected_sensors["thermals"]` 保存选择；无保存 ID 时默认最高的有限 `Current` 摄氏读数，并以稳定 ID 处理并列；保存 ID 消失时保留不可用状态，不静默换选。共享图表增加显式物理范围和有限值过滤，CPU 授权请求增加操作中互斥，传感器菜单支持滚动。`ramag-tool-system` 54 项、`ramag-ui` 131 项、`cargo build --locked -p ramag-bin --example ui-preview`、fmt 和 workspace Clippy `-D warnings` 通过；本地监控/UI 切片不使用 Docker。Headless 覆盖多传感器、零/负值、NaN/Infinity、非 Current 状态、并列和缺失选择；Computer Use 证据覆盖参考窗口和 Ramag 的真实页面、菜单切换、不可用状态及授权入口。不同采样时刻的硬件读数不作数值相等验收，多平台硬件精度和 CPU 授权成功路径仍需在对应主机单独验证。

### 2026-10-02 `A-PULSE-GAP-10` 采样读数视图补齐

System Pulse Settings 的 Sampling 分区实际显示当前周期的大号数值、`seconds between readings` 单位、较短周期的 CPU 影响说明和自动保存提示。Ramag 原系统监控 Settings 只有 `0.5s/1s/2s/5s` 四个按钮，当前周期只能从页头刷新文字间接判断。本补充切片将这四项信息放入同一采样面板，读数直接来自 `SystemMonitor::refresh_interval()`，并在 `360x640`、`1024x768`、`1440x900` 和明暗主题下检查按钮与说明仍可见。采样更新、持久化和跨页面同步不在本次代码范围内。

### 2026-10-02 Summary 逐区域视觉对齐设计

用户要求以本地 System Pulse 源码和真实运行结果逐页对齐，进程键盘增强后置。本轮按已确认的 `A-PULSE-GAP-03` 范围，先完成截图标出的 Summary 区域，再进入详细页。每项通过匹配测试、Computer Use、最终 fmt/Clippy 后独立提交推送。

1. `03D-Summary-Visual`：采用参考程序的中性背景、领域配色、Michroma 标题和数字字体；首排量表固定 245px，CPU 图与进程表按剩余宽度分配，窄窗口换行。量表改为 30 段，温度取全部真实温度传感器中最高的当前有限值；逻辑核心信息置于 CPU 概览下，进程表显示列头、总数和八行交替背景。内存增加分段量表；Summary 趋势使用顶部左对齐量程、顶部右侧历史时长、细密网格、缺口独立面积填充和末值标记，消除宽轴标签预留。磁盘、网络、GPU 卡片的标题、主值、设备、趋势和图例与参考顺序一致。
2. `03E-Summary-Energy`：补上第三张 Energy 卡，沿用详细页的稳定传感器选择，保留零值、缺失、失败和过期状态，不能加总不同范围的功率。
3. `03F-Summary-Thermals`：补齐温度卡和三加二/宽屏五卡布局，采用真实所选温度来源和动态摄氏范围。

验收使用明暗主题及 `360x640`、`1024x768`、`1440x900` headless 区域/交互检查，并通过 Computer Use 分别观察参考程序和刚构建的 Ramag 真实窗口，核对 Summary 首排、内存、底部卡片及详情入口。保留原生 `960x640` 最小窗口尺寸；小尺寸 headless 只检查布局退化，不代表原生允许缩到该尺寸。启动新 System Pulse 前关闭旧实例并核对数量。Docker 服务、镜像、端口和清理均不适用。
