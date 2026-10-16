# 工具入口显示开关交互优化

> 切片：`A-POST-0.5-TOOL-SURFACE-MANAGER-002`
> 日期：2026-10-17
> 基线：`52b8793c`；当前分支 `main`
> 状态：已完成
> 关联：[统一工具管理交互](17-tool-surface-manager-ux-plan.md)、[最新执行计划](01-development-plan.md)

## 问题与范围

上一版管理面板为每个工具直接显示“添加”或“移除”按钮。按钮没有突出当前操作的是首页还是侧边栏。“移除”也可能被理解为卸载工具。

本切片保留一个统一管理面板。面板增加“首页”和“侧边栏”两个范围标签，并显示各自已启用的工具数量。用户先选择范围，再用工具行中的单个开关控制入口显示。状态更新后面板保持打开。首页和侧边栏入口都打开同一种面板。

关闭开关只隐藏所选范围的入口。工具注册、另一范围的入口、工具配置、连接和数据都保持不变。

本切片不增加工具卸载、删除插件文件、第三方插件安装或市场能力。

## 验收条件

- 两处管理入口都打开同一种可搜索管理面板。
- 面板明确显示当前范围，并提供首页和侧边栏切换标签。
- 每行只显示一个入口开关。开关与当前范围一致，操作只修改该范围。
- 两个标签显示当前入口数量；开关切换后数量和对应工具表面即时更新。
- 面板保持打开，用户可以连续切换范围、搜索和管理多个工具。
- 面板明确说明关闭开关只隐藏入口，不删除工具和数据。
- 无注册工具、搜索无匹配结果及 `360x640` 紧凑窗口均有测试覆盖。
- Computer Use 使用完整 Windows 程序验证两处入口、范围切换、首页开关、侧边栏开关和即时刷新。

## 验证命令

- `cargo test --locked -p ramag-ui --lib tool_surface_manager -- --test-threads=1`
- `cargo test --locked -p ramag-ui --lib tool_visibility -- --test-threads=1`
- `cargo fmt --all -- --check`
- `& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked', '--workspace', '--all-targets')`
- `& .\scripts\windows\check-source-size.ps1`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

## 回滚边界

只回滚管理面板范围标签、显示开关、对应测试和本计划。保留首页与侧边栏工具入口、现有工具可见性偏好、注册表、工具配置和数据。

## 验收与交付记录

- Headless：`cargo test --locked -p ramag-ui --lib tool_surface_manager -- --test-threads=1` 通过，5 项通过、0 失败。完整 `ramag-ui` 测试通过，143 项通过、0 失败。测试覆盖范围切换、单范围开关、连续添加/隐藏、独立状态、搜索、空状态和紧凑窗口。
- Computer Use：使用 Windows 完整 `ramag.exe` 验收，分辨率 `2560x1392`。首页入口和侧边栏固定入口都打开同一种面板。面板按入口来源选择首页或侧边栏范围。首页数据库工具隐藏后卡片和数量即时减少，再打开后恢复。侧边栏数据库工具隐藏后侧栏图标和数量即时减少，再打开后恢复。每次操作都保持另一表面的入口不变。面板在两处操作后都保持打开。
- 测试隔离：完整程序使用仓库 `target/ui-tool-surface-manager-switches-20261017-run/full-app` 下的独立数据和系统状态目录。为避免影响已运行的主程序，本机验收二进制临时使用单独的单实例互斥体和事件名称。验收结束后已恢复原代码；`crates/ramag-bin/src/single_instance.rs` 没有差异。
- 补充 Computer Use：使用生产 Shell 的 `ui-preview` 在 `1440x900` 窗口验证首页与侧边栏范围、显示开关、数量更新和面板保持打开。预览进程退出后，其临时 redb 和系统状态目录均已清理。
- 质量门：`cargo fmt --all -- --check`、严格 workspace Clippy、`scripts/windows/check-source-size.ps1`、完整 `ramag-bin` 构建和 `git diff --check` 均通过。
- Docker 集成：不适用。本切片只修改桌面 UI 和本地显示偏好。
- 提交与推送：本切片使用独立的 `fix: clarify tool surface visibility controls` 提交推送到 `origin/main`；交付后的分支状态再由 Git 检查确认。
