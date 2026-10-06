# Ramag Platform 开发主线与设计文档

当前开发与设计只保留 `docs/01` 至 `docs/08` 这一条编号主线。编号表示执行优先级和阅读顺序；专项设计文件只作为实现参考，不另起一套开发排期。

| 编号 | 文档 | 职责 |
|---|---|---|
| 01 | [执行计划](01-development-plan.md) | 当前切片顺序、设计确认、验收条件和交付记录 |
| 02 | [主线路线图](02-development-roadmap.md) | 跨工具产品顺序、依赖和当前队列 |
| 03 | [数据库工作区路线图](03-database-client-datagrip-roadmap.md) | 数据库工作区功能、驱动差异和 DataGrip 风格验收 |
| 03 | [System Pulse 采集与 UI 吸收](03-system-pulse-adoption-plan.md) | 完整源码迁入待确认方案、历史采集记录及验收后的公共 UI 推广 |
| 04 | [插件平台路线图](04-plugin-platform-roadmap.md) | 插件 API、权限、生命周期、目录和协作边界 |
| 04 | [System Pulse 差距矩阵](04-system-pulse-gap-matrix.md) | 固定来源对照、剩余优点、独立切片顺序和本机验收入口 |
| 05 | [插件开发指南](05-plugin-development-guide.md) | 内置插件的实现、测试、注册和安全边界 |
| 06 | [架构说明](06-architecture.md) | 已实现分层、依赖方向和技术决策 |
| 07 | [UI 验收标准](07-ui-acceptance-standard.md) | GPUI 布局、交互、证据和真实窗口边界 |
| 08 | [开发入门指南](08-development-guide.md) | 环境准备、代码阅读顺序和贡献流程 |

历史计划和验收记录保留在 [`archive/2026-09-25-pre-datagrip-rebaseline/`](archive/2026-09-25-pre-datagrip-rebaseline/) 供追溯，不作为当前待办。已完成、重复或与当前主线冲突的旧计划已移除；新增计划必须使用两位数字前缀，并同步更新本目录和仓库链接。

本机测试证据：[2026-10-06 完整测试与 UI 功能测试报告](testing/2026-10-06-ui-integration-test-report.md)。报告记录实际通过范围、外部服务环境和未完成项，不另起开发排期。
