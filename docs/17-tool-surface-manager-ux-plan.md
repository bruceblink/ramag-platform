# 首页与侧边栏统一工具管理交互

> 切片：`A-POST-0.5-TOOL-SURFACE-MANAGER-001`
> 日期：2026-10-17
> 基线：`0e55b681`；当前分支 `main`
> 状态：已完成
> 关联：[工具入口基础能力](14-tool-surface-management-plan.md)、[原快捷操作记录](15-tool-surface-quick-actions-plan.md)、[最新执行计划](01-development-plan.md)、[主线路线图](02-development-roadmap.md)

## 问题与范围

当前首页和侧边栏使用不同操作添加和移除工具。用户从“+”选择器添加工具，再从每个工具项的“⋯”菜单移除工具。入口不一致，移除入口不容易发现。侧边栏工具项还要精确点中较小的菜单按钮。

本切片让首页和侧边栏的固定“管理工具”入口打开同一种列表面板。面板列出所有已注册工具，搜索框查找名称、说明和 ID。每行显示当前表面的添加状态，并直接提供“添加”或“移除”按钮。操作后，面板和工具表面立即更新，面板继续打开，便于连续整理多个入口。

移除只修改当前表面的入口显示偏好。它不会卸载或注销工具，不会影响另一表面、工具配置、连接或数据。Settings 的现有显示开关继续有效。

本切片不增加工具卸载、插件文件删除、第三方插件安装或市场能力。

## 验收条件

- 首页和侧边栏都用明确的“管理工具”入口打开同一种工具列表面板。
- 面板同时列出当前显示和未显示的全部已注册工具，并在每行显示可执行的“添加”或“移除”操作。
- 添加或移除后，行状态、计数和对应表面即时刷新；面板保持打开，可以连续操作。
- 搜索按工具名称、说明和 ID 过滤结果；无匹配项和无已注册工具时都显示明确状态。
- 每次操作只改当前表面偏好；不打开工具、不切换导航，不影响另一表面或工具数据。
- 首页与侧边栏管理面板在 `360x640` 紧凑窗口中保持可见；侧边栏弹出面板按可用宽度收缩。
- GPUI 交互测试覆盖两表面添加、移除、独立状态、搜索、连续操作、空状态和紧凑窗口边界。
- Computer Use 使用完整 Windows 程序验收真实窗口中的连续添加、移除和状态刷新。

## 验证命令

- `cargo test --locked -p ramag-ui --lib tool_surface_manager::tests -- --test-threads=1`
- `cargo test --locked -p ramag-ui --lib activity_bar_visual_tests -- --test-threads=1`
- `cargo fmt --all -- --check`
- `& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked', '--workspace', '--all-targets')`
- `& .\scripts\windows\check-source-size.ps1`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

## 回滚边界

回滚本切片的统一管理面板、对应测试和本计划记录。保留工具注册、首页与侧边栏显示偏好、Settings 管理和既有工具数据。

## 验收与交付记录

- Headless：`cargo test --locked -p ramag-ui --lib -- --test-threads=1` 通过，143 项通过、0 失败；其中统一管理面板 5 项、活动栏布局 2 项通过。测试覆盖两表面连续添加和移除、状态独立、筛选、空状态和紧凑窗口。
- Computer Use：使用完整 Windows `target/debug/ramag.exe` 和隔离数据目录 `target/ui-tool-manager-ux-20261017` 验收，窗口为 `2560x1392`。首页和侧边栏都从“管理工具”进入同一种列表；各执行一次移除和恢复。面板持续打开，按钮文字、首页计数和侧边栏入口即时更新；侧边栏操作没有改变首页状态。
- 紧凑窗口：`360x640` 下首页和侧边栏面板通过 GPUI 边界测试。
- 质量门：`cargo fmt --all -- --check`、严格 workspace Clippy、`scripts/windows/check-source-size.ps1`、`cargo build --locked -p ramag-bin` 和 `git diff --check` 均通过。
- Docker 集成：不适用。本切片只修改本地 UI 和入口显示偏好。
- 推送：在验证通过后提交并推送当前 `main`；最终提交和远端一致性以 Git 检查结果为准。
