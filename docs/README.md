# Ramag Platform 文档索引

[主线开发计划](01-development-plan.md)是唯一开发计划。任务范围、执行顺序、验收缺口和状态只在该文件维护。
本目录不再保留独立路线图、专项计划、发布计划或历史计划副本。旧记录通过 Git 历史查阅。

## 开发与规范

| 文档 | 用途 |
|---|---|
| [主线开发计划](01-development-plan.md) | 当前范围、任务顺序、验收条件、限制和回滚边界 |
| [开发入门指南](08-development-guide.md) | 环境准备、代码阅读顺序和贡献流程 |
| [本地开发速查](LOCAL_DEV.md) | 本地运行和常用命令 |
| [架构说明](06-architecture.md) | 分层、依赖方向和组件职责 |
| [UI 验收标准](07-ui-acceptance-standard.md) | 通用 UI 条件、数据库矩阵、性能与证据要求 |
| [静态插件开发手册](05-plugin-development-guide.md) | 插件实现、测试、注册和权限边界 |
| [插件 API 设计](27-ramag-plugin-api-design-001.md) | 静态接口、版本、生命周期、能力和承载方式边界 |
| [双项目能力矩阵](24-cross-project-capability-matrix.md) | Ramag 与 `it-tools-template` 的独立实现边界 |

## 领域技术参考

以下文档提供设计、接口和证据，不另行安排开发顺序。设计中尚未实现的能力不自动进入主线。

| 文档 | 用途 |
|---|---|
| [容器管理工具设计](container-management-tool-design.md) | Docker/Kubernetes 模型、操作、安全和测试方法 |
| [MQTT 详细设计](mqtt-wu-commtool-rewrite-design.md) | 客户端、本地 Broker、配置和协议边界 |
| [Kafka 工作台功能矩阵](kafka-workbench-feature-matrix.md) | 既有功能与证据范围 |
| [Kafka 传输能力矩阵](kafka-transport-capability-matrix.md) | native/pure-rust 能力差异和限制 |
| [Kafka Broker 指标接入](kafka-broker-metrics.md) | 外部指标格式、配置和安全要求 |
| [数据库 Windows 栈溢出修复记录](dbclient-crash-001.md) | 已修复问题的原因、链接参数和复现证据 |

## 环境、发布与测试

| 文档 | 用途 |
|---|---|
| [WSL Docker 开发环境](26-wsl-docker-development-environment.md) | Docker Engine 使用、登录启动和环境恢复 |
| [桌面端构建与发布](desktop-release.md) | 三平台打包、清单、更新、签名状态和发布流程 |
| [性能报告](performance.md) | 已有测量方法、结果及适用环境 |
| [2026-10-06 测试报告](testing/2026-10-06-ui-integration-test-report.md) | 当次自动测试、Docker、真实窗口和数据恢复记录 |
| [版本变更记录](../CHANGELOG.md) | 已交付版本的用户可见变化 |
| [社区公告](rustcc-announcement.md) | 项目介绍与历史公告 |

新增技术文档先明确职责，再更新此索引。技术规范不承担任务状态和执行排期。
