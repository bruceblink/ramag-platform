# 首页与侧边栏工具显示位置优化

> 切片：`A-POST-0.5-TOOL-SURFACE-MANAGER-004`
> 日期：2026-10-17
> 基线：`6423fa06`；分支：`main`
> 状态：已完成
> 关联：[统一工具管理交互](17-tool-surface-manager-ux-plan.md)、[明确添加和移除操作](19-tool-surface-explicit-add-remove-plan.md)、[最新执行计划](01-development-plan.md)

## 问题与范围

用户反馈，分开显示“已添加”和“可添加”工具，再逐项执行“添加”或“移除”，操作繁琐。首页和侧边栏入口都应打开同一组管理内容。

本切片保留可搜索工具目录。每个已注册工具只显示一行。每行显示两个独立开关：一个控制首页入口，一个控制侧边栏入口。开关直接显示当前状态。搜索、修改状态和数量统计都在同一面板内完成。

关闭开关只隐藏当前入口。工具注册、另一个入口、配置和数据都保留。本切片不增加插件卸载或删除工具文件。

## 验收条件

- 首页与侧边栏的管理按钮打开相同的可搜索工具目录。
- 每个工具只出现一次，并同时显示首页与侧边栏开关。
- 任一开关只更新对应入口；另一入口状态保持不变。
- 状态更新后，工具列表和数量立即刷新；面板保持打开。
- 搜索名称、说明和 ID；无工具及无匹配结果都有清楚提示。
- 面板说明关闭入口只隐藏该位置，不卸载工具或删除数据。
- `360x640` 窄窗口中，面板和两个开关完整可见或可滚动到达。
- GPUI 测试覆盖两个入口、状态隔离、搜索、空状态和紧凑窗口。
- 真实窗口验收使用独立数据目录，不更改已有测试表单。

## 验证命令

- `cargo test --locked -p ramag-ui --lib tool_surface_manager -- --test-threads=1`
- `cargo fmt --all -- --check`
- `& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked', '--workspace', '--all-targets')`
- `& .\scripts\windows\check-source-size.ps1`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

## 回滚边界

只回滚工具管理面板、首页与侧边栏入口文案、对应测试和本计划。保留工具注册、入口显示偏好、设置页和工具数据。

## 验收记录

- Headless：`cargo test --locked -p ramag-ui --lib -- --test-threads=1` 通过，144 项通过、0 失败。工具管理专项 6 项全部通过。
- Computer Use：使用 `target/ui-tool-manager-inline-20261017` 隔离数据启动 Windows 程序，窗口为 `2560x1392`。首页和侧边栏加号均打开相同工具目录。关闭“首页”后首页计数从 12 降到 11，侧边栏仍为 12；关闭“侧边栏”后侧边栏计数从 12 降到 11，首页仍为 12。两项操作都即时更新并保持面板打开。搜索“剪贴板”只显示匹配行及两个入口开关。测试结束时恢复为 12/12，清空搜索并关闭测试程序；原有云存储表单未操作。
- 构建：`cargo build --locked -p ramag-bin --bin ramag-tool-manager-ux-20261017` 完整构建成功。临时二进制目标与单实例名称已从源码恢复。原 `ramag.exe` 仍被对象存储测试窗口占用，因此本次没有替换它。
- 质量门：严格 workspace Clippy、`cargo fmt --all -- --check`、`scripts/windows/check-source-size.ps1` 和 `git diff --check` 均通过。
- Docker 集成：不适用。本切片只修改本地工具入口显示交互。
- 清理：独立验收程序已退出。隔离目录只留下本次测试生成的 `ramag.redb`（548864 字节）。执行策略拒绝删除该文件和临时测试二进制；它仍位于 `target/debug/ramag-tool-manager-ux-20261017.exe`。
