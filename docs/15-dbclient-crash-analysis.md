# DB-CRASH-001：显示查询结果时修复 Windows 栈溢出

## 设计确认（2026-09-26）

- 问题证据：点击本机 MySQL `ramag_ui_test.bulk_records` 后，查询和总数读取均成功，但原生进程报告主线程栈溢出并退出。将 Windows `ramag.exe` 主线程栈预留提高到 4 MiB 后，同一查询和 10,000 行分页浏览可持续运行。
- 设计：仅为 Windows `ramag` 可执行文件将主线程栈预留设为 4 MiB，避免完整 GPUI 数据库结果界面的布局和绘制调用链耗尽默认栈；保留现有查询分页、结果条数和内存上限，不通过缩减结果集掩盖崩溃。
- 改动范围：`ramag-bin` Windows 链接参数；不改变数据库查询、结果格式、非 Windows 平台构建或 Docker 数据。
- 验收条件：普通 `cargo build --locked -p ramag-bin` 产物可启动；使用 Computer Use 在真实窗口连接本机 MySQL 8.4 容器并点击 `bulk_records` 后，原生进程持续运行且结果行可见；通过 dbclient 结果网格测试、workspace fmt、Clippy、源码尺寸和 diff 检查。

## 验收记录（2026-09-26）

- 根因：崩溃日志缺少 Rust panic 记录；从终端启动时复现为 `thread 'main' has overflowed its stack`。以 `/STACK:4194304` 手动链接的 Windows 可执行文件可显示相同结果网格，因此将修复落实在 Windows 链接配置，而不是更改查询或静默减少返回行数。
- 实现与构建：Windows MSVC 输出 `cargo:rustc-link-arg-bin=ramag=/STACK:4194304`；GNU Windows 目标使用对应的 `-Wl,--stack,4194304`。`cargo build --locked -p ramag-bin` 通过，构建脚本输出确认本机 MSVC 目标采用 4 MiB 预留。
- 真实窗口：Computer Use 启动该普通 Cargo 构建产物，连接既有 `127.0.0.1:13318` MySQL 配置并点击 `ramag_ui_test.bulk_records`。结果网格显示 10 列和行数据，分页显示 `1-100 of 100000`；随后将每页大小设为 10,000 并进入第二页，显示 `10001-20000 of 100000`。原生进程在操作后持续运行。
- 本机 Docker：服务 `ramag-visual-test-mysql84`，镜像 `mysql:8.4`，端口 `127.0.0.1:13318 -> 3306/tcp`，验收时容器处于运行状态；根据既有测试数据库保留要求未停止容器或删除数据卷。
- Headless 与质量检查：`cargo test --locked -p ramag-tool-dbclient --lib --quiet`（336 项通过）、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`scripts/windows/check-source-size.ps1`、`git diff --check`、`cargo build --locked -p ramag-bin` 全部通过。
- 未覆盖：本次没有在 macOS 或 Linux 原生窗口验证；Windows GNU 链接参数已配置，但本机只验证 MSVC 工具链。
