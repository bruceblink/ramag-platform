# GPUI Kit 0.7.3 升级与 Ramag 0.6.0 发布计划

> 切片：`A-POST-0.5-GPUI-KIT-001`
> 日期：2026-10-17
> 状态：已完成
> 基线：`main` 与 `origin/main` 均为 `9282af5c`；Ramag 当前发布版为 `v0.5.0`；工作区依赖 `gpui-kit 0.6.6`
> 目标：升级至上游最新稳定版 `gpui-kit 0.7.3`，完成兼容修复和发布验证，再发布 Ramag `v0.6.0`

## 问题证据

上游 GPUI Kit 已发布 `0.7.3`。`0.7.2` 已撤回，`0.7.3` 使用 `gpui-pre 0.3.9` 快照。GPUI Kit `0.7.0` 改为由 Base `Root` 管理窗口浮层，并移除了手动绘制对话框、Sheet 和通知层的 API。Ramag 当前仍在 Shell 和多个 UI 测试中手动绘制这些层。上游还列出表格选择、日期控件、图表、文本视图和输入组件的兼容变化。

上游依据：[GPUI Kit v0.7.3 Release](https://github.com/longbridge/gpui-kit/releases/tag/v0.7.3)、[GPUI Kit v0.7.0 Breaking Changes](https://github.com/longbridge/gpui-kit/releases/tag/v0.7.0)。当前发布版是 `v0.5.0`。该版本之后的主线已包含工具入口管理和 S3 兼容存储，因此本次应用版本提升到 `0.6.0`。

## 实施范围

- 将 workspace 的 `gpui-kit` 依赖升级到 `0.7.3`，保留 `tree-sitter-languages` 和测试支持功能。
- 按锁文件将 GPUI 快照依赖解析到 `gpui-pre 0.3.9`。不启用可选 `gpui-fast` 后端。
- 迁移编译或行为不兼容的 GPUI Kit 调用。由新 Root 管理窗口浮层，删除重复的手动浮层渲染；保留现有对话框、通知和选择行为。
- 审查依赖升级对文本编辑器、TextView、主题、Popover、表格和窗口尺寸的影响。修复升级造成的行为变化，不借此扩大工具功能。
- 将 workspace 产品版本升级到 `0.6.0`，更新锁文件与发布文档。生成从 `v0.5.0` 到 `v0.6.0` 标签的双语发布说明。
- 在验证通过后提交并推送 `main`，创建注释标签 `v0.6.0`。由现有 Desktop Release 工作流完成 Windows、macOS、Linux 打包、安装产物校验和 GitHub Release 发布。

## 验收条件

- `gpui-kit` 为 `0.7.3`；GPUI、Base、Component 和 Assets 解析为兼容的单一版本族；`Cargo.lock` 可用 `--locked` 构建。
- 所有旧版浮层 API、编译错误、Clippy 告警和测试失败均已处理。对话框、通知、Shell、主题、文本输入和工作区导航保持可用。
- 除 `ramag-tool-system` 外的 workspace 测试、格式、严格 Clippy、完整程序 Debug/Release 构建、源码尺寸和差异检查通过。System 工具测试使用用户已确认的既有结果，本轮不重复运行。
- 使用本机 Docker 执行外部服务集成测试；记录镜像、端口、健康状态和测试数据清理结果。用户已确认通过的 System 工具测试不重复运行。
- 完整程序在真实 Windows 窗口启动。优先使用 Computer Use 验收首页、设置、窗口浮层、输入控件、API 工作区和工具入口。若原生窗口接口不可用，则记录有界启动检查和 headless 测试结果，并明确真实窗口操作未验证。
- `v0.6.0` 标签触发的 GitHub Actions 三平台打包、安装器烟测、五个安装包、`SHA256SUMS.txt` 和 `update-manifest.json` 全部通过。GitHub Release 显示正式发布，标签和 Release 版本一致。
- `main` 与 `origin/main` 保持一致且工作区干净；`v0.6.0` 标签指向已验证的发布源码提交。若发布后补充工作流和资产证据，只允许使用单独的文档提交。

## 当前验证记录

- GPUI Kit：锁文件解析为 `gpui-kit 0.7.3` 和 `gpui-pre 0.3.9`；`cargo tree --locked -p ramag-bin -i gpui-kit` 只显示一个 Kit 版本。
- Rust 检查：最终源码的格式检查、workspace 严格 Clippy、完整 workspace 测试（排除用户已确认通过的 `ramag-tool-system`）以及 Debug 和 Release 构建均已通过。Release 构建使用 `--locked --release -p ramag-bin`，耗时 15 分 20 秒。
- Docker：测试容器均为 healthy。MySQL 使用 `mysql:8.4`，实际版本 `8.4.9`（`127.0.0.1:13306`）；PostgreSQL 使用 `postgres:17-alpine`，实际版本 `17.10`（`127.0.0.1:15432`）；Redis 使用 `redis:7-alpine`，实际版本 `7.4.11`（`127.0.0.1:16379`）；MongoDB 使用 `mongo:8.2`，实际版本 `8.2.12`（`127.0.0.1:27018`）；API 代理镜像为 `ramag-api-proxy-test:python-3.12.11-alpine-3.22`（`127.0.0.1:18094`）；gRPC 镜像为 `ramag-api-grpc-test:rust-1.91.0-bookworm`（`127.0.0.1:18090` 和 `127.0.0.1:18092`）。测试复用已有容器，没有启动、重建或停止容器。
- Redis 测试数据：为完整扫描测试补入 `ttl:1` 至 `ttl:1000`，每个键的值为 `expires-<序号>`，过期时间为 8 小时。完整测试通过后逐项核对这些值并删除这 1,000 个测试键。DB0 恢复为 45,014 个键，`ttl:*` 数量为 0。
- Windows 启动检查：最终 Debug 程序在隔离目录 `target/ui-test-data-v0.6.0` 启动；进程保持运行，窗口标题为 `Ramag`，Windows 报告窗口有响应。Computer Use 未提供原生窗口接口（应用清单为空，`listApps` 和 `getApp` 不可用），因此首页、设置和 API 分隔栏的真实鼠标操作尚未验证；GPUI 组件和工作区 headless 测试已通过。

## 发布结果

- 已提交并推送发布源码：`c086d1723f36b1759b01719e70affbb9901a9eda`（`build: upgrade gpui-kit to 0.7.3`）。注释标签 `v0.6.0` 指向该提交。
- [Desktop Release workflow 39010687567](https://github.com/bruceblink/ramag-platform/actions/runs/39010687567) 的 Windows、macOS、Linux 和发布任务全部成功。
- [Ramag v0.6.0 GitHub Release](https://github.com/bruceblink/ramag-platform/releases/tag/v0.6.0) 已正式发布，非草稿且不是预发布版。
- Release 包含五个安装包：Windows x64 安装器、macOS arm64 和 x86_64 DMG、Linux amd64 DEB 和 x86_64 AppImage；另有 `SHA256SUMS.txt` 和 `update-manifest.json`，共七个资产。
- 已下载并解析摘要文件与更新清单。五个安装包的 SHA-256 同时匹配 `SHA256SUMS.txt`、更新清单和 GitHub 资产摘要；五个安装包的文件大小与清单和 GitHub 资产元数据一致。两个元数据文件的 SHA-256 与 GitHub 资产摘要一致；发布说明与本地按 `v0.5.0..v0.6.0` 生成的双语说明一致。
- 标签推送前 `main` 与 `origin/main` 均为 `c086d172` 且工作区干净。发布后的验收更新只修改文档；文档提交推送后，`main` 与 `origin/main` 保持一致，标签继续指向发布源码提交 `c086d172`。

## 验证命令

```powershell
cargo update -p gpui-kit --precise 0.7.3
cargo fmt --all -- --check
& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked','--workspace','--all-targets')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','--workspace','--exclude','ramag-tool-system','--all-targets','--','--test-threads=1')
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand build -CargoOptions @('--locked','--release','-p','ramag-bin')
& .\scripts\windows\check-source-size.ps1
git diff --check
```

CI 已独立构建和打包 Windows、macOS 与 Linux Release。工作流验证通过后生成并发布全部安装资产。

## 回滚边界

若后续发现问题，只回滚或修复 GPUI Kit 兼容迁移及相关源码。保留 `v0.5.0` 和已发布的 `v0.6.0` 标签及资产；不得移动或覆盖已发布标签。后续修复使用新的版本发布。
