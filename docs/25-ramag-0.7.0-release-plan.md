# Ramag 0.7.0 发布计划

> 日期：2026-10-25
> 状态：准备中
> 基线：`v0.6.0..main`；发布标签 `v0.7.0`

## 范围

- 发布自 `v0.6.0` 后已合入 `main` 的首页、API、数据库、滚动条和维护修复。
- 明确本机协作和独立 JSON Path 提取器已移除；其余现有工具继续保留。
- 将 workspace 版本、锁文件、变更记录和 README 对齐到 `0.7.0`。
- 复用 `Desktop Release` 工作流构建 Windows x64、macOS arm64/x86_64 和 Linux amd64 安装包。

## 完成条件

1. `main` 的 Desktop CI 三个系统任务通过；发布源码的格式、严格 Clippy、目标测试、完整程序构建、源码尺寸和差异检查通过。
2. `Cargo.toml`、本地 crate 的 `Cargo.lock` 版本和注释标签均为 `0.7.0`；标签指向已验证且已推送的源码提交。发布说明由 `v0.6.0..v0.7.0` 提交范围生成。
3. 标签触发的 `Desktop Release` 三个平台打包和发布任务通过。正式 Release 包含五个安装包、`SHA256SUMS.txt` 和 `update-manifest.json`。
4. 核对五个安装包的名称、版本、大小和 SHA-256 与清单及 GitHub 资产元数据一致；Release 为正式发布，标签、提交和更新清单一致。
5. 记录本机窗口、headless、Docker、跨平台 CI 和发布资产各自的证据。数据库 `A-UI-REAL` 的剩余验收不写作已完成。

## 验证命令

```powershell
cargo fmt --all -- --check
& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked','--workspace','--all-targets')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','--workspace','--exclude','ramag-tool-system','--all-targets','--','--test-threads=1')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand build -CargoOptions @('--locked','--release','-p','ramag-bin')
& .\scripts\windows\check-source-size.ps1
git diff --check
```

## 回滚边界

标签推送前，只撤销本切片的版本、锁文件和发布文档改动。发布后保留公开标签与资产；如发现缺陷，以新提交和新版本修复，不移动已发布标签或覆盖公开安装包。

## 验收记录

- 本机源码：`cargo fmt --all -- --check`、workspace 严格 Clippy、排除已单独验收的 `ramag-tool-system` 后的 workspace 测试、`cargo build --locked --release -p ramag-bin`、Debug 构建、源码尺寸和 `git diff --check` 均通过。Release 构建耗时 13 分 11 秒。
- 本机打包逻辑：ShellCheck 与 `make linux-package-test` 通过；37 个本地 crate 的锁文件版本均为 `0.7.0`，第三方依赖未因本次版本更新而改变。
- 真实 Windows 窗口：使用隔离数据目录运行当前源码的 Debug 程序。首页显示 10 个工具；API、数据库和系统设置页面可打开。数据库页为 0 个测试数据源。未创建连接、使用凭据或执行数据库写入。Release 程序不支持 Debug 专用的数据目录覆盖，因此本次未让本地 Release 程序访问用户默认数据库；正式包的启动和安装由 CI 烟测单独验证。
- Docker：不适用。本切片只修改版本和发布文档，没有改变数据库或外部服务协议；此前专项的 Docker 证据不充作本次新增集成测试。
- 发布前 CI：`0067968f` 对应的 [Desktop CI run 40200881510](https://github.com/bruceblink/ramag-platform/actions/runs/40200881510) 已在 Windows、macOS、Linux 通过。最终版本提交的 CI 和标签发布任务仍待运行。
