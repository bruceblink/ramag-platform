# 首页与侧边栏工具入口添加和移除

> 切片：`A-POST-0.5-TOOL-SURFACE-CRUD-001`
> 日期：2026-10-16
> 基线：`35c1943b`；当前分支 `main`
> 状态：已完成
> 关联：[`12-tool-visibility-plan.md`](12-tool-visibility-plan.md)、[`01-development-plan.md`](01-development-plan.md)

## 问题与范围

Settings 已支持分别控制工具在首页和侧边栏的显示状态，但用户必须先离开工具表面再进入设置页完成操作。侧边栏“添加工具”目前只显示“即将支持”的提示；首页没有直接添加或移除入口。

本切片在首页和侧边栏分别提供“管理工具”面板。面板有搜索框，并将结果分为“当前显示”和“可添加”两组。每个工具只显示当前界面可执行的操作。用户添加或移除入口后，面板保持打开，列表立即更新，并使用现有工具可见性偏好保存结果。

移除只修改当前表面的入口状态。它不会卸载或注销工具，不会影响另一表面，不会删除工具配置、连接或数据。Settings 的现有开关继续使用相同偏好。

本切片不增加第三方插件安装、工具代码卸载、删除插件文件或 MCP 工具管理。

## 验收条件

- 首页和侧边栏的管理入口都直接打开对应的工具管理面板。
- 面板按当前界面的入口状态分组，并根据名称、说明和 ID 过滤工具；搜索后面板高度随匹配结果收缩。
- 添加或移除后，当前分组、按钮和对应表面立即更新；另一表面的状态保持不变。
- 面板保持打开，用户可以连续管理多个工具；工具可通过“完成”或关闭按钮离开面板。
- 同一工具从两个表面移除后，可由固定 Settings 入口重新显示。
- 操作通过 `tool_visibility` 偏好保存；重启加载仍沿用已有偏好恢复逻辑。
- 注册表条目、工具实例、配置和数据不因入口移除而改变。
- GPUI 交互测试覆盖搜索、首页和侧边栏面板、添加、移除、状态独立、面板保持打开及恢复；布局在窄窗口和桌面窗口均可用，搜索后会缩小空白区域。
- Computer Use 使用完整 Windows 程序验证实际点击、移除、重新添加和表面独立状态。Headless 结果不能代替该验证。

## 验证命令

- `cargo test --locked -p ramag-ui --lib -- --test-threads=1`
- `cargo fmt --all -- --check`
- `& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked', '--workspace', '--all-targets')`
- `& .\scripts\windows\check-source-size.ps1`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

## 回滚边界

只回滚首页和侧边栏管理面板、入口调用、交互测试、本计划和开发队列中对应记录。保留既有工具显示设置、注册表、工具数据、配置与连接。

## 交付记录

实现位于 `crates/ramag-ui/src/tool_surface_manager.rs`。首页和侧边栏的“管理工具”入口共用面板组件。面板按当前表面分组，搜索名称、说明和工具 ID。添加或移除后，面板保持打开并立即重绘。搜索结果减少时，面板高度同步收缩。工具状态只写入现有 `tool_visibility` 偏好。

定向 `ramag-ui --lib` 测试通过：141 项通过，0 项失败。`cargo fmt --all -- --check`、严格工作区 Clippy、源码尺寸检查、完整 `ramag-bin` 构建和 `git diff --check` 均通过。Docker 集成测试不适用；本切片只改桌面 UI 和本地显示偏好。

Computer Use 在真实 Windows 窗口验证了首页与侧边栏入口、搜索“API”、添加、移除、列表分组变化和面板保持打开。隐藏首页入口时，侧边栏入口仍可用；隐藏侧边栏入口时，首页卡片仍可用。使用隔离的本机数据目录重启程序后，显示状态仍然保留。Headless 测试只补充布局和状态断言，不代替上述窗口交互。

程序构建产物：`target/x86_64-pc-windows-msvc/debug/ramag.exe`。构建、测试数据和系统状态都位于仓库的忽略目录 `target/` 中。
