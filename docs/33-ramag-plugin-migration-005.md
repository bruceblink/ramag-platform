# RAMAG-PLUGIN-MIGRATE-005：对象存储插件迁移

> 状态：已完成静态适配与代码验证。
>
> 日期：2026-10-29。

本批次把现有 `ramag-tool-object-storage` 登记为显式第一方静态插件。`ObjectStorageTool`、
`ObjectStorageService`、COS/OSS/S3 兼容驱动、账号存储、对象列表和上传下载工作台保持原有组合路径。

## 固定边界

- 插件 ID 和入口 ID 保持为 `object_storage`，显示名称保持为“云存储”。
- 插件 API 版本为 `1.0`，只声明 `ui.entry`，不新增插件设置、文件、网络或凭据能力。
- 连接、账号读取、对象列表、上传、下载和删除仍由现有宿主服务与用户操作控制；本批次不改变协议、
  凭据、加密存储、请求上限或传输取消行为。
- 注册或初始化失败仍由 `StaticPluginHost` 隔离；其他工具注册、初始化和工具顺序保持不变。

## 实现

- 在 `ramag-bin` 组合根中使用显式对象存储插件描述替换通用 `StaticPluginAdapter::from_tool` 路径。
- 描述固定对象存储插件和入口的稳定 ID、中文名称、用途说明与最小 `ui.entry` 能力。
- 增加描述模型回归测试，确认入口、能力和设置列表不会意外变化。

## 验证

- `cargo fmt --all -- --check`
- `cargo test --locked -p ramag-bin`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

本批次只改变静态插件描述和组合根登记，不增加页面入口或改变对象存储行为；不需要新增真实窗口流程。
既有对象存储页面和连接验收证据继续有效。

## 回滚边界

只回滚对象存储插件描述、组合根登记、专项测试和本记录；不回滚对象存储账号、凭据、网络驱动、
用户数据或其他插件迁移批次。
