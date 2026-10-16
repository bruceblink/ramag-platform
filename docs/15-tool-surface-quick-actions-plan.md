# 首页与侧边栏工具入口快捷操作

> 切片：`A-POST-0.5-TOOL-SURFACE-QUICK-ACTIONS-001`
> 日期：2026-10-17
> 基线：`d8137721`；当前分支 `main`
> 状态：已完成
> 关联：[工具入口管理](14-tool-surface-management-plan.md)、[最新执行计划](01-development-plan.md)、[主线路线图](02-development-roadmap.md)

本文件记录先前已交付的快捷操作版本。后续用户反馈指出，添加选择器和入口“⋯”菜单分开导致操作不一致。当前实现由[统一工具管理计划](17-tool-surface-manager-ux-plan.md)替换；本文件中的验收记录保留为历史事实。

## 问题与范围

当前添加和移除都要打开较大的管理对话框，再搜索并在两个分组中找到目标。常用的添加和移除动作需要经过多步操作。

本切片将添加入口改为紧邻“+”按钮的轻量搜索选择器。选择器只列出当前表面未显示的工具，点击工具即可添加。移除入口放在每个已显示工具自己的更多操作中。首页卡片和侧边栏工具都可以直接移除。

每次操作只修改当前表面的入口偏好。另一表面、工具注册、配置、连接和数据保持不变。工具卡片打开、侧边栏导航、拖动排序和设置页恢复入口的行为保持可用。

## 验收条件

- 首页和侧边栏的“+”都打开对应表面的紧凑、可搜索选择器；选择器只列出未显示的工具。
- 点击搜索结果立即添加入口，选择器保持打开，列表马上更新；无可添加项时显示明确空状态。
- 首页卡片与侧边栏工具入口都提供清楚可发现的“更多操作”，其中有直接的“从首页/侧边栏移除”操作。
- 移除不触发工具导航或卡片打开，不卸载工具，不影响另一表面。
- 管理动作写入现有显示偏好；无工具可见时仍能从固定首页或侧边栏“+”恢复入口。
- GPUI 测试覆盖两个表面的搜索、添加、移除、状态独立、空状态和基础窗口边界。
- Computer Use 使用完整 Windows 程序验收真实点击、连续添加、移除与页面状态。

## 验证命令

- `cargo test --locked -p ramag-ui --lib -- --test-threads=1`
- `cargo fmt --all -- --check`
- `& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked', '--workspace', '--all-targets')`
- `& .\scripts\windows\check-source-size.ps1`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

## 回滚边界

回滚本切片的快速选择器、工具入口的更多操作、对应测试及计划记录。保留现有工具显示偏好、注册表、工具数据和设置页恢复能力。

## 验收与交付记录

- 状态：已完成；在 `main` 基线 `d8137721` 上实现，随本切片提交并推送。
- Headless：`cargo test --locked -p ramag-ui --lib -- --test-threads=1` 通过，141 项通过、0 失败。测试覆盖搜索、当前表面过滤、点击添加、菜单移除、两个表面状态独立、无意导航防护、空状态和紧凑窗口边界。
- 原生窗口：使用完整 Windows `ramag.exe` 和隔离测试数据目录完成交互。首页验证移除与搜索恢复；侧边栏验证从工具更多菜单移除 API 入口，再从底部“+”找到并恢复，选择器在操作后保持打开并显示更新结果。设置页的工具注册和首页入口保持可见。
- 质量门：`cargo fmt --all -- --check`、严格 workspace Clippy、`scripts/windows/check-source-size.ps1`、`cargo build --locked -p ramag-bin` 和 `git diff --check` 均通过。
- Docker 集成：不适用。本切片只修改本地 UI 与入口显示偏好，不访问外部服务。
- UI 尺寸：当前 Windows 主屏窗口为 `2560x1392`；`360x640` 紧凑窗口边界由 GPUI 测试覆盖。
- 推送结果：本文件和实现一起提交到当前 `main`，并推送至 `origin/main`；提交 SHA 以 Git 历史为准。
