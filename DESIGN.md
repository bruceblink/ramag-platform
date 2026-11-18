---
version: alpha
name: Ramag
description: 面向开发和运维人员的本地工具工作台。
colors:
  light-background: "#FFFFFF"
  light-surface: "#FAFAFA"
  light-raised: "#F4F4F4"
  light-hover: "#EEEEEE"
  light-border: "#E4E4E4"
  light-strong-border: "#C9C9C9" # 原型参考值；运行时没有独立的强边框令牌
  light-text: "#171717"
  light-muted: "#656565"
  light-soft: "#737373" # 原型参考值；运行时弱化文字使用 muted
  light-primary: "#171717"
  light-on-primary: "#FFFFFF"
  light-accent: "#175CBB"
  light-success: "#176F46" # 运行时值；原型参考值为 #187448
  light-warning: "#885913"
  light-danger: "#B92525"
  dark-background: "#090909"
  dark-surface: "#101010"
  dark-raised: "#171717"
  dark-hover: "#202020"
  dark-border: "#282828"
  dark-strong-border: "#414141" # 原型参考值；运行时没有独立的强边框令牌
  dark-text: "#EDEDED"
  dark-muted: "#A1A1A1"
  dark-soft: "#858585" # 原型参考值；运行时弱化文字使用 muted
  dark-primary: "#EDEDED"
  dark-on-primary: "#111111"
  dark-accent: "#82B9FF"
  dark-success: "#74D5A2"
  dark-warning: "#E9BF75"
  dark-danger: "#FF9B9B"
typography:
  sans:
    fontFamily: "Inter Variable"
  mono:
    fontFamily: "JetBrains Mono"
  display:
    fontFamily: "Michroma"
rounded:
  DEFAULT: "6px"
  lg: "8px"
spacing:
  workbench-toolbar: "32px"
  workbench-tab: "30px"
  dense-row: "26px"
components:
  page-light:
    backgroundColor: "{colors.light-background}"
    textColor: "{colors.light-text}"
  page-dark:
    backgroundColor: "{colors.dark-background}"
    textColor: "{colors.dark-text}"
  panel-light:
    backgroundColor: "{colors.light-surface}"
    textColor: "{colors.light-text}"
  panel-dark:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-text}"
  divider-light:
    backgroundColor: "{colors.light-border}"
  divider-dark:
    backgroundColor: "{colors.dark-border}"
  button-primary-light:
    backgroundColor: "{colors.light-primary}"
    textColor: "{colors.light-on-primary}"
  button-primary-dark:
    backgroundColor: "{colors.dark-primary}"
    textColor: "{colors.dark-on-primary}"
  focus-light:
    color: "{colors.light-accent}"
  focus-dark:
    color: "{colors.dark-accent}"
  dialog-light:
    backgroundColor: "{colors.light-surface}"
  dialog-dark:
    backgroundColor: "{colors.dark-surface}"
  input-light:
    backgroundColor: "{colors.light-background}"
    borderColor: "{colors.light-border}"
  input-dark:
    backgroundColor: "{colors.dark-background}"
    borderColor: "{colors.dark-border}"
---

# Ramag 设计约定

## 产品与设计方向

Ramag 是供开发和运维人员在本地使用的多工具工作台。用户需要在同一窗口中查找工具、连接服务、编辑请求和查看数据。界面应支持长时间操作，并让数据、当前状态和主要操作保持清楚。

[交互原型](docs/prototypes/ramag-vercel-dashboard.html)是本轮布局与视觉目标。其特点是中性色背景、清晰的文字导航、简洁的顶栏和适合工具内容的密度。API 保留请求与响应分区；数据库保留对象树和结果表。其他工具应保留各自任务所需的信息结构。原型中的数据和模拟操作不代表真实功能。

现有界面以中文为主，代码、协议、路径和技术标识保留原文。目标市场及其他语言支持尚无足够证据，不作推断。工具提示和错误信息应说明对象、结果及可执行的下一步。

## 令牌和迁移状态

上面的颜色表以原型的 `--r-*` 值为基线，并标出运行时没有独立映射的参考值。
`pulse_palette_for_mode` 位于 `crates/ramag-ui/src/pulse_ui.rs`，实际提供背景、表面、抬升表面、边框、文字、弱化文字和选中背景。
悬停背景由 `crates/ramag-ui/src/theme.rs` 的 `list_hover` 映射；选中背景使用 `r-raised`。
强边框和 `r-soft` 目前只作原型参考，不是独立运行时令牌。主题代码还设置状态按钮、链接和键盘焦点颜色。
主按钮使用黑白中性色；蓝色保留给链接、信息强调和键盘焦点。状态颜色保留成功、警告和失败的语义。

浅色原型的 `r-green` 为 `#187448`。这个颜色用于状态文字时，与状态自身 12% 透明底色叠在
`r-raised` `#F4F4F4` 上时未达到 4.5:1。当前浅色状态绿调整为 `#176F46`，保留绿色语义并通过
实际叠色对比检查。原型的 `r-soft` 浅色值 `#737373` 在 `#F4F4F4` 上对比为 4.31:1；应用使用
`r-muted` `#656565` 作为弱化文字，并在共享主题测试覆盖的表面上验证小字对比度。`theme.rs` 当前圆角为 6px 和
8px，原型 `r-radius` 为 7px；各控件圆角仍需逐项核对。代码只实现明确映射的运行时令牌；
原型参考值不表示代码提供同名令牌。本文件说明目标与已记录的差异，不生成代码。

字体和字号偏好由 `crates/ramag-ui/src/system_settings.rs` 设置。正文默认 Inter，数据默认
JetBrains Mono；用户可以选择 IBM Plex 字体。Michroma 目前用于部分监控标题，不应推广为所有
页面的默认标题。

## 布局与组件

- 宽窗口使用带文字的工具导航和工作区顶栏。原型侧栏宽 196px，顶栏高 59px；中宽 API 和数据库工作区使用 62px 图标栏。760px 以下原型使用抽屉和底部快捷入口。正式窗口目前最小宽度为 1008px，较窄布局只能作为无头测试范围。
- 对象导航、工具栏、标签和稠密行的现有尺寸集中在 `crates/ramag-ui/src/workbench.rs`。工具应复用这些尺寸或明确说明例外，避免为每页复制近似值。
- 背景和面板主要通过颜色及细边框区分；阴影保持轻微。使用现有主题半径，不给同类控件各自定义圆角。
- 文本、图标和按钮应提供可辨认的选中、悬停、焦点、禁用和忙碌状态。危险操作使用语义色并保留文字说明，不只依靠颜色。
- 使用共享按钮、表单、对话框、通知和滚动组件。长标题可以截断，但应能查看完整内容；表格保留对长数据和双轴滚动的访问。
- 动效用于说明加载、切换和排序，不应遮挡操作或使数据状态难以判断。遵循已有动效和用户显示设置，不新增装饰性持续动画。

## 验收

按[开发计划第 4.48 节](docs/01-development-plan.md)逐工具核对原型、明暗主题、主要操作和异常状态。
共享主题已按运行时映射调整背景、表面、边框、文字和中性主按钮；原型中的强边框与弱化文字参考值尚无独立令牌。
仍需完成各工具逐页对照，
并核对例外颜色、状态、对比度和原生可访问性。真实窗口、headless 测试与缺少平台的结果分别记录。
未迁移页面和未验证的平台不得标为已验收。
