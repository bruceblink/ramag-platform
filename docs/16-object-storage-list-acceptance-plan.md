# 对象存储账号列表与 RustFS 数据源验收

> 切片：`A-POST-0.5-OBJECT-001`
> 日期：2026-10-17
> 基线：`0e55b681`；当前分支 `main`
> 状态：待恢复（当前优先处理工具入口交互优化）
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

## 回滚边界

回滚本切片新增的账号列表状态修复、对应 UI/服务回归和本计划记录。只清理本切片创建的 RustFS 临时容器与隔离测试数据。保留用户账号、用户凭据、已有 Docker 容器和数据卷。
