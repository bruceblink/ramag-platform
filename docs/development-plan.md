# Ramag Platform 执行计划与验收记录入口

> 状态：现行执行规则
> 更新日期：2026-09-25
> 主线：[`development-roadmap.md`](development-roadmap.md)
> 统一 UI 标准：[`ui-acceptance-standard.md`](ui-acceptance-standard.md)
> 历史执行记录：[`archive/2026-09-25-pre-datagrip-rebaseline/development-plan.md`](archive/2026-09-25-pre-datagrip-rebaseline/development-plan.md)

## 术语表与命名约定

| 规范中文名 | English / Acronym | 当前文件中的职责边界 | 不代表什么 |
|---|---|---|---|
| 设计确认 | Design Confirmation | 在实现前确认切片范围、界面规则和验收条件 | 不代表代码已完成 |
| 验收记录 | Acceptance Record | 记录命令、环境、结果、证据边界和未完成项 | 不代表所有产品线都通过 |
| 本机 Docker 集成 | Local Docker Integration | 使用本机容器验证真实数据库或协议服务 | 不代表远程集群或生产验证 |
| 真实窗口 | Native Window | 通过 Computer Use 在 Windows 窗口中完成的操作 | 不代表 headless 渲染或进程启动 |

## 1. 每个切片的固定顺序

1. 阅读现行主线和对应专项路线图，确认没有把归档事项重新列为待办。
2. 写明问题证据、用户流程、设计、改动范围、不做事项和验收条件；先完成设计确认，再开始代码。
3. 实现代码、测试、必要注释和文档；所有新建/修改文本文件使用 LF 换行。
4. 先运行目标测试和 UI 验收，再运行 Rust workspace 的 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`git diff --check` 和适用的源码尺寸检查。
5. 涉及外部服务时使用本机 Docker，记录服务名、镜像版本、端口、启动/健康状态和停止/清理状态。
6. 测试通过后使用一个英文 Conventional Commit，立即推送 `main`；未经验证、部分完成或存在未说明无关改动时不得提交。

## 2. UI 验收顺序

- 先在 headless GPUI 中验证 `360x640`、`1024x768`、`1440x900`；弹窗追加 `360x240`。
- Computer Use 可用时，按真实用户流程完成启动、连接/加载、点击、键盘、滚动、编辑、取消和截图；每张截图对应一个验收条件。
- Computer Use 不可用时，先记录启动、窗口发现或交互失败原因，再使用 headless 或系统截图作为替代，并明确没有覆盖的真实窗口行为。
- 不得把系统截图、静态图片或仅进程启动描述为真实窗口交互完成。

## 3. 验收记录模板

每个切片在对应专项文档追加以下字段：

- 切片 ID、日期、提交和推送结果；
- 设计与改动范围、不做事项；
- 目标测试、UI 尺寸和交互步骤；
- Docker 服务、镜像、端口、启动/健康/清理状态；
- Headless、真实窗口、数据库/协议结果及各自边界；
- 未完成项、阻塞项和下一项依赖。

## 4. 当前切片

当前按照 [`development-roadmap.md`](development-roadmap.md) 从 `SHELL-001` 开始。未完成的旧 UI-001、M1-M4、R 系列或工具专项事项必须先映射到新的切片 ID，并重新满足统一 UI 标准后才能恢复；不能仅修改状态文字宣称完成。

## 5. 分支和清理

默认在最新 `main` 上开发和推送。只有用户明确要求功能分支时才创建分支；分支必须基于最新 `main`，验证后合并回 `main`，重新验证并推送，再确认源分支无未合并/未推送提交和关联 worktree 后清理。不得删除 `main` 或未明确纳入本次合并的分支。
