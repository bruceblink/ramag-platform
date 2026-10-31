# Ramag Platform 文档索引

[主线开发计划](01-development-plan.md)是唯一开发计划。任务范围、执行顺序、验收缺口和状态只在该文件维护。
本目录不再保留独立路线图、专项计划、发布计划或历史计划副本。旧记录通过 Git 历史查阅。

## 编号与职责

编号表示阅读顺序。只有 `01` 维护开发任务；其他文档提供规范、设计、操作方法或证据。
设计中尚未实现的能力不自动进入主线。一个主题只维护一份说明。

| 编号 | 文档 | 用途 |
|---|---|---|
| 01 | [主线开发计划](01-development-plan.md) | 范围、后续任务、依赖、完成条件和回滚边界 |
| 02 | [架构说明](02-architecture.md) | 分层、组件和依赖方向 |
| 03 | [开发入门指南](03-development-guide.md) | 环境、代码阅读、命令、本地服务与文档维护 |
| 04 | [UI 验收标准](04-ui-acceptance-standard.md) | UI 条件、数据库矩阵、性能与证据要求 |
| 05 | [静态插件接口与开发手册](05-plugin-development-guide.md) | 接口、生命周期、权限、设置、任务与测试 |
| 06 | [双项目能力矩阵](06-project-capability-matrix.md) | Ramag 与 `it-tools-template` 的独立边界 |
| 07 | [WSL Docker 环境](07-wsl-docker-environment.md) | Engine 使用、登录启动与环境恢复 |
| 08 | [桌面端构建与发布](08-desktop-release.md) | 三平台包、校验清单、更新和签名状态 |
| 09 | [容器管理设计](09-container-management-design.md) | Docker/Kubernetes 模型、操作和测试边界 |
| 10 | [MQTT 工作台设计](10-mqtt-workbench-design.md) | 客户端、本地 Broker 和配置协议 |
| 11 | [Kafka 工作台能力](11-kafka-workbench-capabilities.md) | 功能和证据范围 |
| 12 | [Kafka 传输能力](12-kafka-transport-capabilities.md) | native/pure-rust 的差异和限制 |
| 13 | [Kafka Broker 指标](13-kafka-broker-metrics.md) | 外部指标格式、认证和来源边界 |
| 14 | [性能报告](14-performance.md) | 已有测量及环境限制，不作为当前性能保证 |
| 15 | [数据库崩溃分析](15-dbclient-crash-analysis.md) | Windows 栈溢出的修复和证据 |
| 16 | [社区公告](16-community-announcement.md) | 上游旧版本介绍，不作为当前功能清单 |

## 历史证据与版本

[2026-10-06 测试报告](testing/2026-10-06-ui-integration-test-report.md)保留当次测试和数据恢复记录。
测试报告与截图按日期或版本命名，不参加现行说明文档的编号。
[版本变更记录](../CHANGELOG.md)保留已交付版本的用户可见变化。

新增技术文档先明确职责，再更新此索引；已有主题优先更新所属文档，不创建重复说明。
技术规范不承担任务状态和执行排期。所有设计图、流程图、架构图、状态图和文本图按 A4
打印与屏幕阅读要求排布，具体条件见[开发入门指南](03-development-guide.md#12-文档与图表维护)。
