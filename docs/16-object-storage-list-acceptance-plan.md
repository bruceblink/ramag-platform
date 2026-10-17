# 对象存储账号列表与 RustFS 数据源验收

> 切片：`A-POST-0.5-OBJECT-001`
> 日期：2026-10-17
> 基线：`38bb1ee2`；当前分支 `main`
> 状态：已完成
> 关联：[S3 兼容账号计划](13-object-storage-s3-compatibility-plan.md)、[最新执行计划](01-development-plan.md)、[主线路线图](02-development-roadmap.md)

## 问题与范围

对象存储账号列表已有窄窗口布局、排序、搜索和行操作测试。现有布局测试直接向视图注入测试数据，没有证明 S3 兼容账号能在真实本机 RustFS 数据源下保存、回读并进入列表。长账号名称、排序和筛选后的稳定选择、账号行操作也需要按路线图复核。

本切片验收完整 Windows 程序中的账号创建、持久化、列表显示和 Bucket 工作区连接。使用独立 RustFS 临时容器和隔离的 Ramag 测试数据。复核账号列表在 360、640、768、1024 和 1440 逻辑像素宽度下的布局、明暗主题、长名称、行操作、排序和稳定账号 ID。

测试只使用本机 RustFS，不连接 COS、OSS 或其他远端服务。不得启动、停止、删除或修改已有数据库容器、已有 RustFS 容器和数据卷。临时账号和测试数据不得写入用户配置目录、仓库或日志。

## 验收条件

- 使用 S3 兼容 Endpoint、临时凭据和一次性 Bucket 创建测试账号；保存后账号服务可以重新读取该账号。
- 完整 Windows 程序显示保存的账号、S3 服务商、Bucket 数量和可用行操作；重新启动后账号仍在列表中。
- 选择账号后，工具可通过该账号连接本机 RustFS，并显示真实 Bucket/对象列表。测试对象使用随机名称并在完成或失败后清理。
- 账号搜索、名称排序和供应商排序后，选中的账号仍由稳定账号 ID 标识；编辑、删除确认和选择操作命中正确行。
- 明暗主题及 360、640、768、1024、1440 宽度下，长名称、服务商、只读状态和行操作保持可见、可达且不越界。窄窗口不强行显示 Bucket 计数列。
- 无账号、无匹配项、加载和失败状态均有明确可见表现；长列表仍可滚动到首尾。
- Windows 原生窗口使用隔离数据执行真实点击；headless 测试只作为布局和状态回归证据。
- 记录 Docker Engine 版本、镜像 digest、映射端口、健康结果、测试结果和本次容器清理结果。现有容器和数据卷保持原样。

## 验证命令

- `cargo test --locked -p ramag-tool-object-storage --lib -- --test-threads=1`
- `cargo test --locked -p ramag-infra-object-storage --test live -- s3_compatible_live_round_trip --ignored --test-threads=1`
- `cargo fmt --all -- --check`
- `& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked', '--workspace', '--all-targets')`
- `& .\scripts\windows\check-source-size.ps1`
- `cargo build --locked -p ramag-bin`
- `git diff --check`

## 执行与验收结果

- 本轮使用隔离的 Ramag 数据目录 `$env:TEMP\ramag-object-list-797073d0`。完整程序保存并验证了 1 个 S3 兼容账号及 1 个 Bucket 挂载。关闭并重启程序后，账号仍显示在列表中，服务商为 S3 兼容存储，Bucket 数量为 1。隔离数据库为 `ramag.redb`，大小 548,864 字节；测试账号及其加密凭据仍保留在该临时目录。该目录不在仓库、应用用户配置目录或日志目录中。
- Computer Use 使用真实 Windows 窗口完成验收。对象存储账号页显示账号、服务商、Bucket 数量、编辑和删除操作。搜索账号名后显示匹配行；无匹配搜索显示“暂无匹配”。编辑操作打开该账号表单并保留掩码凭据。删除确认对话框显示同一账号名称和本地影响说明；本轮取消了删除，账号保持不变。
- 完整程序通过本机 RustFS 保存账号后，Bucket 工作区显示真实目录和 `visible-object.txt`（35 字节）。程序重启后再次进入相同目录，文件仍可列出。该文件随后通过 S3 API 删除，并确认测试前缀下剩余对象数为 0。
- `cargo test --locked -p ramag-tool-object-storage --lib -- --test-threads=1`：35 passed、0 failed。布局测试覆盖 360、640、768、1024、1440 宽度；账号行命中区域、长名称、供应商、只读状态和 Bucket 数量边界均有断言。账号管理布局和空状态测试覆盖明暗主题。排序测试覆盖名称与供应商排序、升降序及稳定账号 ID。
- `cargo test --locked -p ramag-infra-object-storage --test live -- s3_compatible_live_round_trip --ignored --test-threads=1`：1 passed。测试覆盖实际列出、上传、拒绝重复上传、读取元数据、下载内容校验、只读写入/删除拒绝、测试对象删除和驱动关闭。
- `cargo build --locked -p ramag-bin`：通过。RustFS 使用 Docker Desktop Engine 29.7.2（linux/amd64）和 `rustfs/rustfs:1.0.1`，镜像 digest 为 `sha256:1803faef57627e2d9c2e7d89d655d712ddded5389040054987163043fecb6a3c`。本轮仅映射 `127.0.0.1:19012 -> 9000`（S3 API）及 `127.0.0.1:19013 -> 9001`（Console）；`/health` 返回 HTTP 200。容器没有配置 Docker healthcheck。
- 测试容器使用 `--rm` 和匿名 `/data` 卷。停止容器后已确认容器和匿名卷均被移除。未启动、停止或修改已有数据库容器、旧 RustFS 容器或其数据卷。Computer Use 窗口和完整程序均已关闭。
- `cargo fmt --all -- --check`、工作区全目标严格 Clippy（`-D warnings`）、`scripts/windows/check-source-size.ps1` 和 `git diff --check`：均通过。提交与推送状态以当前 Git 历史及远端分支为准。

## 回滚边界

回滚本切片新增的账号列表状态修复、对应 UI/服务回归和本计划记录。只清理本切片创建的 RustFS 临时容器与隔离测试数据。保留用户账号、用户凭据、已有 Docker 容器和数据卷。
