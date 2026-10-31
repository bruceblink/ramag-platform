# Ramag 0.8.0 发布计划

> 日期：2026-10-31
> 状态：准备中
> 版本决定：`0.7.0` → `0.8.0`
> 基线：`v0.7.0..main`；目标标签 `v0.8.0`

## 评估结论

本次发布使用次版本 `0.8.0`。`v0.7.0` 之后包含九批保留工具的编译期第一方插件迁移、查询和同步取消流程、多个工具的用户界面维护、共享动画收口以及可选原生 UI 性能采样。变更范围超过补丁修复，仍保持 `0.x` 版本约定，不提升主版本。

## 发布范围

- 系统监控、API、剪贴板、版本管理、SSH、对象存储、容器、数据库客户端、Kafka 和 MQTT 的静态第一方插件装配。
- 数据库查询取消、取消历史、同步状态、设置状态、空状态、失败状态和结果摘要维护。
- 首页和共享工作区 UI 视觉维护、流畅度收口和可选 UI profiler；正常构建和正常启动不启用 profiler。
- WSL Docker 开发说明、真实窗口验收记录、Linux/macOS GitHub CI 检查和 Windows MSVC 构建记录。

## 完成条件

1. 根 `Cargo.toml`、workspace crate 元数据、`Cargo.lock`、README 和 `CHANGELOG.md` 对齐 `0.8.0`。
2. 版本提交的 Desktop CI 三个平台任务通过，且包含 opt-in UI profiler 编译检查；Linux 本机只使用 WSL 编译，Linux/macOS 测试不在本机执行。
3. 创建指向已验证提交的带注释 `v0.8.0` 标签并推送；标签触发 `Desktop Release` 的 Windows x64、macOS arm64/x86_64 和 Linux amd64 打包任务全部通过。
4. GitHub Release 正式发布五个安装包、`SHA256SUMS.txt` 和 `update-manifest.json`；包名、版本、大小和 SHA-256 与清单一致。
5. Release 正文由 `scripts/generate-release-notes.sh v0.8.0 bruceblink/ramag-platform` 生成，包含 `v0.7.0...v0.8.0` 比较链接；不把未完成设计写作已交付功能。

## 验证命令

```powershell
cargo fmt --all -- --check
git diff --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets -- --test-threads=1
cargo build --locked --release -p ramag-bin
./scripts/windows/check-source-size.ps1
```

Linux 本机只执行 WSL `cargo build --locked --release -p ramag-bin` 或目标示例编译，不执行 Linux 测试。Linux/macOS 测试由 `.github/workflows/desktop-ci.yml` 执行。发布打包由 `.github/workflows/desktop-release.yml` 执行。当前版本切片没有新增服务集成测试；后续集成测试统一使用 WSL `Ubuntu-24.04` 的 Docker Engine。

## 回滚边界

标签推送前，只回滚版本、变更记录、发布计划和本次发布提交。标签推送后不移动或覆盖公开标签；若发现缺陷，使用新提交和新版本修复。已发布安装包、校验清单和更新清单保留作为公开记录。

## 验收记录

发布前填写最终 Desktop CI run、Desktop Release run、五个安装包名称、大小、SHA-256、`SHA256SUMS.txt` 和 `update-manifest.json` 的核对结果。未完成的数据库专项或外部服务验证单独记录，不转写为本版本已完成。
