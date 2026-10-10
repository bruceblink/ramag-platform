# 0.5.0 发布后 UI 与功能修复切片

> 更新日期：2026-10-10
> 范围：剪贴板首页的删除撤销交互回归

## 目标

让删除后的“撤销”测试通过真实通知按钮触发恢复流程。测试必须覆盖恢复成功、恢复失败、当前搜索重新执行和错误通知边界。

## 验收条件

- 删除成功后，通知中的 `clip-undo-delete` 按钮可以被测试窗口定位。
- 点击撤销后，测试存储收到恢复写入，当前搜索重新执行并显示条目。
- 恢复失败时，条目不会被测试存储重新写入，视图保留错误通知。
- 现有删除、搜索和渲染回归测试继续通过。

## 验证命令

```powershell
& .\scripts\windows\invoke-cargo-msvc.ps1 -CargoCommand test -CargoOptions @('--locked','-p','ramag-tool-clipboard','--lib','--','--test-threads=1')
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
& .\scripts\windows\check-source-size.ps1
git diff --check
```

## 回滚边界

只回滚本切片新增的测试辅助状态、撤销测试和本计划记录。不改变 `0.5.0` 标签及已发布资产。
