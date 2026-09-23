# GPUI Kit 依赖迁移结果与后续开发基线

记录日期：2026-09-23

## 术语表与命名约定

| 规范名称 | English / Identifier | 当前方案中的职责边界 | 不代表什么 |
|---|---|---|---|
| GPUI 运行时 | GPUI / `gpui` | 提供窗口、元素、事件、状态和测试运行时；应用代码通过 `gpui_kit` 使用 | 不代表独立的应用依赖入口 |
| GPUI 组件入口 | Component facade / `gpui_kit::component` | 提供 `Input`、`Root`、布局辅助函数、主题和组件状态 | 不代表仓库仍直接依赖 Git 版 `gpui-component` |
| GPUI Kit | GPUI Kit / `gpui-kit`, `gpui_kit` | 聚合匹配版本的 GPUI、Base、Component、Assets 和 Platform，作为应用唯一 UI 依赖入口 | 不代表旧 `gpui` Git 包的兼容别名 |
| 发布版 GPUI 家族 | Published GPUI family / `gpui-pre-*` | 由 GPUI Kit 0.6.6 统一管理的底层版本集合 | 不代表应用需要直接声明这些底层包 |
| UI 无头验收 | Headless UI acceptance | 在 GPUI 测试窗口中检查布局、焦点、输入、滚动和回调 | 不代表真实 Windows 窗口截图或键鼠操作 |

## 当前结论

GPUI Kit 迁移已在提交 `344c5b36` 完成。后续 UI 和工具功能开发统一以 `gpui-kit` 为主线；新增组件、测试入口和平台能力都从 `gpui_kit` 访问。可复现的 UI/功能缺陷继续单独修复，不与依赖升级混合。

当前已确认的结果：

- 根 `Cargo.toml` 只声明发布版 `gpui-kit = 0.6.6`；workspace crate 通过 `gpui-kit = { workspace = true }` 使用 UI 类型。
- 旧的 `gpui`、`gpui_platform`、`gpui_macros`、`gpui-component` 和 `gpui-component-assets` 不再是 workspace 的直接依赖。
- `Cargo.lock` 中仍出现 `gpui-component` 属于 `gpui-kit` 的传递依赖，这是发布版组件实现的一部分，不应再次添加为应用直接依赖。
- 源码和测试使用 `gpui_kit::...`、`gpui_kit::component::...` 以及 `#[gpui_kit::test]`；不通过别名恢复旧命名空间。

## 验收证据

WSL 原生 ext4 工作区 `/home/likanug/workspace/ramag-platform` 已完成以下检查：

- `cargo test --workspace --lib --quiet`：所有 workspace 库测试套件通过，无失败用例。
- `cargo check --workspace --all-targets`：所有 workspace target 检查通过。
- 提交钩子中的格式、源码尺寸和 Clippy 检查通过。
- Kafka、MQTT、SSH 的视觉和交互回归测试分别通过；近期修复已分别提交为 `c1d87082`、`f6a0a3bc` 和 `2b18f6eb`。
- `scripts/api-test/__pycache__/` 已由 `.gitignore` 忽略，避免测试脚本生成的 Python 字节码进入工作树。

这些证据覆盖编译、库测试和无头 UI 行为。真实 Windows 窗口截图、文件对话框、平台主题切换以及依赖真实数据库或 Broker 的集成测试仍需在对应环境中单独验收，不能用无头测试替代。

## 依赖与命名空间规则

根 workspace 的 UI 依赖保持为：

```toml
gpui-kit = { version = "0.6.6", features = [
    "tree-sitter-languages",
] }
```

应用代码按下面的映射使用：

- `gpui::...` -> `gpui_kit::...`。
- `gpui_component::...` -> `gpui_kit::component::...`。
- `gpui_platform::application()` -> `gpui_kit::platform::application()`。
- `gpui_component_assets::Assets` -> `gpui_kit::assets::Assets`。
- UI 测试宏使用 `#[gpui_kit::test]`，测试支持通过具体 crate 的 `test-support` feature 开启。

依赖升级时只修改根 `gpui-kit` 版本和必要 feature，并重新执行全目标检查、workspace 库测试和真实平台验收。不要为了普通功能修复直接刷新整个 `Cargo.lock`，也不要把 `gpui-component`、`gpui` 或平台底层包重新添加到 workspace manifest。

## 已处理的迁移回归

迁移后的无头测试暴露了几类行为差异，已经按独立提交修复：

- Git 非仓库测试使用系统临时目录，避免 WSL 根目录权限差异：`077c8199`。
- Kafka 窄窗口主题标题取消不适用的满高约束，搜索框保持在标题容器内：`c1d87082`。
- MQTT 本地 Broker 视觉测试先滚动到发布和账号控件，再执行真实点击：`f6a0a3bc`。
- SSH 工作区记录每个连接的目录栏宽度，窗口缩放后恢复用户设置，同时保留最小宽度约束：`2b18f6eb`。

后续发现类似问题时，先补充能复现窗口宽度、焦点、滚动或回调行为的测试，再修改对应 UI；不要只放宽断言。

## 后续开发顺序

1. 复现并修复 UI 或功能缺陷，明确组件、输入、状态变化和失败结果。
2. 为跨窗口尺寸、焦点、滚动、点击和异步结果补充定向测试。
3. 通过目标 crate 测试和 Clippy 后，创建独立提交并推送。
4. 涉及多个 UI crate 或共享组件时，再运行 `cargo test --workspace --lib --quiet` 和 `cargo check --workspace --all-targets`。
5. 只有用户明确要求升级 GPUI Kit 版本时，才创建单独的依赖升级切片，并记录新版本的真实平台验收结果。

## 决策

当前决策：**项目已切换到 `gpui-kit` 主线；后续以 UI/功能缺陷修复为主，依赖升级单独处理。**

任何新 UI 代码都应直接使用 GPUI Kit 的入口和组件，不再复制旧 Git 依赖的类型或建立第二套 UI 适配层。
