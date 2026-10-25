# Ramag 0.7.0 发布计划

> 计划日期：2026-10-25
> 完成日期：2026-10-26
> 状态：已完成
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
- 发布质量门：`6dfb311d` 对应的 [Desktop CI run 40207442289](https://github.com/bruceblink/ramag-platform/actions/runs/40207442289) 中 Windows、macOS、Linux 全部通过。
- 预演：首次 [Desktop Release artifact run 40205269279](https://github.com/bruceblink/ramag-platform/actions/runs/40205269279) 的 Linux 包构建因 AppImage `continuous` 资产更新、SHA-256 不匹配而失败。脚本改为使用官方 `20251108` 标签资产，固定 SHA-256 为 `2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d`；下载的 944,632 字节运行时实际摘要与官方资产元数据一致。ShellCheck 与 `make linux-package-test` 通过。随后 [Desktop Release artifact run 40207466952](https://github.com/bruceblink/ramag-platform/actions/runs/40207466952) 的 Windows、macOS、Linux 打包和测试全部通过；正式发布 job 按空 `release_tag` 配置跳过。下载这次预演的三个 Artifact 后，五个安装包的摘要均与各自 `SHA256SUMS.txt` 匹配。
- 正式发布：`v0.7.0` 是注释标签，指向 `6dfb311d8f504f8c67baeac55e2846fa91c65971`。[正式 Desktop Release run 40237074238](https://github.com/bruceblink/ramag-platform/actions/runs/40237074238) 的 Linux、Windows、macOS 构建及 GitHub Release 发布 job 全部成功。GitHub 于 2026-10-26 06:40（香港时间）发布 [Ramag 0.7.0](https://github.com/bruceblink/ramag-platform/releases/tag/v0.7.0)；Release 不是草稿，也不是预发布版本。
- 发布说明：GitHub Release 正文与 `scripts/generate-release-notes.sh v0.7.0 bruceblink/ramag-platform` 的输出一致，包含 `v0.6.0...v0.7.0` 比较链接。
- 正式资产：从 Release 下载 `SHA256SUMS.txt` 和 `update-manifest.json`，核对版本、标签、五个安装包的名称、摘要和大小。五个安装包的 SHA-256 同时匹配两份清单与 GitHub 资产摘要；两份清单文件自身的 SHA-256 和大小也匹配 GitHub 资产元数据。

| 文件 | 字节 | SHA-256 |
|---|---:|---|
| `Ramag-0.7.0-windows-x64-setup.exe` | 24005642 | `e9779c6067e5d899e41c0a4b6635878f37de9354c2134a8c9da6bca198a1f1ec` |
| `Ramag-0.7.0-macos-arm64.dmg` | 22850754 | `08a8b6307ca7611a97c42e51d19295f6fff127f9a228c2d4a2d5becbfd9d381e` |
| `Ramag-0.7.0-macos-x86_64.dmg` | 27153372 | `9dea5d04a3ebcfe4b6314dd0c319437493ab1520603a2b6868551c2df9dafe86` |
| `Ramag-0.7.0-linux-amd64.deb` | 24882132 | `a0e04f78bc0a346028b83e5f9f6657bb8723c1509cc0b4b3ef5f6beb736b7017` |
| `Ramag-0.7.0-linux-x86_64.AppImage` | 38267384 | `bbef6cff4548f90d0b858e6f6f07af8339db40745f436a3f5b3aeb11df9978d2` |
| `SHA256SUMS.txt` | 483 | `d4543d9f16223d192eac517d11bf7b77f930daa26ad4141beb5965620a31052a` |
| `update-manifest.json` | 5945 | `7f49670e8c214826ccdfba051bae8bfcb5ce2d8609161a76137f6eaf89849098` |
