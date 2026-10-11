# System Pulse Disks 与 Network 详细页对齐计划

> 切片：`A-PULSE-GAP-03-DN`
> 日期：2026-10-11
> 范围：System Pulse 详细页的磁盘与网络设备工作区

## 目标

让 Disks 与 Network 详细页使用清晰且稳定的设备选择、主读数、合并趋势和统计分区。保留现有设备身份、选择恢复、缺失状态和采样数据来源。

## 当前差异

- Disks 使用容量作为主量表，但趋势区标题仍为 `Disk transfer rate`。
- Network 将速率、累计字节和 TCP 连接混在一个统计区，累计值没有独立分组。
- 两个页面共用通用渲染函数，缺少可验证的页面分区边界。

## 实施范围

- Disks：保留文件系统选择器和容量主量表；将读写速率放入 `Read / write throughput` 合并趋势区；将 IOPS、延迟等辅助指标放入独立统计区。
- Network：保留接口选择器和接收/发送速率主量表；将接收/发送速率放入 `Receive / transmit throughput` 合并趋势区；将累计 RX/TX 与 TCP 连接放入 `Cumulative traffic` 统计区。
- 为主量表、趋势区和统计区增加稳定的 GPUI 调试选择器，覆盖已选设备、设备重排、已保存设备缺失和窄窗口滚动边界。
- 不修改采集器、驱动协议、设备选择持久化、Summary 卡片或其他详细页的业务语义。

## 验收条件

- `Disks` 和 `Network` 在 `360x640`、`1024x768`、`1440x900` 下保持选择器、主量表、趋势区和统计区边界；内容在页面滚动区域内，不遮挡页头或滚动条。
- Disks 的容量主量表与读写趋势语义一致，Network 的速率趋势与累计统计分别可定位。
- 设备重排后保留稳定选择；选中设备缺失时保留不可用身份和原因，不自动切换到其他设备。
- 目标测试、格式、workspace Clippy、源码尺寸和 `git diff --check` 通过。
- Computer Use 可用时核对参考程序和 Ramag 窗口；当前 CUA 若仍无原生窗口，只记录启动路径和限制，不把 headless 结果写成原生验收。

## 验证命令

```powershell
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-system','--lib','--','--test-threads=1')
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
& .\scripts\windows\check-source-size.ps1
git diff --check
```

## 回滚边界

只回滚本切片的详细页布局、调试选择器、目标回归测试和本计划记录。不改变采集器数据模型、`v0.5.0` 标签及已发布资产。

## 验收记录

2026-10-11：`ramag-tool-system` 全量 112 项测试通过，新增 Disks/Network 主量表、趋势和统计区边界回归通过；`ramag-bin` MSVC 调试构建、workspace 严格 Clippy、fmt、Windows 源码尺寸和 `git diff --check` 通过。已启动 `target/x86_64-pc-windows-msvc/debug/ramag.exe` 并确认窗口标题为 `Ramag — 数据库客户端`；当前 CUA 返回空应用清单，真实窗口鼠标/键盘流程保持待补，不把 headless 结果写成原生验收。
