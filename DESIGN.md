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

- 宽窗口使用带文字的工具导航和工作区顶栏。原型侧栏宽 196px，顶栏高 59px。中宽 API 和数据库工作区使用 62px 图标栏。760px 及以下使用导航抽屉和底部快捷入口。数据库窄屏通过“对象”入口打开对象树，保持查询区宽度。正式窗口最小宽度为 1008px。更窄布局使用无头测试和显式的隔离预览窗口检查。
- 对象导航、工具栏、标签和稠密行的现有尺寸集中在 `crates/ramag-ui/src/workbench.rs`。工具应复用这些尺寸或明确说明例外，避免为每页复制近似值。
- 背景和面板主要通过颜色及细边框区分；阴影保持轻微。使用现有主题半径，不给同类控件各自定义圆角。
- 文本、图标和按钮应提供可辨认的选中、悬停、焦点、禁用和忙碌状态。危险操作使用语义色并保留文字说明，不只依靠颜色。
- 使用共享按钮、表单、对话框、通知和滚动组件。长标题可以截断，但应能查看完整内容；表格保留对长数据和双轴滚动的访问。
- 动效用于说明加载、切换和排序，不应遮挡操作或使数据状态难以判断。遵循已有动效和用户显示设置，不新增装饰性持续动画。

## 各工具页面设计

页面从上到下显示当前对象、主要操作和工作内容。页头使用一行主标题和必要的上下文，例如集群名称与连接状态、当前仓库与分支。主要操作紧邻其作用对象，每个操作区突出一个动作。常用次要操作使用描边或无底色按钮，图标按钮提供可读名称。共享工具栏、页签和稠密行分别以 32px、30px 和 26px 为基准。页面背景、导航与内容面板通过细边框和一层表面色分开。

连接、对象和目录使用左侧导航，列表、编辑器或终端占用主要空间。窄窗口按任务缩减同时显示的面板。选择连接或对象后返回工作区，关闭选择界面后把焦点还给入口。长名称允许省略，并通过提示或详情显示完整值。长数据在所属表格、编辑器或终端内滚动，标题和操作入口保持可达。

| 工具 | 页面布局和主要操作 | 窄窗口设计 |
|---|---|---|
| 总览 | 搜索和筛选在上，工具网格或列表在下。卡片统一图标、名称、说明和打开入口 | 工具列表按宽度改为单列或双列，筛选与布局切换可换行 |
| API | 左侧查看请求集合和历史，右侧顶部放请求目标和发送操作，下方并列请求编辑与响应 | 通过“请求集合”打开弹窗。请求与响应通过页签切换。gRPC 地址独占一行，服务和方法并排 |
| 数据库 | 页头显示连接与状态，左侧浏览对象，主区显示查询编辑器与结果。关系数据库、Redis 和 MongoDB 保留各自的数据编辑控件 | 通过“对象”打开对象树。查询、表格或文档内容优先占满宽度，长值可在内容区滚动 |
| Kafka | 集群导航固定在左侧。主区顶部显示连接、只读状态和集群操作。主题、消息、消费者组和管理页面沿用同一页签与操作栏 | 从独立入口选择集群，次级页签可滚动，消息筛选可换行，消息详情独立滚动 |
| MQTT | 左侧管理 Broker 配置，主区统一概览、发布和订阅的页头。主题筛选、发布表单与消息流放在对应任务区，本地 Broker 管理保留独立入口 | 连接选择可单独打开。主题、载荷与发布按钮按编辑顺序排列，消息正文与消息列表分区 |
| SSH／SFTP／终端 | 左侧管理主机。会话标签、连接状态与常用命令集中在页头。终端使用剩余空间，SFTP 路径栏跟随文件列表 | 主机选择单独打开。会话栏可滚动，SFTP 在本地与远端之间切换，避免挤压表格和终端 |
| 端口转发 | 以规则列表为主，列出方向、本地与远端地址、状态和启动或停止操作。编辑规则放在独立表单 | 保留规则名称、连接状态和启动或停止入口，长地址在详情中完整显示 |
| 对象存储 | 连接与存储桶在左侧。路径、上传与下载操作在文件列表上方，传输进度放在独立区域 | 通过入口选择连接和存储桶。路径允许省略并可查看完整值，传输状态可从工具栏打开 |
| 容器 | 页头显示运行环境和连接状态，容器、镜像、网络、数据卷和镜像仓库使用一致的列表操作区。日志保留可滚动正文 | 次级页签可滚动，表格保留名称与状态，详情和日志独占内容区 |
| Git | 页头显示仓库、分支与操作状态。变更列表和差异视图并列，保留暂存和提交操作的上下文 | 变更列表与差异视图按页切换，保持当前文件选择，长差异独立滚动 |
| 剪贴板 | 搜索与筛选在顶部，条目和预览分栏。条目显示类型与时间，复制和固定放在条目或预览的操作区 | 选中条目后打开预览，关闭时返回原列表位置。长文本和图片保留完整预览入口 |
| 系统监控 | 页头集中刷新与采样状态。概览突出当前资源读数，进程页面单独提供搜索、排序和进程详情 | 指标卡减少列数。进程表优先显示名称和当前排序指标，详情可查看其余字段 |
| 设置、更新与插件 | 设置按功能分组。更新卡集中显示当前版本、检查结果和更新进度。插件列表统一显示名称、状态与对应操作 | 设置分组可单列访问。说明文字和操作可换行，更新进度保留在当前任务区 |

初始空状态说明如何开始，筛选无结果时提供清除筛选入口。加载反馈留在受影响的区域。连接、保存或执行失败时，在对应页面显示错误、恢复入口与当前数据。破坏性操作沿用已有确认流程。明暗主题使用同一布局和操作层级。

## 验收

按[开发计划第 4.48 节](docs/01-development-plan.md)逐工具核对原型、明暗主题、主要操作和异常状态。
共享主题已按运行时映射调整背景、表面、边框、文字和中性主按钮；原型中的强边框与弱化文字参考值尚无独立令牌。
首页、API、数据库、Kafka、MQTT、SSH、对象存储、容器、Git、剪贴板和系统监控已完成逐页切片记录。
设置、更新、插件、MongoDB、Redis 和端口转发已完成代码审查与定向测试。真实窗口、headless 测试与缺少平台的结果分别记录。
当前原型验收使用 macOS 原生窗口、GPUI headless 测试和截图。Windows 原生 UI 不属于本次验收范围。
插件目录签名、下载、安装、沙箱和进程树属于独立功能验收。
