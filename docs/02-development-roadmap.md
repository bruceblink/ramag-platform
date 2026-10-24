# Ramag Platform 主线路线：原生核心收缩与双端协议

> 状态：现行路线
> 更新日期：2026-10-24
> 执行入口：[01-development-plan.md](01-development-plan.md)
> 目标：把 Ramag 从“重复的全功能工具箱”收缩为原生核心能力和 GPUI 客户端

## 1. 路线判断

it-tools-template 已经具备 Vue、Web/PWA、Electron、插件和桌面数据库/系统监视能力。
继续在 GPUI 中重复实现同一批通用工具，会同时增加 UI、协议、测试、发布和维护成本。
因此，今后的路线以“一个产品所有者、一个能力协议、两个可选客户端”为准：

    Web / PWA ──> it-tools-template 产品层
    Electron ───> it-tools-template 产品层 ──> Electron 平台适配
    GPUI ────────> ramag-engine 原生核心
                                └────────────> ramag-protocol

it-tools-template 拥有用户可见的通用工具和 Electron 工作台。
ramag-platform 拥有原生核心服务、GPUI 客户端和共享协议。
Web 只使用浏览器或 WASM 可用的部分。Electron 和 GPUI 对同一原生能力遵循同一协议。

## 2. 功能归属路线

| 路线 | it-tools-template 承担 | ramag-platform 只保留 |
|---|---|---|
| 通用工具 | 格式转换、编码、加密、文本、数据、网络、图片、JSON/YAML/XML 等 | 不保留重复入口 |
| 数据与服务工作台 | 数据库、Redis、MongoDB、API、Kafka、MQTT | 不保留重复 GPUI 工作台 |
| 开发工作台 | Git、容器、对象存储、普通系统监控、剪贴板 | 不保留重复产品页面 |
| 原生连接 | Electron 页面、权限提示、历史和发布 | SSH/SFTP、PTY、隧道和会话安全 |
| 本地安全 | 负责应用配置和凭据；原生核心只在需要时调用 | 仅在 Electron 无法满足共享保护边界时保留引擎级存储 |
| 硬件诊断 | 普通采集和图表 | 仅保留有明确精度或资源优势的采集器 |
| 协作与市场 | 暂缓或另行产品设计 | 不进入当前主线 |

## 3. 阶段路线

### 阶段 A：边界冻结

目标是停止重复投资，建立逐项迁移表。为每个当前工具记录：

- 用户入口和主要流程。
- it-tools-template 的目标入口。
- 是否需要 ramag-engine。
- 数据、凭据、权限、取消和资源上限。
- 替代完成条件、兼容期和回滚版本。

阶段 A 完成前，不新增被标记为“迁移”的 GPUI 功能。

### 阶段 B：产品能力转移

由 it-tools-template 按优先级接收数据库、API、Git、消息系统、容器、对象存储、
剪贴板和普通系统监控。每一项必须先有 Electron 可用流程，再从 Ramag 移除导航和默认构建。

优先顺序：

1. 数据库管理器和普通系统监控。
2. API、Git 和对象存储。
3. Redis、MongoDB、Kafka、MQTT。
4. 容器管理、剪贴板和更新流程。
5. 纯计算工具和 JSON Path 入口。

顺序按重复度和迁移收益排序，不表示协议重要性排序。

### 阶段 C：原生核心抽取

把 SSH/SFTP/PTY、端口转发、加密本地存储和必要的低层采集从 GPUI 视图中抽成
ramag-engine。引擎必须可以在无窗口环境运行，并提供：

- 明确的能力注册和版本协商。
- 连接、流式输出、分页、取消和超时。
- 受控凭据访问和日志脱敏。
- Electron 与 GPUI 的相同协议测试。

### 阶段 D：双客户端接入

GPUI 只实现原生核心工作区。Electron 使用 Vue 页面和主进程适配器调用同一引擎。
两端的验收重点是操作结果和安全行为一致，不复制内部 UI 组件。

### 阶段 E：删除重复实现

完成替代验收后，删除 Ramag 中对应的 ramag-tool-*、基础设施依赖、入口注册、
专项排期和过时截图。保留历史文档，但明确标记为历史。删除前检查：

- 新入口覆盖原流程。
- 旧配置可读取或有迁移提示。
- 用户数据和凭据没有被删除。
- workspace、安装包和文档不再引用旧工具。

## 4. 当前队列

| 优先级 | ID | 状态 | 下一步 |
|---|---|---|---|
| P0 | BOUNDARY-001 | 新路线起点 | 完成功能矩阵和所有者确认 |
| P1 | TRANSFER-DB-SYSTEM-001 | 待开始 | 以 it-tools-template 现有数据库和系统监视器为首批替代 |
| P1 | TRANSFER-WORKBENCH-001 | 待开始 | 接收 API、Git、对象存储和普通容器流程 |
| P2 | TRANSFER-MESSAGE-001 | 待开始 | 接收 Redis、MongoDB、Kafka 和 MQTT |
| P2 | ENGINE-NATIVE-001 | 待开始 | 抽取 SSH/SFTP/PTY、隧道和安全存储 |
| P3 | DUAL-CLIENT-001 | 待开始 | Electron 与 GPUI 共享原生协议 |
| P4 | RETIRE-DUPLICATES-001 | 待开始 | 删除重复工具、依赖、导航和路线 |

当前旧的数据库、API、Kafka、SSH、容器和 Git UI 验收队列不再是新功能路线。
它们只在迁移替代或核心抽取需要时继续维护。具体旧证据见专项文档和历史归档。

## 5. 质量门槛与停止条件

每个阶段独立设计、验证、提交和回滚。Rust 变更执行格式、严格 Clippy、目标测试和
完整构建；原生连接使用本机 Docker 或真实本机服务；UI 证据区分 headless 与真实窗口。

出现以下情况时停止扩大范围：

- it-tools-template 尚未覆盖原流程的关键操作。
- Electron 与 GPUI 的错误、取消、权限或数据上限不一致。
- 迁移导致凭据、连接配置或本地数据丢失风险。
- 新增能力只是在两个仓库复制同一套业务逻辑。

## 6. 明确不做

- 不在 Ramag 继续维护数据库、Git、API、Kafka、MQTT、容器、对象存储、剪贴板和普通系统监控的第二套产品 UI。
- 不把 WebView 作为 GPUI 的替代实现。
- 不要求 Electron 和 GPUI 使用相同的 UI 组件或源码。
- 不在核心抽取完成前实现第三方动态插件市场、生产 Relay 或协同画布。
- 不以编译成功、静态截图或自动化启动代替原生交互验收。
