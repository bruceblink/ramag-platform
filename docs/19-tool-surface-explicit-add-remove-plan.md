# 首页与侧边栏工具入口的明确添加和移除操作

> 切片：`A-POST-0.5-TOOL-SURFACE-MANAGER-003`
> 日期：2026-10-17
> 基线：`a4f68d2f`；当前分支 `main`
> 状态：已完成
> 关联：[统一工具管理交互](17-tool-surface-manager-ux-plan.md)、[入口显示开关优化](18-tool-surface-manager-switches-plan.md)、[最新执行计划](01-development-plan.md)

## 问题与范围

当前面板要求用户先选择首页或侧边栏，再操作每个工具的开关。开关把“添加”和“移除”压成同一个状态变化，用户不容易看出操作结果，也容易误解为卸载工具。

本切片保留首页和侧边栏各自的入口按钮。首页按钮直接显示“添加工具”，侧边栏使用加号图标。按钮打开当前表面的工具管理面板。面板按“已添加”和“可添加”分组，每行显示明确的“移除”或“添加”操作。操作只更新当前表面的入口偏好，面板保持打开，列表立即移动该工具到另一组。

移除只隐藏当前表面的入口。它不卸载工具，不影响另一表面、连接、配置或数据。

本切片不增加插件卸载、插件文件删除、第三方插件安装或工具市场。

## 验收条件

- 首页和侧边栏入口分别打开对应范围的管理面板；面板标题清楚标出当前范围。
- 面板分别显示已添加和可添加工具的数量、分组和空状态。
- 已添加工具只提供明确的“移除”操作；可添加工具只提供明确的“添加”操作。
- 操作后工具立即移动到另一分组；面板保持打开，搜索内容和当前范围不变。
- 操作只更新当前表面；另一表面、工具注册、连接、配置和数据不变。
- 搜索继续匹配名称、说明和 ID；无匹配、无注册工具、无已添加工具和无可添加工具都有清楚提示。
- `360x640` 窄窗口中面板和操作按钮完整落在窗口内；长工具名称不挤压操作按钮。
- GPUI 交互测试覆盖首页和侧边栏的添加、移除、范围隔离、搜索、空状态和紧凑窗口。
- Computer Use 使用隔离数据的完整 Windows 程序检查两个入口、面板分组、操作结果及显示更新。

## 验证命令

- `cargo test --locked -p ramag-ui --lib tool_surface_manager -- --test-threads=1`
- `cargo test --locked -p ramag-ui --lib activity_bar_visual_tests -- --test-threads=1`
- `cargo fmt --all -- --check`
- `& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked', '--workspace', '--all-targets')`
- `& .\scripts\windows\check-source-size.ps1`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

## 回滚边界

只回滚本切片的管理面板、首页和侧边栏管理入口、对应测试及本计划。保留工具注册、入口可见性偏好、Settings 管理和工具数据。

## 验收与交付记录

- Headless：`cargo test --locked -p ramag-ui --lib tool_surface_manager -- --test-threads=1` 通过，6 项通过、0 失败。`cargo test --locked -p ramag-ui --lib -- --test-threads=1` 通过，144 项通过、0 失败。
- Computer Use：在隔离数据目录 `target/ui-tool-manager-explicit-add-remove-20261017` 中运行完整 Windows 程序，窗口为 `1200x811`。首页和侧边栏入口分别显示“添加工具”或加号，并打开各自范围的管理面板。搜索“剪贴板”后分别移除再添加；面板保持打开，工具在两组间移动，计数和对应入口即时更新，另一表面的计数与入口保持不变。
- 构建：标准 `cargo build --locked -p ramag-bin` 因原 `target/debug/ramag.exe` 正在运行而无法替换目标文件。使用临时独立二进制名和单实例名称执行 `cargo build --locked -p ramag-bin --bin ramag-ui-ux-test`，完整程序构建成功；构建后已恢复 `Cargo.toml` 和 `single_instance.rs`，未留下差异。
- 质量门：`cargo fmt --all -- --check`、严格 workspace Clippy、`scripts/windows/check-source-size.ps1` 和 `git diff --check` 均通过。
- Docker 集成：不适用。本切片只修改本地 UI 和入口显示偏好。
- 清理：验收进程退出后删除隔离数据目录；未关闭或更改原有 Ramag 窗口。
