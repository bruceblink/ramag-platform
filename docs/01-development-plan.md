# Ramag Platform 最新开发计划

> 状态：现行计划
> 更新日期：2026-10-17
> 本轮起点：`main` 与 `origin/main` 均为 `0e55b681`；`v0.5.0` 已发布。工具显示设置、入口增删和 S3 兼容驱动已完成；当前先优化首页与侧边栏工具管理交互，对象存储账号列表验收随后恢复。
> 当前主线：收口已有桌面工具的 UI 差异和功能状态；阶段 B 继续维护原生工作区；阶段 C 协同画布后置

本文件只描述当前可执行的开发顺序。旧的逐切片验收记录保存在
[`01-development-plan-history.md`](01-development-plan-history.md)，不再作为新的排队入口。

## 1. 目标与边界

当前版本的目标是让已有工具在真实工作流程中保持可读、可操作、可恢复。优先修复布局、字体、状态反馈、滚动、筛选、排序、删除/撤销和加载失败问题。已有数据库、API、Kafka、MQTT、SSH/SFTP、容器、Git、对象存储、剪贴板和 System Pulse 能力继续在原生 GPUI 工作区中演进。

本轮新增范围包括工具入口显示管理、首页和侧边栏的入口添加/移除，以及 S3 兼容对象存储账号，不扩大其他工具能力：

- 首页和侧边栏各自可以添加或移除已注册工具入口。移除只影响当前表面；工具注册、另一表面入口、配置和数据保持不变。
- 首页和侧边栏使用统一的可搜索管理面板。每个工具行直接提供当前表面的添加或移除操作。设置页继续提供完整显示状态管理。
- 不实现工具代码卸载、删除插件文件或第三方动态插件市场。

- 不启动原生协同画布、生产 Relay、Relay B4 或动态插件市场。
- 不使用 WebView 或嵌入式浏览器替代原生工具工作区。
- 不把静态截图、headless 渲染或成功启动写成真实窗口交互通过。
- 不把本机硬件、跨平台、发布性能或 GitHub CI 的未验证结果写成已完成。

## 2. 已完成基线

| 范围 | 当前事实 | 证据 |
|---|---|---|
| 版本与主线 | `v0.5.0` 标签已存在；当前 `main` 与远端一致 | `git log -1`、`git status --short --branch` |
| 首页与工具入口 | 第一方工具首页层级、入口和剪贴板状态已统一 | `c4c20d06`、`docs/08-v0.5.0-ui-release-plan.md` |
| 剪贴板 | 删除后的搜索结果、详情、撤销恢复和失败通知已补齐测试路径 | `1dfb4ab3`、`docs/09-post-0.5.0-ui-fixes-plan.md` |
| System Pulse 详细页 | Disks/Network 已按主量表、趋势和统计区分组，并保留设备身份与缺失原因 | `0ede5b9e`、`docs/10-system-disks-network-plan.md` |
| 对象存储 | 账号列表已按窗口宽度切换行密度，窄窗口保留名称、元数据和操作可达 | `09c25b08`、`crates/ramag-tool-object-storage/src/views/render_test/account_tests.rs` |
| 系统监控 | 采集替换、十页页面、进程排序/详情、Energy/Thermals 来源状态和 Settings 收口已完成主要代码切片 | [`03-system-pulse-adoption-plan.md`](03-system-pulse-adoption-plan.md)、[`04-system-pulse-gap-matrix.md`](04-system-pulse-gap-matrix.md) |
| 首页与侧边栏工具入口 | 统一管理面板已替换原来的分离操作；搜索、连续添加和移除已通过测试与真实窗口验收 | [`17-tool-surface-manager-ux-plan.md`](17-tool-surface-manager-ux-plan.md) |

本轮 DBClient 切片使用隔离的 `ui-preview` 检查生产 Shell 和 DBClient 视图。真实窗口交互与 headless 检查分别记录。该入口不读取用户连接，也不会连接数据库。它不代表完整 `ramag.exe` 和其他工具已经完成原生窗口验收。

## 3. 当前队列

同一时间只允许一个切片处于“开发中”。每个切片完成目标测试、匹配的 UI 验收和质量门后独立提交并推送 `main`。

| 顺序 | 切片 | 状态 | 完成条件 |
|---|---|---|---|
| 1 | `A-POST-0.5-TOOL-VISIBILITY-001` 首页与侧边栏工具显示管理 | 已完成；目标测试、严格 Clippy、程序构建和真实窗口交互通过 | 在设置页为每个已注册工具分别控制首页和侧边栏入口；设置持久化，切换后两个表面即时更新；隐藏入口不卸载工具或删除数据。详见 [专项计划](12-tool-visibility-plan.md)。 |
| 2 | `A-POST-0.5-DBCLIENT-001` 数据源列表差异复核 | 已完成；全工作区 Docker 测试 2702 passed、0 failed、19 ignored | 无匹配搜索、长错误、重试、账号列、行操作和多尺寸均有对应验证；详见 [本轮计划](11-dbclient-list-state-plan.md)。 |
| 3 | `A-POST-0.5-CLIPBOARD-001` 剪贴板删除/撤销回归收口 | 代码、测试和验收记录已在 `1dfb4ab3` 与 [专项计划](09-post-0.5.0-ui-fixes-plan.md) 完成 | 删除后当前搜索和详情失效；撤销重新查询并恢复；恢复失败保留原记录和通知。 |
| 4 | `A-POST-0.5-S3-COMPAT-001` S3 兼容账号与本机数据源 | 已完成；目标测试、全工作区检查、RustFS 实际读写和真实窗口账号表单均通过 | 增加 S3 兼容账号和安全 Endpoint 校验；独立本机 RustFS 容器通过真实列表和传输验证。详见 [专项计划](13-object-storage-s3-compatibility-plan.md)。 |
| 5 | `A-POST-0.5-TOOL-SURFACE-CRUD-001` 首页与侧边栏添加/移除工具入口 | 已完成；交互已由下一切片替换 | 基础入口增删能力已通过；早期管理面板记录见 [专项计划](14-tool-surface-management-plan.md)。 |
| 6 | `A-POST-0.5-TOOL-SURFACE-MANAGER-001` 首页与侧边栏统一工具管理交互 | 已完成；143 项 UI 单测、严格 Clippy、完整程序构建和真实 Windows 窗口添加/移除通过 | 一个面板列出当前显示和未显示的工具；每行直接添加或移除；只修改当前表面。详见 [专项计划](17-tool-surface-manager-ux-plan.md)。 |
| 7 | `A-POST-0.5-OBJECT-001` 对象存储列表与真实数据源验收 | 待恢复；已完成 RustFS live 测试和响应式 headless 布局检查 | 恢复后完成隔离数据下的完整程序验收：账号持久化、RustFS 对象列表、筛选/排序后的稳定 ID 以及 360/640/768/1024/1440 宽度检查。详见 [专项计划](16-object-storage-list-acceptance-plan.md)。 |
| 8 | `A-PULSE-GAP-03-DN-UI-001` Disks/Network 真实窗口核验 | 代码与 headless 已完成，CUA 阻塞 | 在参考程序和 Ramag 中依次核对主量表、读写/收发趋势、统计区、设备重排、缺失身份和页面滚动；CUA 恢复前保留未完成。 |
| 9 | `A-POST-0.5-LIST-ACCEPTANCE-001` 其余列表型工作区完整程序核验 | 后续 | 依次核对容器、VCS、插件的标题、字体、背景、滚动、空/加载/失败状态和主要操作；已拆出的 DBClient、对象存储和剪贴板切片不重复排队。原生窗口可用时使用 Computer Use；不可用时明确列出未覆盖操作。 |
| 10 | `A-POST-0.5-CONNECTION-ACCEPTANCE-001` 编辑/连接型工作区核验 | 后续 | 按 SSH/SFTP、API、Kafka、MQTT 顺序检查连接、取消、保存、错误恢复和结果上下文；只有协议行为变化时才启动本机 Docker。 |
| 11 | `A-POST-0.5-REGRESSION-001` 全局回归与文档收口 | 后续 | 重新构建完整程序，复核首页往返、主题、全局 Settings、共享组件和本轮受影响工具；更新对应专项文档、测试报告和未完成项。 |

### 3.1 后置项目

以下项目不阻挡前面的功能修复和逐工具 UI 核验：

- `A-PULSE-MOTION-001`：减少动画偏好、可中断过渡和停止空闲重绘。
- `A-PULSE-PROFILING-001`：发布构建的点击反馈、活动帧和资源上限采样。
- `A-UI-REAL`：在 Computer Use 恢复后补齐需要真实 Windows 窗口的鼠标、键盘、滚动、保存和错误恢复证据。
- Linux/macOS 原生窗口、真实硬件精度、系统凭据的非 Windows 后端和正式签名包验证。

后置项目必须使用独立验收条件和证据，不能用本机 headless 结果替代。

## 4. 统一验收门槛

### 4.1 设计确认

每个切片开始前记录问题证据、用户流程、改动范围、不做事项、回滚边界和验收条件。修改公共 `ramag-ui` 组件时，同时列出实际消费者和回归范围。

### 4.2 UI 条件

- 明暗主题、标准字号 16px 与最大字号 18px，检查 `360x640`、`1024x768`、`1440x900`；`360x640` 为 headless 防御检查，原生窗口按工具最小宽度执行。
- 检查真实控件命中、键盘焦点、滚动范围、长名称、空/加载/成功/失败/取消状态和主要操作可达性。
- 记录 headless、Computer Use、系统截图和服务回放的证据边界。任一证据缺失时，状态写为“未完成”或“部分完成”。

### 4.3 Rust 与服务验证

受影响 Rust crate 至少执行目标测试、格式、严格 Clippy、源码尺寸和差异检查；公共组件或存储改动追加 workspace 测试和完整 `ramag-bin` 构建。

```powershell
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','<受影响crate>','--lib','--','--test-threads=1')
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
& .\scripts\windows\check-source-size.ps1
git diff --check
cargo build --locked -p ramag-bin
```

文本文件使用 LF。数据库和协议集成只使用本机 Docker；PostgreSQL 使用 17+，MySQL 使用 8.4+，并记录镜像、端口、健康状态、启动结果和清理结果。纯 UI、监控和本地偏好切片标记 Docker 为“不适用”。

### 4.4 交付规则

验证必须针对最终文件执行。每个独立功能使用一个英文 Conventional Commit，并立即推送当前 `main`。不提交未说明的无关修改。任何后续源码修改都必须重新运行受影响检查。

## 5. 证据记录模板

每个切片在对应专项文档中记录：

1. 切片 ID、日期、提交、分支和推送结果。
2. 问题证据、设计、范围、不做事项和回滚边界。
3. 目标测试、完整命令、退出码、测试环境和测试数据。
4. UI 尺寸、主题、字号、操作步骤及 headless/真实窗口结果。
5. Docker 服务与清理结果，或明确写“不适用”。
6. 未完成项、证据限制和下一项依赖。

专项入口：[`02-development-roadmap.md`](02-development-roadmap.md)、[`03-system-pulse-adoption-plan.md`](03-system-pulse-adoption-plan.md)、[`04-system-pulse-gap-matrix.md`](04-system-pulse-gap-matrix.md)、[`07-ui-acceptance-standard.md`](07-ui-acceptance-standard.md)、[`09-post-0.5.0-ui-fixes-plan.md`](09-post-0.5.0-ui-fixes-plan.md)、[`10-system-disks-network-plan.md`](10-system-disks-network-plan.md)、[`12-tool-visibility-plan.md`](12-tool-visibility-plan.md)、[`13-object-storage-s3-compatibility-plan.md`](13-object-storage-s3-compatibility-plan.md)、[`15-tool-surface-quick-actions-plan.md`](15-tool-surface-quick-actions-plan.md)、[`16-object-storage-list-acceptance-plan.md`](16-object-storage-list-acceptance-plan.md)、[`17-tool-surface-manager-ux-plan.md`](17-tool-surface-manager-ux-plan.md)。

## 6. 回滚边界

普通 UI 或功能切片只回滚对应渲染模块、状态逻辑、目标测试和专项文档；公共组件改动必须同时回归其消费者。不得回滚 `v0.5.0` 标签、已发布资产、用户连接、用户剪贴板历史或全局 Codex 配置。
