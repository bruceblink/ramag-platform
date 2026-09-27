# Ramag 静态插件开发手册

## 术语与命名

| 名称 | English / Acronym | 本手册中的职责 | 不表示什么 |
|---|---|---|---|
| 静态插件 | Built-in Plugin | 编译进 Ramag、通过 Rust 接口注册的工具 | 不表示可从外部目录动态加载 |
| 插件清单 | Plugin Descriptor | 描述身份、API 版本、入口、能力和设置模式 | 不表示已经授予全部权限 |
| 插件宿主 | Static Plugin Host | 按顺序注册、初始化、诊断和关闭静态插件 | 不执行外部不受信任代码 |
| 插件上下文 | Plugin Context | 在生命周期回调中标识插件并检查可用状态和能力授权 | 不提供对 Shell 或 GPUI 内部状态的直接访问 |

本手册适用于当前 `bruceblink/ramag-platform` 的静态插件 API。动态插件、插件市场、外部插件进程和跨语言 ABI 尚未实现。

## 能力范围

插件必须使用稳定的小写 ASCII `PluginId`，入口 ID 必须与工具元数据 ID 一致。宿主当前接受的能力只有 `ui.entry`、`ui.notification`、`storage.plugin` 和 `task.scoped`。插件不能直接读取全局配置、其他插件设置、凭据、连接配置或 `ramag-ui` 内部对象。

## 创建工具插件

插件实现 `ramag_domain::Tool`，把工具身份集中在 `ToolMeta`，再用 `StaticPluginAdapter` 接入宿主：

```rust
use std::sync::Arc;
use ramag_app::{StaticPluginAdapter, StaticPluginHost};
use ramag_domain::{Tool, ToolMeta};

struct ExampleTool { meta: ToolMeta }

impl ExampleTool {
    fn new() -> Self {
        Self { meta: ToolMeta::new("example", "Example", "A bounded example tool") }
    }
}

impl Tool for ExampleTool {
    fn meta(&self) -> &ToolMeta { &self.meta }
}

fn register_example(host: &StaticPluginHost) -> Result<(), Box<dyn std::error::Error>> {
    let tool: Arc<dyn Tool> = Arc::new(ExampleTool::new());
    host.register_plugin(Arc::new(StaticPluginAdapter::from_tool(tool)?))?;
    Ok(())
}
```

需要自定义资源清理时，实现 `StaticPlugin` 的 `initialize` 和 `shutdown`。初始化失败只禁用当前插件；宿主继续处理后续插件。关闭按成功初始化的逆序执行，关闭后的 `PluginContext::ensure_available` 会拒绝迟到调用。

## 清单、设置和生命周期

需要声明完整描述时，使用 `PluginDescriptor::new`，再通过 builder 添加说明、能力和设置。设置只允许 `Boolean`、`Integer`、`String`、`Enum` 和 `StringList`，并受数量、长度和枚举值上限约束。API 主版本不同、重复插件 ID、重复入口 ID、未知能力、非法设置键和默认值类型错误都会在注册阶段返回诊断。

`ramag-bin` 在启动时创建 `StaticPluginHost`，注册内置工具后调用 `initialize_all`；退出时调用 `shutdown_all`。插件状态包括 `Registered`、`Initializing`、`Ready`、`ShuttingDown`、`Failed` 和 `Unloaded`。一个插件失败不得阻塞其他插件，也不得让失败插件继续出现在可用工具列表中。

### 能力授予和运行时检查

清单中的能力声明只说明插件需要什么，不能自动获得权限。宿主通过 `PluginPermissionPolicy` 按插件 ID 显式授予能力，再用 `PluginContext::require_capability` 包住每一次受控服务调用。该检查会先验证插件仍处于可用生命周期，再验证能力已在清单中声明且已由策略授予；默认策略不授予任何能力。设置命名空间和持久化迁移仍待 P0-C 后续切片。

### 设置命名空间和运行时快照

宿主使用 `PluginSettingsSnapshot::from_namespaced_values` 读取某个插件的候选设置。完整键必须以 `plugin.<plugin-id>.` 开头，并且键名和值类型必须出现在该插件清单中；其他插件命名空间、未知键、重复键、敏感设置、类型不匹配和超出字符串/列表上限的值都会返回 `PluginSettingsError`。缺失值只使用清单默认值补齐。敏感设置在秘密存储接口完成前不会进入普通快照。

设置修改通过 `update` 或 `update_namespaced` 生成新的快照，旧快照保持不变。这样运行中的插件只会看到通过校验的完整值集，后续原子持久化层可以在替换快照前执行迁移和备份。本切片不写配置文件、不迁移旧版本数据，也不把 `sensitive` 设置交给普通存储；秘密存储接口另行实现。

`PluginSettingsStore` 将快照编码为版本 1 的 JSON 包络，使用 `plugin.<plugin-id>.__snapshot.v1` 保存主记录，并在替换前把旧主记录复制到 `plugin.<plugin-id>.__snapshot.backup.v1`。读取主记录失败时自动校验备份；两者都损坏时返回恢复失败，宿主不得启动受影响插件。备份和主记录只保存普通设置，迁移旧版本与敏感设置仍需后续专用接口。

需要升级旧包络时，宿主通过 `PluginSettingsMigrator` 注册连续的版本步骤。每一步在内存中接收和返回有界的命名空间映射，所有步骤成功且生成的新快照通过清单校验后才可交给插件；迁移函数返回失败、缺少步骤或超过步骤上限都会拒绝启动，并保留主记录以便备份恢复。迁移不会自动写回配置，避免失败流程覆盖可恢复数据。

敏感设置必须使用 `PluginSecretStore`，不能放入普通设置快照。秘密快照只接受清单中 `sensitive` 为真的键，宿主使用 `Storage::seal` 和 `Storage::unseal` 保存到独立的 `plugin.<plugin-id>.__secrets.v1` 密文键，并保留加密备份。密文前缀、包络版本、明文和密文大小都会校验；插件命名空间、普通设置键、类型或格式不匹配会被拒绝，日志和错误不会输出秘密正文。

## 测试和验收

插件至少覆盖清单校验、重复入口、初始化失败隔离、逆序关闭、迟到上下文调用、未声明/未授予能力拒绝、设置命名空间隔离、类型/大小校验、不可变快照更新、主记录备份恢复、双记录损坏拒绝、迁移失败回退和秘密密文读写，以及 Activity Bar 和设置诊断在 360/1024/1440 宽度下的 headless 布局。提交前运行：

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
bash scripts/check-source-size.sh
git diff --check
```

真实 Windows 窗口截图和键盘操作仍需单独完成；headless 测试不能替代真实窗口证据。

## 当前未实现能力

当前版本不能安装或加载第三方动态插件，不能执行插件包中的外部代码，不能从远程市场发现或升级插件，也没有跨语言 ABI、签名验证、沙箱、安装回滚和独立插件进程。需要这些能力时，应先更新 `docs/04-plugin-platform-roadmap.md`，完成威胁模型、权限边界和协议决策，再新增独立实现任务。
