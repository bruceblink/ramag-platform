# DBClient 数据源列表状态复核与修复

> 切片：`A-POST-0.5-DBCLIENT-001`
> 日期：2026-10-12
> 基线：`8b65421f`；数据源首页主要布局已由 `c4c20d06` 实现
> 状态：代码与 headless 验收完成；真实窗口验收待补

## 问题与范围

读取连接列表失败时，页面保留旧连接并显示失败原因和重试入口。但搜索没有匹配旧连接时，渲染分支只显示无匹配提示，隐藏读取失败及重试。用户不能区分无匹配与未能刷新。

本切片统一旧列表上方的失败通知，使它在有匹配和无匹配两种结果下都可见。保留搜索条件、旧连接、稳定连接 ID、固定表头与虚拟列表。复核行操作命中，确认编辑、同步和删除请求不会误触发打开连接。

## 验收条件

- 有旧连接但当前搜索无匹配时，失败原因、重试、无匹配提示和搜索框均保留。
- 搜索改变后重新匹配旧连接时，失败通知仍可见；状态区不遮挡表头或行操作。
- 错误信息过长时，页面显示长度受限；重试仍可操作，悬停可查看完整原因。
- 初次加载、空列表、非空列表与读取失败保持可区分；重试调用现有刷新路径。
- 明暗主题、16px/18px、360x640/1024x768/1440x900 下，状态、搜索和主要操作可达。
- 对应 headless 交互、`ramag-tool-dbclient` 全量测试、workspace fmt/严格 Clippy、源码尺寸、完整构建及 LF/diff 检查通过。

## 验证入口

```powershell
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-dbclient','--lib','--','--test-threads=1')
cargo fmt --all -- --check
& .\scripts\windows\invoke-cargo-msvc.ps1 -DenyWarnings -CargoCommand clippy -CargoOptions @('--locked','--workspace','--all-targets')
& .\scripts\windows\check-source-size.ps1
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand build -CargoOptions @('--locked','-p','ramag-bin')
git diff --check
```

不改变数据库协议、连接保存或凭据；Docker 集成不适用。Computer Use 在上一轮被用户通过 Escape 停止，本轮不继续窗口输入；真实窗口验收保留未完成，headless 不替代该证据。

## 验收记录（2026-10-12）

- DBClient lib 测试：375 passed，0 failed；包括无匹配时保留失败通知、长错误仍能访问重试、重试连续失败后保留搜索和旧连接、成功重试后清除失败状态，以及筛选和重排后的行操作身份。
- `cargo fmt --all -- --check`：通过。
- MSVC workspace all-target 严格 Clippy（`-D warnings`）：通过。
- Windows 源码尺寸检查：通过。
- MSVC `cargo build --locked -p ramag-bin`：通过。
- `git diff --check` 和本轮修改文本的 UTF-8/LF 检查：通过。
- Docker 集成：不适用；本切片不改变数据库协议或外部服务。
- Computer Use 真实窗口检查：未执行；真实窗口验收仍待补，headless 结果不作为替代。

错误提示最多直接显示 240 个 Unicode 字符，末尾用省略号标记；悬停提示保留完整错误文本。实现只改变连接列表展示和测试，不改变存储内容。

## 回滚边界

只回滚数据源列表状态渲染、对应测试和本切片文档。不修改用户连接、配置、`v0.5.0` 标签或已发布资产。
