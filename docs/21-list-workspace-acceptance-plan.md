# 列表型工作区完整程序验收计划

> 切片：`A-POST-0.5-LIST-ACCEPTANCE-001`
> 日期：2026-10-17
> 状态：完成；Container、VCS 和插件子切片均已完成
> 范围：完整程序中的 Container、VCS 和插件列表工作区

## 问题证据

`docs/testing/2026-10-06-ui-integration-test-report.md` 已记录 Container Engine 信息和镜像排序、VCS diff 和暂存操作的真实窗口流程。该报告没有覆盖三个工作区全部的滚动、空状态、加载状态和失败状态。计划开始时，插件诊断页尚无完整程序的真实窗口记录。

## 实施范围

- 按 Container、VCS、插件的顺序逐项验收。三个子切片现已完成。
- Container 使用隔离的 Ramag 设置目录。只读访问本机 Docker Engine；不创建、启动、停止、修改或删除任何 Engine 资源。
- 检查页面标题、字号与颜色、滚动范围、筛选后空结果、加载反馈、失败反馈和主要只读操作。
- 已通过的现有测试继续作为 headless 证据；使用 Computer Use 检查从当前源码构建的真实窗口。
- 发现问题时，只修改当前工作区所需的 UI、目标回归测试和专项记录。本轮插件改动只增加空状态定位标识和对应 headless 回归测试。

## 验收条件

- 从当前 `main` 构建完整 `ramag.exe`，并在临时数据目录中启动，确认没有读取或修改用户连接和偏好。
- Container 在常用窗口尺寸下可读、可滚动；列表有内容和无匹配结果时，标题、状态与主要只读操作均可达。
- 可从真实窗口观察加载与连接失败反馈；失败后能安全重试或回到可用状态，不触发 Engine 写操作。
- VCS 在真实窗口中显示仓库状态、无匹配筛选、差异加载和双栏差异；验收过程不得修改测试仓库。
- 插件页在真实窗口中显示第一方目录、可用入口和生命周期状态，并且能够滚动到页面末尾。生产宿主的空状态和初始化失败使用受控测试宿主通过 headless 视口测试；插件诊断页同步读取宿主快照，没有独立页面加载态。
- `ramag-tool-container` 目标测试、完整程序构建、格式、严格 workspace Clippy、源码尺寸和差异检查通过。
- 记录 Docker Engine 版本、访问入口、操作范围和清理结果；未启动容器时明确说明。

## 验证命令

```powershell
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-container','--lib','--','--test-threads=1')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-vcs','--lib','--','--test-threads=1')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-ui','plugin_diagnostics','--','--test-threads=1')
cargo fmt --all -- --check
& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked','--workspace','--all-targets')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand build -CargoOptions @('--locked','-p','ramag-bin')
& .\scripts\windows\check-source-size.ps1
git diff --check
```

## Container 子切片执行记录

- 用户确认 System 工具已全部测试通过且无已知问题。本轮不重复运行 System 工具测试。
- 本机服务为 Docker Desktop Linux Engine，版本 `29.7.2`，API `1.55`，架构 `linux/amd64`。程序通过 `npipe:////./pipe/docker_engine` 连接。该测试没有使用 TCP 端口，也没有启动容器镜像；镜像版本不适用。
- Docker 驱动只读集成测试通过：`tests::reads_local_engine_without_write_operations`，1 项通过、0 项失败。测试读取 Engine 信息、容器、镜像、网络、数据卷，并查询不存在的容器。
- Engine 已在本机运行。测试没有启动、停止、创建、修改或删除 Docker 资源。容器清理不适用；无需执行 Docker 清理。
- 调试程序使用 `%TEMP%` 下独立的连接数据目录和系统状态目录。程序没有读取或修改用户目录中的连接、凭据或偏好。测试程序已关闭，两个临时目录已删除。
- Computer Use 已在完整程序的真实窗口完成验收。窗口尺寸为 `1202×812`。概览显示 18 个容器、7 个运行中容器、30 个镜像、10 个网络、25 个数据卷、24 个 CPU 核数和 `23.4 GiB` 内存。容器表显示真实本机记录，且可纵向和横向滚动。
- 无匹配筛选显示“暂无容器”。连接请求显示“读取中”。明文 TCP 地址在本机校验后显示“采集失败”和 `远程 Docker TCP 端点必须使用 HTTPS/TLS`，没有发送远程请求。改回本机命名管道后，连接恢复并再次显示 Engine 数据。
- 修复了 Docker 地址输入观察器与刷新请求之间的竞态。同地址的延迟观察不再使刷新请求失效。地址变化仍会取消旧请求并清除旧连接状态。
- Container 库测试通过：36 项通过、0 项失败。格式、严格 workspace Clippy、完整程序构建、源码尺寸和差异检查均通过。

## VCS 子切片执行记录

- 用户确认 System 工具已全部测试通过且无已知问题。本轮不重复运行 System 工具测试。
- 使用隔离仓库 `.ramag/ui-acceptance/repo` 和隔离的 Ramag 数据目录。完整调试程序在 `1202×812` 真实 Windows 窗口中运行。测试没有读取用户的仓库历史记录或偏好。
- 仓库管理页显示最近打开的 1 个仓库。搜索不存在的名称时，页面显示“没有匹配「ramag-no-match-acceptance」的仓库”；清空搜索后，原仓库恢复显示。只有一个仓库，因此长列表滚动验收不适用。
- 打开隔离仓库后，页面显示分支 `main` 和 `sample.txt` 的修改状态。选择文件时页面先显示“拉取中…”，随后正确显示左右双栏差异，包括原始行 `Ramag UI test data.` 和已存在的修改行 `Changed line for UI diff inspection.`。
- 本轮只读查看仓库和差异，没有暂存、取消暂存、提交、推送、删除仓库或修改文件。操作前后 `git status --short --branch` 均为 `## main`、` M sample.txt`；文件差异与进入程序前一致。
- 本轮真实窗口验证了无匹配状态、异步加载反馈和成功差异显示。打开仓库失败状态未通过真实窗口触发；现有渲染测试在 360、640、1024 像素宽度下检查仓库列表与工作区错误通知布局，不将其描述为真实窗口验证。
- VCS 库测试通过：188 项通过、0 项失败、5 项忽略。`cargo fmt --all -- --check`、workspace 严格 Clippy、`ramag-bin` 构建、源码尺寸检查和 `git diff --check` 均通过。

## 插件子切片执行记录

- 用户确认 System 工具已经全部测试通过且无已知问题。本轮不重复运行 System 工具测试。
- 完整调试程序使用隔离的 `%TEMP%` Ramag 数据目录和系统状态目录，在 `1202×812` 的真实 Windows 窗口运行。程序窗口已在验收后关闭；隔离目录仍保留在 `%TEMP%`，没有读取或修改用户设置、连接或凭据。
- 插件设置页显示 `12` 个静态插件、`12` 个已就绪、`0` 个待处理。第一方工具目录、可用入口和插件状态列表均可在页面纵向滚动中访问，所有 12 个入口状态显示“已就绪”。界面只列出随应用发布的静态入口。
- 生产宿主没有空列表或初始化失败。本轮没有把未触发的状态写成真实窗口通过。空目录、空入口和空生命周期列表由空 `StaticPluginHost` 在 360、640、1024 像素宽度的 headless 测试覆盖。初始化失败与注册失败、长诊断文案由 360、1024、1440 像素宽度的 headless 布局测试覆盖。
- 插件诊断视图同步读取宿主目录、入口和生命周期快照，没有独立的异步加载流程，因此插件页面加载态不适用。本轮不启动 Docker 或其他外部服务。
- 插件诊断测试通过：2 项通过、0 项失败。格式、workspace 严格 Clippy、`ramag-bin` 构建、源码尺寸检查和差异检查均通过。

## 回滚边界

只回滚本轮新增的插件空状态定位标识、headless 回归测试和对应验收记录。Container 修复与 VCS 验收记录保留各自的提交边界；不修改 Docker Engine 资源、用户连接、隔离 VCS 仓库内容或插件注册与目录。
