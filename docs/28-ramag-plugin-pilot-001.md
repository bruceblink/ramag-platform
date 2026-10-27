# RAMAG-PLUGIN-PILOT-001：系统监控第一方插件试点

> 状态：已完成静态第一方插件试点，代码、Headless GPUI 和 Windows 真实窗口验收均已通过。
>
> 日期：2026-10-27。
>
> 试点对象：`ramag-tool-system`。本文件记录试点范围、实现和验收边界。试点不改变系统采集、配置、进程控制或用户数据。

## 选择理由

`ramag-tool-system` 是当前最小风险的第一方试点候选：

- 只读取本机性能、磁盘、网络、GPU、温度和进程信息。
- 不读取用户凭据，不连接数据库、消息服务或远程 API。
- 不写入业务数据，不修改 Git、文件或连接配置。
- 已有独立 `ramag-tool-system` crate、`SystemTool` 元数据和 `create_system_view` GPUI 视图。
- 已有系统监控的 Headless GPUI、构建和真实窗口证据，可作为迁移前后的对照。

试点使用 `PluginDescriptor` 和 `PluginContext` 的最小静态边界。系统采集仍由现有 `SystemTool` 和宿主专用 GPUI 工作台负责，不新增插件能力。

## 试点范围

### 允许修改

1. 在 `ramag-bin` 组合根中把 `SystemTool` 的静态适配登记为明确的第一方插件描述。
2. 保持 `system` 工具 ID、显示名称、图标、首页/侧栏可见性和现有工具顺序。
3. 保持 `create_system_view`、采样周期、设备选择、进程列表和已有监控设置路径。
4. 增加插件目录中可读的版本、入口、平台和数据处理说明。
5. 增加针对试点描述、注册、初始化、关闭、入口目录和失败隔离的测试。

### 明确不修改

- 不新增系统采集能力，不改变采样算法、权限提示、帮助程序或系统调用。
- 不把 GPU、温度、进程终止或系统控制操作扩展为插件权限。
- 不引入动态库、WASM 桌面运行时、外部插件进程、WebView 或远程市场。
- 不迁移数据库、API、Git、SSH、容器、剪贴板或对象存储工具。
- 不删除或重写用户配置、历史数据、凭据、监控预设和工具布局偏好。

## 预期接口形状

试点使用静态 Rust 插件，不创建新的外部 ABI。初步描述如下，最终字段以设计评审结果为准：

| 字段 | 候选值 | 约束 |
|---|---|---|
| 插件 ID | `system` | 与现有工具入口保持兼容 |
| API 版本 | `1.0` | 使用当前宿主版本检查 |
| 主入口 ID | `system` | 保持 Activity Bar 和工具布局 ID |
| 平台 | 桌面原生 | 不声明 Web/WASM，系统指标依赖桌面采集器 |
| 能力 | `ui.entry` | 不声明网络、文件、数据库或系统控制能力 |
| 设置 | 无新增插件设置 | 继续使用既有系统设置路径，直到设置迁移方案评审通过 |
| 视图 | `create_system_view` | 由宿主按需创建，插件不持有 `Shell` 或 GPUI 根对象 |
| 数据处理 | 本机指标、进程和设备状态 | 目录说明不包含凭据、原始业务数据或远程传输 |

如果现有系统视图无法在不扩大插件权限的情况下接入，试点应停在适配层，不把系统采集器强行搬入插件上下文。

## 实现记录与验收结果

2026-10-27 已在 `crates/ramag-bin/src/composition.rs` 中增加系统监控专用注册函数。该函数为现有 `SystemTool` 创建明确的 `system` 插件描述，再通过 `StaticPluginAdapter` 注册；`SystemTool` 实例、`create_system_view`、工具顺序、设置路径和采样行为保持不变。描述固定为 API `1.0`、主入口 `system`、桌面原生平台、本机指标数据处理和唯一的 `ui.entry` 能力，插件设置为空。

适配层在描述构造或注册失败时记录有界诊断并跳过该插件。初始化、关闭、卸载、入口目录和失败隔离继续由现有 `StaticPluginHost`、`ToolRegistry` 和生命周期测试覆盖。没有新增动态库、WASM 桌面运行时、外部进程、WebView 或网络能力。

Headless 与静态验证结果如下：

- `cargo test --locked -p ramag-app`：283 项单元测试、2 项应用测试和 6 项数据库测试、9 项传输测试通过。
- `cargo test --locked -p ramag-tool-system`：114 项测试通过。
- `cargo test --locked -p ramag-ui`：163 项测试通过。
- `cargo test --locked -p ramag-bin`：17 项测试通过，包含稳定描述和工具注册顺序检查。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --locked -p ramag-bin` 和 `git diff --check` 通过。

Computer Use 真实窗口在 `1202x774` 隔离数据目录中完成检查。设置 → 插件页面显示 `system` 插件、`system` 入口、API `1.0`、桌面原生平台、`ui.entry` 能力和“已审核”状态；首页仍显示“系统监控”卡片；打开后标题为“系统监控”，九个监控页签、实时内存指标和进程采样状态可见。验收未连接外部服务、未写入业务数据、未修改用户配置，窗口已关闭。

## 验收条件

### 功能与兼容

- 首页和 Activity Bar 仍显示“系统监控”，入口 ID 和工具顺序不变。
- 设置页、系统监控首页、GPU、磁盘、网络、温度和进程页面保持现有导航和数据来源。
- 插件初始化失败时系统入口被隔离，其他工具继续可用；关闭失败不阻塞其他插件关闭。
- 插件目录显示 `system` 的 API 版本、桌面原生平台、`ui.entry` 能力和本机指标数据处理说明。

### 安全与边界

- 插件没有网络、任意文件、子进程、数据库或秘密存储能力。
- 试点不把系统指标、进程列表或路径写入插件普通设置、日志或目录之外的持久化位置。
- 失败信息保持有界，不包含用户凭据、连接配置或完整环境变量。

### 验证范围

已执行：

```text
cargo fmt --all -- --check
cargo test --locked -p ramag-app
cargo test --locked -p ramag-tool-system
cargo test --locked -p ramag-ui
cargo clippy --workspace --all-targets -- -D warnings
cargo build --locked -p ramag-bin
git diff --check
```

UI 验收分别记录 Headless GPUI 和 Computer Use 真实窗口证据。真实窗口检查了入口、导航和目录；失败隔离、生命周期、设置、权限和卸载由现有宿主专项测试覆盖。启动成功本身不作为试点完成依据。

## 实施顺序

1. 冻结 `27-ramag-plugin-api-design-001.md` 中静态 Rust/GPUI 第一阶段边界和本候选范围。
2. 固定 `system` 插件描述、入口 ID、能力和目录字段。
3. 编写适配层和最小描述测试，保持现有 `SystemTool` 实例和视图创建路径。
4. 运行受影响测试、严格 Clippy、构建和真实窗口验收。
5. 单独提交并推送试点；记录迁移前后工具顺序、设置和监控数据边界。

## 回滚边界

只回滚试点插件描述、组合根适配、目录字段和对应测试；保留现有 `SystemTool`、`create_system_view`、采集器、设置、工具布局和用户数据。试点失败时恢复原静态适配即可，不删除配置或历史样本。

## 后续边界

以下问题保留给后续迁移评审，不阻塞本次静态试点交付：

1. `system` 是否作为第一方插件 ID 和入口 ID 保持不变。
2. 系统采集设置是否继续由宿主保存，还是另行设计插件设置命名空间。
3. 现有系统视图是否继续由宿主注册为专用工作台，并只把元数据登记为插件贡献点。

