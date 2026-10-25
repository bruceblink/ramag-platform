# Ramag Platform 开发主线与设计文档

当前开发与设计使用一条编号主线。编号表示执行优先级和阅读顺序；专项设计文件只作为实现参考，不另起一套开发排期。`01-development-plan.md` 是当前单一执行入口，历史长记录保留在 `01-development-plan-history.md`。

| 编号 | 文档 | 职责 |
|---|---|---|
| 01 | [最新执行计划](01-development-plan.md) | 当前职责边界、切片顺序、验收门槛和交付规则 |
| 01 | [执行历史](01-development-plan-history.md) | 旧 `01` 的详细切片设计和验收记录 |
| 02 | [主线路线图](02-development-roadmap.md) | 当前维护主线、完成门槛和主线后的插件化阶段 |
| 03 | [数据库工作区路线图](03-database-client-datagrip-roadmap.md) | 数据库工作区功能、驱动差异和 DataGrip 风格验收 |
| 03 | [System Pulse 采集与 UI 吸收](03-system-pulse-adoption-plan.md) | 采集替换、视觉基准、当前 UI 队列和逐工具证据 |
| 04 | [插件平台路线图](04-plugin-platform-roadmap.md) | 当前静态插件基线、后主线接口设计、试点和迁移顺序 |
| 04 | [System Pulse 差距矩阵](04-system-pulse-gap-matrix.md) | 固定来源对照、系统监控差距和本机验收入口 |
| 05 | [插件开发指南](05-plugin-development-guide.md) | 内置插件的实现、测试、注册和安全边界 |
| 06 | [架构说明](06-architecture.md) | 已实现分层、依赖方向和技术决策 |
| 07 | [UI 验收标准](07-ui-acceptance-standard.md) | GPUI 布局、交互、证据和真实窗口边界 |
| 08 | [开发入门指南](08-development-guide.md) | 环境准备、代码阅读顺序和贡献流程 |
| 专项 | [v0.5.0 发布计划](08-v0.5.0-ui-release-plan.md) | v0.5.0 发布与首页统一的完成记录 |
| 专项 | [v0.5.0 后 UI 修复](09-post-0.5.0-ui-fixes-plan.md) | 剪贴板删除、搜索和撤销状态切片 |
| 专项 | [Disks/Network 详细页计划](10-system-disks-network-plan.md) | System Pulse 磁盘与网络详细页对齐 |
| 专项 | [DBClient 列表状态修复](11-dbclient-list-state-plan.md) | 搜索无匹配时的读取失败、重试与稳定 ID 点击回归 |
| 专项 | [首页与侧边栏工具快捷操作（历史记录）](15-tool-surface-quick-actions-plan.md) | 记录选择器添加和入口菜单移除版本；现由统一管理面板替换 |
| 专项 | [对象存储账号列表与 RustFS 数据源验收](16-object-storage-list-acceptance-plan.md) | 验证持久化账号列表、行状态和本机 RustFS 工作区连接 |
| 专项 | [首页与侧边栏统一工具管理交互](17-tool-surface-manager-ux-plan.md) | 记录统一搜索和工具入口管理面板的基础实现 |
| 专项 | [工具入口显示开关交互优化](18-tool-surface-manager-switches-plan.md) | 通过首页/侧边栏范围标签和单个显示开关管理入口 |
| 专项 | [首页与侧边栏工具入口的明确添加和移除操作](19-tool-surface-explicit-add-remove-plan.md) | 用明确的添加/移除操作和分组替代范围切换及单一显示开关 |
| 专项 | [首页与侧边栏工具显示位置优化](20-tool-surface-manager-inline-visibility-plan.md) | 每个工具只列一次，并在同一行管理首页和侧边栏显示状态 |
| 专项 | [列表型工作区完整程序验收](21-list-workspace-acceptance-plan.md) | 按 Container、VCS、插件顺序记录真实窗口、滚动和列表状态验收 |
| 专项 | [连接型工作区完整程序验收](22-connection-workspace-acceptance-plan.md) | 按 SSH/SFTP、API、Kafka、MQTT 顺序记录表单、连接、取消和错误恢复验收 |
| 专项 | [GPUI Kit 0.7.3 升级与 Ramag 0.6.0 发布](23-gpui-kit-0.7.3-upgrade-release-plan.md) | 迁移 GPUI Kit 0.7.3、执行完整验证并发布多平台桌面版本 |

历史计划和验收记录保留在 [`archive/2026-09-25-pre-datagrip-rebaseline/`](archive/2026-09-25-pre-datagrip-rebaseline/) 及 [执行历史](01-development-plan-history.md) 供追溯，不作为当前待办。新增计划必须使用两位数字前缀，并同步更新本目录和仓库链接。

本机测试证据：[2026-10-06 完整测试与 UI 功能测试报告](testing/2026-10-06-ui-integration-test-report.md)。报告记录实际通过范围、外部服务环境和未完成项，不另起开发排期。
