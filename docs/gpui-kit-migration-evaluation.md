# GPUI Kit 依赖迁移实施基线

评估日期：2026-09-22

## 术语表与命名约定

| 规范名称 | English / Identifier | 当前方案中的职责边界 | 不代表什么 |
|---|---|---|---|
| GPUI 运行时 | GPUI / `gpui` | 提供窗口、元素、事件、状态和测试运行时 | 不代表样式组件库 |
| GPUI 组件库 | GPUI Component / `gpui-component` | 提供 `Input`、`Root`、布局辅助函数和主题组件 | 不代表完整应用入口 |
| GPUI Kit facade | GPUI Kit / `gpui-kit`, `gpui_kit` | 聚合匹配版本的 GPUI、Base、Component、Assets 和 Platform，供应用使用 | 不代表旧 `gpui` Git 包的兼容别名 |
| 发布版 GPUI 家族 | Published GPUI family / `gpui-pre-*` | GPUI Kit 0.6.x 使用的匹配版本依赖集合 | 不代表当前 Cargo.lock 中的 Zed Git 版本 |
| UI 无头验收 | Headless UI acceptance | 在 GPUI 测试窗口中检查布局、焦点、输入和回调 | 不代表真实 Windows 窗口截图或键鼠操作 |

## 结论

迁移技术上可行，但 `gpui-kit` 不是当前依赖的单行替换。功能扩展主线已阶段性收敛，后续开发以 UI/功能缺陷修复和 `gpui-kit` 迁移为主；可复现的 UI/功能缺陷优先于迁移切片处理，迁移本身按独立批次提交和验收。

主要原因：当前项目使用 Zed Git 主干的 `gpui`、`gpui_platform`、`gpui_macros`，以及 Git 版 `gpui-component` 和 `gpui-component-assets`；锁文件中的版本分别为 GPUI 0.2.2、组件库 0.5.1。当前发布版 `gpui-kit` 为 0.6.6，聚合 `gpui-pre` 0.3.6、`gpui-base` 0.6.6、`gpui-component` 0.6.6、`gpui-kit-assets` 和 `gpui-pre-platform`。这同时改变了包来源、版本族和库路径。

## 现状证据

- 根 `Cargo.toml` 在 GPUI 配置中直接声明 3 个 Zed Git 包，并直接声明 2 个 `gpui-component` Git 包。
- 15 个 workspace crate 直接依赖 `gpui`，15 个 workspace crate 直接依赖 `gpui-component`；当前源码中分别有 439 个文件出现 `gpui::`/`use gpui`，335 个文件出现 `gpui_component::`/`use gpui_component`，迁移需要覆盖源码、测试和 Cargo manifest。
- `ramag-bin` 直接调用 `gpui_platform::application()`；`ramag-ui` 的 `RamagAssets` 直接回退到 `gpui_component_assets::Assets`，这两个入口需要分别映射到 `gpui_kit::platform` 和 `gpui_kit::assets`。
- workspace 已经钉定 `lsp-types 0.97.0` 和 `ropey 2.0.0-beta.1`，与 GPUI Kit 0.6.6 的公开依赖版本一致，输入编辑器类型冲突风险低于旧版本迁移。

## 兼容性验证

在仓库外建立临时最小 crate，仅依赖：

```toml
gpui-kit = { version = "0.6.6", features = ["tree-sitter-languages", "test-support"] }
```

验证结果：

- `gpui_kit::{Render, Window, Context, ParentElement}`、`gpui_kit::component::{Root, Input, InputState, h_flex, v_flex}` 可以编译。
- `cargo test --no-run --locked` 可以编译 `#[gpui_kit::test]` 测试宏。
- `cargo test --locked facade_test -- --nocapture` 通过 1 项测试。
- 该探针只证明 facade 的基础 API 和测试支持可用，没有证明 Ramag 全 workspace 已完成迁移；workspace 迁移必须重新运行各工具的 headless 测试和真实服务回归。

官方安装文档也将 `gpui-kit` 定义为应用侧的聚合依赖：应用不再直接列出 GPUI，组件、Base、Assets 和 Platform 分别从 `gpui_kit::component`、`gpui_kit::base`、`gpui_kit::assets` 和 `gpui_kit::platform` 访问。详见 [GPUI Kit Installation](https://gpui-kit.com/docs/installation/)。

## 迁移影响

### 依赖配置

根 workspace 可以收敛为一个 `gpui-kit` 依赖，并打开现有的 `tree-sitter-languages`；测试 crate 使用 `test-support`。显式的 `gpui_platform`、`gpui_macros` 和 `gpui-component-assets` 依赖可以删除，平台 feature 由 Kit 的匹配 `gpui-pre-platform` 统一提供。

### 源码命名空间

应用源码需要把 GPUI 类型、宏和组件入口迁移到 facade：

- `gpui::...` -> `gpui_kit::...`，测试宏迁移到 `#[gpui_kit::test]`。
- `gpui_component::...` -> `gpui_kit::component::...`。
- `gpui_platform::application()` -> `gpui_kit::platform::application()`。
- `gpui_component_assets::Assets` -> `gpui_kit::assets::Assets`。

仅在 crate 根部建立 `gpui` 别名可以降低第一类路径的改动量，但不能让所有子模块继续无条件解析 `gpui_component::`；因此不建议用隐式别名掩盖组件命名空间迁移。

### 版本与行为回归

从 Git 版组件库 0.5.1 到发布版 0.6.4 属于跨小版本升级，需要重新检查 `InputState`、`Root`、菜单/弹窗、图标 assets、Tree-sitter、窗口平台初始化和测试 API。GPUI Kit 的 0.6.x 发布说明包含 headless UI testing、编辑器输入行为和组件交互变化，不能只以编译通过作为完成条件，详见 [GPUI Kit Releases](https://github.com/longbridge/gpui-kit/releases)。

## 推荐实施顺序

1. 建立单独的迁移分支和锁文件变更，只修改 workspace 依赖及命名空间，不加入业务功能。
2. 先迁移 `ramag-ui`、`ramag-bin`、`ramag-terminal` 和共享测试基础设施，再迁移各工具 crate；每一批保持 `cargo check`、fmt、Clippy 和源码行数检查通过。
3. 运行全部 GPUI headless 测试，重点复核 360/640/1024/1440px 布局、输入焦点、弹窗、滚动、菜单和快捷键。
4. 在本机 Docker 环境复跑 API、MQTT、数据库和容器工具的协议/服务集成测试；GPUI 迁移不能改变这些领域行为。
5. 构建 `ramag-bin` 的 Windows MSVC 版本，并补充真实窗口启动、主题切换、文件对话框、资产加载和至少一组 API/MQTT 操作证据。
6. 迁移验收通过后再删除旧 Git 依赖，避免在编译失败时留下两套 GPUI 类型或重复资产包。

## 决策

当前决策：**正式进入后续开发主线；先处理可复现 UI/功能缺陷，再按独立切片推进 `gpui-kit` 迁移**。

收益是依赖入口更简单、GPUI 组件版本族由 Kit 统一管理、后续升级路径更清晰。代价是 15 个 crate 的依赖配置调整、数百个源码文件的命名空间迁移，以及从 Git 版 `gpui-component` 0.5.1 到发布版 0.6.6 的 UI 行为回归。当前切换按 `GPUI-KIT-001` 主线执行；任何业务功能修复仍单独提交，不与依赖迁移混合。
