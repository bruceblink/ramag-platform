# Wu.CommTool MQTT 重写详细设计

> 状态：设计已完成；Phase 1 进行中，本轮已完成订阅 Topic 列表、No Local、列表持久化、消息时间线控制和消息查看器首版。
>
> 适用范围：Ramag Platform 的 MQTT 工作台、内置本地 MQTT Broker、远端 MQTT Client 连接、消息发布/订阅和与 Wu.CommTool 的配置及交互兼容。
>
> 参考基线：Wu.CommTool 仓库当前主分支提交 d75ec7f（2026-09-17 读取）；Ramag 当前分支 feat/mqtt-wu-commtool-core。

## 术语与命名规则

| 规范中文名 | English / Acronym | 本方案中的职责边界 | 不表示什么 |
|---|---|---|---|
| MQTT 客户端 | MQTT Client | 主动连接远端或本地 Broker，执行连接、发布和订阅的数据面会话 | 不表示本地 Broker，也不表示 Mosquitto 管理接口 |
| 本地 MQTT Broker | Local MQTT Broker | 由 Ramag 在本机启动和停止的嵌入式 Broker，供测试客户端连接 | 不表示远端 Mosquitto，也不表示 MQTT 客户端连接本身 |
| 远端 Broker | Remote Broker | MQTT 客户端实际连接的目标地址，可以是本地服务、局域网服务或远端服务 | 不表示 Ramag 的配置存储 |
| Broker 配置 | Broker Profile | 保存连接地址、协议、认证、TLS 和管理参数的本地配置记录 | 不表示一次正在运行的连接 |
| 订阅记录 | MQTT Subscription | 一个 Topic Filter 及其 QoS 等订阅选项 | 不表示已经收到的消息 |
| 消息时间线 | Message Timeline | 工作台内有界的发送、接收和状态消息列表 | 不表示 Broker 的持久化消息队列 |
| 载荷格式 | Payload Format | 文本编辑器输入和消息显示时使用的 UTF-8、JSON、Hex、Base64 等转换规则 | 不表示 MQTT 协议的线速编码 |
| MQTT 数据面 | MQTT Data Plane | 连接、发布、订阅和接收消息的协议路径 | 不表示 Mosquitto Dynamic Security 管理命令 |
| Mosquitto 管理面 | Mosquitto Management Plane | Dynamic Security 和 password_file/acl_file 的明确管理路径 | 不表示通用 MQTT Broker 都具备的能力 |
| 传输驱动 | MQTT Driver | ramag-domain 定义、ramag-infra-mqtt 实现的 MQTT 协议适配接口 | 不表示 GPUI 视图或 MQTTnet 类型 |
| 本地服务驱动 | Local Server Driver | 本地 MQTT Broker 的启动、停止和状态接口 | 不表示远端 Broker 的发布订阅驱动 |

正文首次出现使用表中的规范中文名和英文名；后续只使用规范中文名。图、表和代码边界使用同一套名称。

## 1. 设计背景与目标

### 1.1 参考项目能力

Wu.CommTool 是一个 Windows WPF 工具，MQTT 部分分为 MQTT Server 和 MQTT Client 两个入口：

1. MQTT Server 在本机启动 MQTT Broker，显示连接、订阅、收发消息和客户端状态，并可以由 Broker 注入发布消息。
2. MQTT Client 配置 Server IP、端口、Client ID、用户名、密码、MQTT 版本、Keep Alive、自动重连和 TLS，执行订阅、取消订阅、发布及消息格式转换。
3. 两个入口都有暂停、清空、接收载荷格式选择和配置导入/导出；消息右键操作可以查看格式化 JSON 或进行 UTF-8、Hex、Base64 转换。
4. MQTT Client 支持 Topic、No Local、QoS 订阅记录，发布支持 QoS、Retain、载荷格式和回车发送；参考项目另有 SM4 加密发布能力。

参考文件与截图：

- README.md 的 MQTT 功能说明。
- Modules/Wu.CommTool.Modules.MqttClient/Models/MqttClientConfig.cs。
- Modules/Wu.CommTool.Modules.MqttClient/Models/MqttTopic.cs。
- Modules/Wu.CommTool.Modules.MqttClient/Views/MqttClientView.xaml。
- Modules/Wu.CommTool.Modules.MqttServer/Models/MqttServerConfig.cs。
- Modules/Wu.CommTool.Modules.MqttServer/Views/MqttServerView.xaml。
- Wu.CommTool/Images/About/Mqtt服务器.png。
- Wu.CommTool/Images/About/Mqtt客户端.png。
- Wu.CommTool/Images/About/Mqtt服务器查看格式化Json.png。

### 1.2 Ramag 重写目标

本次工作是对参考项目 MQTT 工作流的 Rust/GPUI 重写，不是另起一个只保留协议 API 的 MQTT 管理页。目标按以下优先级执行：

1. 保留参考项目中用户可直接感知的 MQTT Server/Client 工作流：配置、启动或连接、订阅、发布、暂停、清空、载荷转换和消息查看。
2. 复用 Ramag 已有的 Domain/App/Infrastructure/Tool 分层，不让 GPUI 视图依赖 rumqttc、oximqtt 或 Mosquitto 管理命令。
3. 在不改变消息语义的前提下使用 Ramag 的本地配置存储、取消、错误分类、凭据脱敏和有界缓冲能力。
4. 把参考项目没有明确保证的行为写成验收条件或未决项，不用静态界面或演示数据代替真实 Broker 结果。
5. 完成一个独立功能后，分别执行领域/基础设施测试、headless UI 测试和需要时的真实 Windows 窗口验收，再提交和推送。

### 1.3 非目标

- 本设计不把 Modbus、UDP、JSON 独立转换器或其他 Wu.CommTool 模块纳入 MQTT 重写范围。
- 不提供云 Broker 账号管理、集群编排、任意远程 Shell 或默认开放的 Mosquitto 配置写入。
- 不绕过 TLS 证书校验；自签名证书必须通过明确的 CA 或用户选择的验证策略接入。
- 不把 MQTT Topic 目录、在线客户端列表描述为 MQTT 标准保证的完整结果；驱动必须标记结果是否完整。
- 不把 headless 渲染测试写成真实窗口验收，也不把远端服务结果写成本机 Docker 集成结果。
- SM4 加密发布先保留为兼容性未决项，不在第一阶段偷偷加入新的密码学实现。

## 2. 当前实现基线

当前代码已经具备一条可工作的 Ramag MQTT 路径，后续设计以现有边界为起点，不重新创建重复的连接模型。

| 代码边界 | 当前职责 | 当前状态 |
|---|---|---|
| ramag-domain/entities/mqtt/connection_config.rs | Broker 配置、协议版本、TCP/TLS、凭据、Keep Alive 和 Mosquitto 管理参数 | 已实现验证和脱敏 Debug 输出 |
| ramag-domain/entities/mqtt_protocol.rs | QoS、订阅请求、发布请求、接收消息、User Property 和 Broker 观察结果 | 已实现有界模型和字段校验 |
| ramag-domain/entities/mqtt/local_server.rs | 本地 Broker 监听地址、端口、匿名策略、固定账号和状态 | 已实现认证配置前置校验 |
| ramag-domain/traits/mqtt_driver.rs | 测试连接、发布、持续订阅和 Broker 观察接口 | 已实现接口 |
| ramag-domain/traits/mqtt_local_server.rs | 本地 Broker 启动、停止和状态接口 | 已实现接口 |
| ramag-app/usecases/mqtt_service.rs | 校验、调用驱动、存储配置、错误和日志编排 | 已实现 |
| ramag-infra-mqtt/src/lib.rs | Native MQTT 驱动和 Mosquitto Dynamic Security 管理 | 已实现 MQTT 3.1.1、MQTT 5、TCP/TLS、发布和订阅 |
| ramag-infra-mqtt/src/local_server.rs | Native 本地 MQTT Broker 生命周期和固定账号认证 | 已实现，使用 oximqtt |
| ramag-tool-mqtt/src/lib.rs | GPUI 工作区状态、配置、状态、发布、订阅、本地服务和 Mosquitto 页面 | 已实现主要页面 |
| ramag-tool-mqtt/src/mqtt_view/payload_format.rs | 发布编码和接收显示格式 | 已实现 UTF-8、JSON、Hex、Base64 三类转换及参考项目的组合模式 |
| ramag-tool-mqtt/src/mqtt_view/message_operations_view.rs | 发布和订阅操作区、QoS、Retain、消息元数据 | 已实现，已有窄窗口 headless 覆盖 |
| ramag-tool-mqtt/src/mqtt_view/local_server_view.rs | 本地 Broker 地址、端口、匿名策略、账号、启动停止和填入客户端配置 | 已实现生命周期交互 |
| ramag-tool-mqtt/src/mqtt_view/subscription_operations.rs | 订阅 Topic 列表新增、删除、逐条 QoS/No Local 编辑和协议能力约束 | 本次 Phase 1 切片已实现，列表随 Broker 配置保存 |
| ramag-tool-mqtt/src/mqtt_view/message_timeline_operations.rs | 消息时间线暂停展示、恢复展示、清空本地消息和有界追加 | 本次 Phase 1 切片已实现，暂停不停止订阅连接 |
| ramag-tool-mqtt/src/mqtt_view/message_viewer.rs | 消息右键菜单、JSON/文本格式查看、复制 Topic 和当前格式 | 本次 Phase 1 切片已实现，查看正文和复制内容均有大小上限 |

已完成的近期 MQTT 切片包括原生订阅取消、MQTT 3.1.1/5 Docker 集成、QoS/Retain 选项、未保存配置生效、停止状态保护、消息元数据显示、本地 Broker 退出等待和匿名认证前置校验。详细验证记录保留在 docs/development-roadmap.md；这些记录不会替代本设计中新增功能的验收。

本轮新增消息时间线控制：订阅运行时点击“暂停展示”只停止向当前窗口追加消息，MQTT 连接和接收任务继续运行；点击“恢复展示”后新消息继续追加；点击“清空时间线”只清除本地列表，不向 Broker 发送删除命令，也不改变 retained 消息。`ramag-tool-mqtt` 的 19 项库测试通过，其中包含 360px 和 1440px headless 布局及暂停、恢复、清空交互测试。真实 Windows 窗口和远端 Broker 验证仍未完成。

本轮还将订阅 Topic 列表写入 `MqttProfile` 的加密配置记录。保存配置时记录每条 Filter、QoS 和 No Local；切换配置或重新加载后恢复同一列表；缺少 `subscriptions` 字段的旧 JSON 继续使用 `+/#`、QoS 1、关闭 No Local 的默认记录。领域、加密存储和 headless 保存/恢复测试已覆盖该路径，逐条连接状态和取消订阅操作仍未完成。

本轮新增消息查看器：消息卡片提供上下文菜单，可查看有界的 UTF-8、JSON、Hex 和 Base64 文本，切换显示格式，复制 Topic 或当前格式结果。JSON 当前使用格式化文本显示，后续再补树形节点交互；查看器支持 360/1024/1440px headless 布局，未把 headless 结果写成真实 Windows 窗口验收。

当前与参考项目仍存在的主要差距：

- 本地 Broker 还没有完整的 Broker 侧消息事件、在线客户端和每客户端订阅管理工作流。
- 订阅记录已经扩展为可新增、删除和编辑 QoS/No Local 的 Topic 列表，并随 `MqttProfile` 加密保存；逐条订阅/取消订阅仍待补齐。
- 消息时间线已支持暂停展示、恢复展示、清空本地列表和有界消息查看器；当前查看器使用格式化 JSON 文本，还没有参考项目式的 JSON 树节点交互。
- 参考项目的 jsonMCC/jsonMSC 配置导入导出和快速配置列表尚未完成兼容层。
- 本地 Broker TLS 证书端点、Broker 注入发布和客户端管理能力需要扩展本地服务接口。
- 参考项目的 AutoReconnect 选项尚未在 Ramag 配置和界面中形成明确的开关及重连策略。
- 当前 Mosquitto 管理面是 Ramag 的额外能力，不能代替本地 Broker 的 MQTT Server 工作流。

## 3. 兼容性范围与功能矩阵

| 参考行为 | 目标行为 | 当前实现 | 后续阶段 | 验收证据 |
|---|---|---|---|---|
| MQTT Server 启动/停止本地服务 | 本地 Broker 使用监听地址、端口、匿名策略和固定账号启动；运行中配置不能静默改变 | 启动、停止、状态和配置一致性已实现 | 补 Broker 事件和 TLS | oximqtt 本机测试、端口回读、UI 操作 |
| MQTT Server 接收消息 | 本地 Broker 将连接、订阅、发布和接收事件送入有界消息时间线 | 生命周期已实现，事件流未完成 | Phase 2 | 本机 Docker 客户端连接本地 Broker，窗口收到真实消息 |
| MQTT Server Broker 发布 | 从本地 Broker 注入 Topic、载荷、QoS、Retain 消息 | 尚无本地服务发布接口 | Phase 2 | 客户端订阅收到真实注入消息 |
| MQTT Server 客户端管理 | 显示真实在线 Client ID、用户名、连接时间和订阅 Topic | 尚无本地 Broker 观察接口 | Phase 2 | 两个真实客户端连接后的窗口结果 |
| MQTT Client 连接 | 支持 MQTT 3.1.1/5、TCP/TLS、Client ID、认证、Keep Alive、自动重连、取消和错误分类 | Native 驱动已实现连接和取消；自动重连开关未对齐 | Phase 1 | 本机 Docker Mosquitto 3.1.1/5 测试 |
| MQTT Client Topic 列表 | 多条 Topic Filter 可添加、删除、编辑，逐条显示 QoS 和 No Local | 已实现多条列表、添加/删除、逐条 QoS/No Local 编辑和随配置保存 | Phase 1 后续补逐条订阅动作 | Domain 校验、加密存储往返、headless 操作、真实订阅回读 |
| MQTT Client 订阅/取消订阅 | 启动和停止状态可见，停止等待驱动真正退出；暂停只停止当前窗口追加 | 持续订阅、取消、暂停展示、恢复展示和清空本地列表已实现 | Phase 1 补列表语义 | 取消延迟测试、headless 操作回读、真实窗口状态回读 |
| MQTT Client 发布 | Topic、载荷、载荷格式、QoS、Retain、回车发送 | 发布和 QoS/Retain 已实现 | Phase 1 补交互 | 发布回执、消息时间线和 UI |
| 载荷转换 | UTF-8、JSON、Hex、Base64、组合模式；转换失败不发送 | 发布编码和接收格式化已实现 | Phase 1 补查看器 | 单元测试和消息查看器 |
| 消息右键查看 | JSON 以树形或格式化文本查看，原始字节可切换 UTF-8/Hex/Base64 | 已实现消息上下文菜单、格式化 JSON/文本查看、格式切换和复制；JSON 树节点交互未实现 | Phase 1 后续补树形查看 | JSON 非法输入回退、复制内容、有界查看器和真实窗口操作 |
| 配置导入/导出 | 支持 Ramag 配置存储，并提供 jsonMCC/jsonMSC 兼容导入 | Ramag Storage 配置 CRUD 已实现 | Phase 3 | 旧文件导入、导出再导入字段一致性 |
| SM4 加密发布 | 明确算法、密钥存储和线速行为后再加入 | 未实现 | 未决 | 设计批准后单独切片 |
| Mosquitto 管理面 | 与 MQTT 数据面分栏，使用明确管理接口和真实结果 | Dynamic Security/静态文件已实现部分 | 持续维护 | 本机 Mosquitto 管理测试 |

兼容优先级为：连接和消息数据面 > 本地服务生命周期 > 消息查看和格式转换 > 配置文件兼容 > SM4 扩展。任何阶段都不能以增加按钮数量代替对应的真实行为。

## 4. 总体架构

### 4.1 依赖边界

MQTT 工作台沿用 Ramag 的依赖方向。工具界面只构造结构化请求和展示真实返回；应用服务负责校验、取消、状态编排和日志；基础设施负责 MQTT 协议或本地 Broker 的实际运行。

~~~mermaid
flowchart LR
    User[用户] -->|填写配置、点击操作| ToolUI[MQTT 工作台界面]
    ToolUI -->|结构化请求| MqttApp[MQTT 应用服务]
    MqttApp -->|配置校验与领域模型| Domain[MQTT 领域模型与接口]
    MqttApp -->|连接、发布、订阅| MqttDriver[传输驱动]
    MqttDriver -->|MQTT 数据面| RemoteBroker[远端 Broker]
    MqttApp -->|启动、停止、状态| LocalDriver[本地服务驱动]
    LocalDriver -->|Broker 监听与事件| LocalBroker[本地 MQTT Broker]
    MqttApp -->|配置读写| Storage[本地配置与秘密存储]
    MqttDriver -->|消息回调与结果| MqttApp
    LocalDriver -->|状态与 Broker 事件| MqttApp
    MqttApp -->|有界消息、状态和错误| ToolUI
~~~

图中 MQTT 传输驱动和本地服务驱动是并列接口。远端 Broker 的 MQTT 数据面不能调用本地服务驱动；Mosquitto 管理面继续通过单独的管理驱动访问 Dynamic Security 或静态文件。

### 4.2 建议 crate 职责

| 位置 | 负责内容 | 不负责内容 |
|---|---|---|
| ramag-domain | 配置、请求、消息、状态、错误、上限和跨层 trait | GPUI、rumqttc、oximqtt、文件选择器 |
| ramag-app | 配置校验、用例编排、取消、消息背压、状态日志和存储调用 | MQTT 线协议、界面布局 |
| ramag-infra-mqtt | Native 连接、TLS、发布、订阅、本地 Broker 和真实错误映射 | 用户输入、按钮状态、配置导入界面 |
| ramag-tool-mqtt | 配置表单、工具栏、消息时间线、主题管理、查看器和 headless UI | 存储明文凭据、绕过应用层直接调用基础设施适配器 |
| ramag-infra-storage | 配置和敏感字段的本地保存、读取和删除 | MQTT 连接生命周期 |
| ramag-bin | 驱动组合、应用退出时等待本地 Broker 停止 | MQTT 业务规则 |

### 4.3 运行时并发规则

1. 每次发布、测试连接和管理请求使用有界操作超时；持续订阅使用取消标志和独立停止路径，不被短操作超时提前终止。
2. 消息从驱动进入应用层的通道必须有容量上限。满载时返回背压结果并记录丢弃原因，不无限增长内存。
3. 视图切换配置或删除配置时，先取消旧订阅；迟到结果只能更新对应的操作代次，不能覆盖新选中的配置。
4. 本地 Broker 在启动、停止、应用退出和线程异常时都清理停止句柄；端口未确认释放前，界面保持“正在停止”。
5. 发布和订阅是否共用一条 MQTT Client 会话由第 12 节未决项决定。决定前沿用当前隔离驱动和 UI 禁用规则，不创建隐式第二连接。

## 5. 领域数据模型

### 5.1 远端 Broker 配置

Broker 配置继续使用 ramag-domain 的 MqttProfile，字段语义如下：

| 字段组 | 字段 | 规则 |
|---|---|---|
| 标识 | id、name、remark | id 稳定；name 非空；Debug 和日志不输出秘密 |
| 网络 | host、port、transport | host 不带协议前缀；TCP 默认 1883，TLS 默认 8883；端口必须在 1-65535 |
| 协议 | protocol_version、keep_alive_seconds、clean_start、session_expiry_seconds、auto_reconnect | MQTT 3.1.1 不接受 MQTT 5 专属会话过期字段；Keep Alive 使用 0 或不小于最小值；自动重连由应用层状态机控制 |
| 身份 | client_id、username、password | password 有值时 username 必须有值；空 Client ID 使用驱动生成的临时 ID |
| TLS | verify、ca_cert_path、client_cert_path、client_key_path | 客户端证书和密钥必须成对；非 TLS 不能配置证书路径；禁止无提示跳过验证 |
| 订阅 | subscriptions | 保存 Topic Filter、QoS 和 No Local；旧 JSON 缺失时使用默认订阅；列表内容不表示 Broker 当前已订阅状态 |
| 管理 | management | 仅供 Mosquitto 管理面使用，不改变通用 MQTT 数据面身份 |

配置表单的当前值和已保存配置分开处理。发布、订阅和测试连接默认使用当前表单快照；保存操作只负责持久化，不是让未保存输入生效的前置条件。

### 5.2 消息和请求

| 模型 | 必填字段 | 重要约束 |
|---|---|---|
| MqttSubscription | filter、qos、no_local | Filter 按 MQTT 通配符规则校验；最多 128 条；重复 Filter 在 UI 中拒绝；No Local 仅在 MQTT 5 线速请求中生效 |
| MqttSubscribeRequest | subscriptions | 不能为空；驱动返回前不写入“已订阅”状态 |
| MqttPublishRequest | topic、payload、qos、retain、user_properties | 发布 Topic 不能有 + 或 #；载荷、有属性数量和属性长度有上限 |
| MqttMessage | topic、payload、qos、retain、duplicate、received_at、user_properties | 真实接收时间由驱动或应用生成；Debug 只记录载荷长度，不记录载荷正文 |
| MqttPublishResult | topic、packet_id、qos | 只有驱动成功返回后才将消息标为已发送 |

消息时间线在 UI 层保留最多 MAX_MESSAGES 条。清空只清理当前视图，不向 Broker 发送删除消息或改变 retained 状态。

### 5.3 本地 MQTT Broker

本地服务配置使用 MqttLocalServerConfig：

| 字段 | 规则 |
|---|---|
| bind_host | 必须是 IPv4/IPv6 地址；0.0.0.0 或 :: 需要显示暴露到所有网卡的提示 |
| port | 1-65535；启动前验证端口 |
| allow_anonymous | true 时允许无凭据连接；false 时至少有一个有效固定账号 |
| users | 用户名唯一，用户名和密码不能为空，密码不进入日志 |

运行状态使用 MqttLocalServerStatus。状态至少区分已停止、启动中、运行中、停止中和错误；配置变更不会修改已运行实例，必须先停止再启动。

### 5.4 载荷格式

| 名称 | 发布编码 | 接收显示 |
|---|---|---|
| UTF-8 | 文本按 UTF-8 编码 | 无损 UTF-8，非法字节使用明确替代显示 |
| JSON | 先按 UTF-8 发送 | 可解析时格式化，不可解析时保留原始文本并提示 |
| Hex | 去除空白后解码十六进制 | 以分组十六进制显示 |
| Base64 | 去除空白后解码 | 对原始字节编码为 Base64 |
| Base64+UTF-8 | 文本 UTF-8 后编码为 Base64 字节 | 按参考项目行为解码显示 |
| Base64+Base64 | 保留参考项目的线速行为 | 必须用回归样例锁定，不能仅按名称猜测 |

格式转换在 UI 预览或发送前执行。解析失败时保留输入、显示错误并且不发送。

## 6. 交互与界面设计

### 6.1 参考与取舍

参考项目的可借鉴交互是：顶部固定操作栏、配置抽屉、消息时间线、底部发布编辑区、主题管理抽屉、暂停/清空和消息右键查看。Ramag 不复制 WPF 抽屉、圆形按钮和颜色，而是在现有 GPUI 工作台中保留同样的操作顺序和反馈：

1. 左侧配置列表保持 Broker 配置可发现性。
2. 主区使用“配置、状态、发布、订阅、本地服务、Mosquitto”页签，避免窄窗口同时挤出多个抽屉。
3. 发布和订阅操作区保持内容宽度，长 Topic、载荷和元数据可换行或滚动。
4. 需要详情时使用上下文菜单或局部查看器，不把大 JSON 直接塞进消息卡片。

### 6.2 页面结构

| 区域 | 内容 | 关键操作 |
|---|---|---|
| 配置栏 | 搜索、配置列表、新建、当前配置名称 | 选择、创建、删除 |
| 页头 | 当前配置、连接状态、测试连接、保存、窄屏配置栏开关 | 取消旧操作后切换配置 |
| 配置页 | 名称、地址、端口、协议、Client ID、认证、Keep Alive、TLS、Mosquitto 管理开关 | 输入校验、保存、测试 |
| 状态页 | 驱动能力、Broker 观察结果、Topic/在线客户端计数和完整性标记 | 刷新观察结果 |
| 发布页 | Topic、载荷、格式、QoS、Retain、发布按钮 | 发布成功后写入时间线 |
| 订阅页 | Topic Filter 列表、每条 QoS/No Local、接收格式、开始/停止、消息时间线 | 添加、删除、订阅、取消、暂停、清空 |
| 本地服务页 | 监听地址、端口、匿名策略、固定账号、状态、启动/停止、填入客户端配置 | 启动、停止、添加/删除账号 |
| Mosquitto 页 | Dynamic Security 用户/组/角色、静态 password_file/acl_file | 读取、保存、删除；只在启用管理面时可用 |

### 6.3 MQTT 客户端交互规则

1. 点击“测试连接”只执行连接验证，不改变订阅状态和消息时间线。
2. 点击“开始订阅”先读取当前表单快照，校验所有 Topic Filter；驱动确认订阅前按钮显示加载状态，不能显示“已订阅”。
3. 点击“停止订阅”后保持“正在停止订阅”，直到取消连接和驱动任务都返回；期间禁用重新开始、切换配置和删除配置。
4. 点击“发布消息”先转换载荷和校验 Topic；失败时保留输入。驱动返回成功后才将“发送”记录写入消息时间线。
5. 暂停只停止当前窗口追加消息，不停止 Broker 连接；恢复后继续接收新消息。清空只清理窗口列表。
6. 订阅输入支持回车或明确按钮提交；发布输入按照参考项目保留回车发送，但多行编辑器使用 Ctrl+Enter 作为无歧义发送快捷键，最终以 UI 验收确定。
7. “填入客户端配置”复制本地服务地址和端口；有固定账号时复制第一条账号，无固定账号时清空客户端用户名和密码，避免把旧凭据带到匿名服务。

### 6.4 MQTT Server 交互规则

1. 启动前锁定监听地址、端口、匿名策略和固定账号表单；运行中只允许查看状态和消息。
2. 运行状态显示实际监听端点、匿名策略和客户端数量；启动失败显示绑定、认证或 TLS 的具体原因。
3. Broker 事件按来源显示“系统、客户端连接、客户端订阅、客户端发布、客户端断开、Broker 发布”，并使用不同的可读标记。
4. 客户端管理列表只显示本地 Broker 的真实连接；没有事件或驱动不支持时显示“未提供”，不生成示例客户端。
5. Broker 发布编辑器沿用客户端发布字段；注入成功后显示发送记录，不能把本地注入误标记为某个客户端发布。

### 6.5 消息上下文菜单和查看器

每条消息保存原始字节和元数据。上下文菜单至少提供：

- 查看格式化 JSON：JSON 可解析时显示可展开树或格式化文本；不可解析时显示原文和解析失败原因。
- 查看 UTF-8、Hex、Base64：只改变查看方式，不修改时间线中的原始字节。
- 复制 Topic、复制载荷和复制当前格式化结果。

查看器关闭后回到原消息位置。大载荷使用上限和滚动容器；超过上限只显示截断信息并保留原始长度。

### 6.6 响应式规则

1. 760px 以下主内容区使用更小内边距和可换行的字段行；480px 以下默认隐藏配置栏，提供显式显示按钮。
2. 所有固定格式控件使用稳定宽度或最小宽度；按钮不得因加载文字改变布局。
3. Topic、Client ID、错误文本和消息元数据都允许截断或换行，不覆盖相邻控件。
4. UI 测试覆盖 360、640、1024、1440px；headless 结果和真实 Windows 截图分开记录。

## 7. 状态与数据流

### 7.1 MQTT 客户端操作状态

~~~mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Connecting: 测试、发布或开始订阅
    Connecting --> Connected: Broker 返回 CONNACK
    Connecting --> Failed: 校验、网络或认证失败
    Connected --> Subscribing: 发送订阅请求
    Subscribing --> Subscribed: 订阅确认
    Subscribed --> Stopping: 用户停止或配置切换
    Stopping --> Idle: DISCONNECT 和任务退出
    Connected --> Publishing: 发布请求
    Publishing --> Idle: 发布确认或失败
    Subscribed --> Failed: 连接异常
    Failed --> Idle: 用户关闭错误提示
~~~

状态转换规则：

- “Connected”是操作内部状态；只有 UI 需要持续接收时才保持到“Subscribed”。
- “Failed”必须保留可读错误类别，不能统一成“连接失败”。
- 取消和错误都必须释放任务、通道和临时 Client ID。
- 旧操作的迟到结果通过 operation generation 丢弃。

### 7.2 本地 MQTT Broker 状态

~~~mermaid
stateDiagram-v2
    [*] --> Stopped
    Stopped --> Starting: 启动服务
    Starting --> Running: 监听成功
    Starting --> Error: 绑定、认证或 TLS 失败
    Running --> Stopping: 用户停止或应用退出
    Running --> Error: Broker 线程异常退出
    Stopping --> Stopped: 端口释放且线程结束
    Stopping --> Error: 等待线程超时或句柄失效
    Error --> Stopped: 清理句柄并重新读取状态
~~~

“运行中”只在监听成功后显示；“停止中”不得允许再次启动或修改配置。应用退出必须等待本地服务驱动完成停止，避免端口或后台线程残留。

## 8. 配置持久化与兼容导入

### 8.1 Ramag 配置

Broker 配置通过 Storage trait 保存，订阅 Topic 列表随 `MqttProfile` 一并加密保存；密码、TLS 私钥路径和 Mosquitto 管理密码按现有秘密存储策略保存。普通日志只记录配置 ID、目标地址、操作类型和结果，不记录密码、完整证书内容或载荷正文。

本地服务配置可以先作为工作台偏好保存；是否升级为独立的 Local Server Profile 由 Phase 2 的客户端管理和 TLS 需求决定。运行状态字段禁止写入持久化配置。

### 8.2 Wu.CommTool 兼容导入

兼容层识别以下扩展名：

- jsonMCC：映射为一个 Broker 配置和订阅列表。
- jsonMSC：映射为一个本地服务配置、授权用户和发布默认值。

导入流程：

1. 读取文件大小和 JSON 结构上限。
2. 解析旧字段名，包括参考项目中的 Paylod 拼写。
3. 将 MQTT 3.1、3.1.1、5.0、Qos0/1/2、载荷格式和 TLS 字段映射到 Ramag 枚举。
4. 丢弃参考项目的运行态字段 IsOpened、SubscribeSucceeds 和客户端列表。
5. 对密码和证书路径执行本机存储策略，导入失败不覆盖当前配置。
6. 显示字段映射、丢弃字段和需要用户确认的差异。

导出默认生成 Ramag 版本化 JSON。兼容导出若需要保留 jsonMCC/jsonMSC 扩展名，必须单独选择，不把 Ramag 的管理面字段伪装成 Wu.CommTool 字段。

## 9. 错误、安全与可观测性

### 9.1 错误分类

界面至少区分：

| 类别 | 典型原因 | 用户动作 |
|---|---|---|
| 配置错误 | 地址、端口、Topic、凭据组合或载荷格式无效 | 修正表单，不启动网络操作 |
| 连接错误 | DNS、TCP、TLS 握手或 MQTT CONNACK 拒绝 | 检查目标、证书和认证 |
| 权限错误 | Broker 拒绝发布/订阅或管理命令 | 检查账号、ACL 和管理面配置 |
| 取消/停止 | 用户停止、配置切换或应用退出 | 显示停止进度，等待任务收尾 |
| 驱动能力不足 | 当前编译特性或 Broker 不支持目标操作 | 显示能力差异，不伪造成功 |
| 资源限制 | 载荷、消息、Topic、User Property 或文件超过上限 | 缩小输入或清理时间线 |

错误通知必须包含操作、目标和可执行的下一步；日志使用结构化字段并对凭据和载荷脱敏。

### 9.2 安全规则

1. TLS 校验默认开启；自定义 CA 只能缩小信任范围，不能关闭系统验证后静默连接。
2. 密码输入默认隐藏；调试格式、通知、截图和日志不显示密码或完整 TLS 私钥。
3. 本地服务关闭匿名时，启动前拒绝空用户列表；用户改动认证配置必须停止服务后生效。
4. 绑定所有网卡时显示暴露提示；本地服务不默认暴露到公网。
5. 兼容导入只接受受限 JSON，不执行文件中的路径、命令或表达式。
6. 消息查看器对 JSON、文本和 Base64 使用大小上限；不执行消息内容。

### 9.3 可观测性

每个操作记录 operation、profile_id（如有）、目标地址、耗时、结果和错误类别。订阅额外记录取消原因、消息数量、背压次数和任务退出状态；不记录载荷正文。Windows 原生验收保存截图和操作日志，本机 Docker 集成记录 Compose 服务、镜像、端口、启动和清理状态。

## 10. 分阶段开发计划

### Phase 0：设计和基线收敛（当前）

交付：

- 本文件和主线计划链接。
- 参考项目版本、字段、界面和差距矩阵固定。
- 不提交未经设计确认的行为变更。

退出条件：文档静态检查、术语检查和仓库格式检查通过；工作树只包含本设计提交。

### Phase 1：MQTT Client 交互兼容

范围：

- 将单条 Topic 输入扩展为订阅记录列表，加入 No Local 字段或明确能力禁用。
- 补齐订阅 Topic 列表持久化和订阅/取消订阅逐条状态；列表持久化、暂停展示、恢复展示、清空本地列表和有界消息时间线已完成，后续补齐逐条运行状态和查看器。
- 消息上下文菜单、格式化 JSON/文本查看、UTF-8/Hex/Base64 切换和复制已完成；后续补 JSON 树节点交互。
- 固定参考项目六种载荷格式的回归样例，补回车发送和失败保留输入。

退出条件：

- Domain 对 Topic 列表、No Local 和载荷转换有单元测试。
- Native MQTT Docker 测试验证 3.1.1/5、QoS、Retain、User Property、取消和真实消息。
- ramag-tool-mqtt headless 测试覆盖 360/640/1024/1440px 和关键点击/输入。
- 真实 Windows 窗口完成订阅、发布、格式查看和停止操作；没有窗口证据的部分单独标为未完成。

### Phase 2：本地 MQTT Broker Server 兼容

范围：

- 扩展本地服务驱动，传出真实客户端连接、订阅、发布和断开事件。
- 添加 Broker 注入发布，复用客户端载荷、QoS、Retain 和查看器。
- 添加在线客户端和每客户端订阅列表，标记数据是否完整。
- 评估 TLS 端点、证书选择和本地服务配置持久化。

退出条件：本机内置 Broker 与两个真实客户端完成连接、订阅、发布、保留消息和断开回读；应用退出后端口释放。

### Phase 3：配置兼容和快速导入

范围：

- jsonMCC/jsonMSC 导入、字段映射预览、Ramag 版本化导出和快速配置列表。
- 导入失败不覆盖当前配置，密码和证书路径按本地秘密存储处理。

退出条件：使用固定旧文件样例完成导入、导出、再导入和敏感字段脱敏测试。

### Phase 4：Mosquitto 管理面对齐

范围：

- 保持 Dynamic Security、静态文件和通用 MQTT 数据面分离。
- 补齐能力探测、错误分类、刷新和保存后的 Broker reload/重启提示。

退出条件：本机 Docker Mosquitto 管理测试、headless 管理页面测试和真实窗口最小验收通过。

### Phase 5：发布前质量与证据

范围：

- 完整功能矩阵回归、源码尺寸检查、Windows 原生窗口证据、文档和变更记录。
- 根据需要补充本机 Docker Compose 测试脚本的版本、端口、启动和清理说明。

退出条件：对应功能的测试、UI 证据和真实服务证据都已记录；未完成项不能写成已交付。

## 11. 测试与验收设计

### 11.1 领域与应用测试

- 配置：端口、地址、TLS 字段、凭据组合、MQTT 3.1.1/5 专属字段。
- Topic：发布 Topic 通配符拒绝、订阅通配符规则、共享订阅和重复项。
- 载荷：六种格式的成功和失败样例，非法 Base64/Hex 不发送。
- 本地认证：匿名开启、匿名关闭空用户拒绝、重复用户名和密码边界。
- 生命周期：重复启动、冲突配置、取消、迟到结果和应用退出。
- 脱敏：Debug、日志和错误文本不出现密码、私钥或完整载荷。

### 11.2 Native MQTT 集成测试

统一使用本机 Docker，不使用远程集群：

- 服务建议：ramag-mqtt-test，镜像 eclipse-mosquitto:2.0.20，监听 127.0.0.1:18883。
- 启动方式：scripts/mqtt-test 的 Compose test；测试完成后按脚本 down 或 clean 清理容器和网络。
- 覆盖：MQTT 3.1.1、MQTT 5、QoS 1 回执、Retain、User Property、并发临时 Client ID、空闲订阅两秒内停止。
- 本地 Broker Phase 2 另增加 oximqtt 监听端口、两个真实客户端和注入发布回读。

测试记录必须写明服务名、镜像、端口、启动时间、清理状态和失败原因。Docker 不可用时标记集成测试未完成，不能用 mock 或静态 fixture 代替。

### 11.3 UI 验收

- Headless：GPUI 交互测试覆盖配置选择、Topic 输入、发布、订阅、停止、清空、查看器和 360/640/1024/1440px 布局。
- Native：优先使用真实 Windows MSVC Debug 窗口；记录输入、点击、截图、真实 Broker 消息和停止结果。
- 证据边界：基础设施测试不能代替 UI 验收；headless 测试不能代替真实窗口截图。

### 11.4 提交前命令

每个功能切片完成后至少执行：

1. cargo test -p ramag-domain mqtt
2. cargo test -p ramag-app mqtt
3. cargo test -p ramag-infra-mqtt --no-default-features
4. cargo test -p ramag-infra-mqtt --features native
5. cargo test -p ramag-tool-mqtt
6. cargo fmt --all -- --check
7. cargo clippy --workspace --all-targets -- -D warnings
8. git diff --check

命令按功能风险增减；涉及 Docker、TLS、本地 Broker 或真实窗口时，必须追加对应的集成和 UI 证据。

## 12. 未决项与默认处理

| 未决项 | 影响 | 未决定前的默认处理 |
|---|---|---|
| 发布是否复用正在订阅的 Client 会话 | 影响 Client ID、断线和 UI 并发 | 沿用当前隔离请求；订阅运行时禁用发布，避免隐式第二连接 |
| No Local 的其他驱动支持 | 影响订阅模型和兼容性 | Native MQTT 5 已映射 No Local；MQTT 3.1.1 禁用并明确拒绝，其他驱动按能力禁用并提示 |
| MQTT 3.1 支持 | 当前 Domain 只有 3.1.1/5 | 先保证 3.1.1/5；若要补 3.1，单独增加驱动和 Docker 样例 |
| 本地 Broker TLS | 影响证书模型、配置持久化和端口语义 | Phase 2 调研 oximqtt 能力；未支持前不显示虚假的 TLS 开关 |
| 本地 Broker 在线客户端和订阅 API | 影响 Server 侧客户端管理 | 先显示能力不足，不生成客户端列表 |
| jsonMCC/jsonMSC 密码导入 | 影响秘密迁移 | 导入后重新写入 Ramag 秘密存储，不在普通 JSON 导出中明文回写 |
| SM4 加密发布 | 影响算法、密钥来源和线速载荷 | 暂不实现，避免引入未审查密码学代码 |
| 默认 MQTT 版本和 Keep Alive | 影响新建配置和旧配置导入 | 新配置保持当前 Ramag 默认；旧配置按文件字段，缺失时使用 Domain 默认 |
| 自动重连策略 | 影响订阅停止、Client ID 和失败提示 | 先由当前驱动负责有限重连；Phase 1 再决定是否暴露用户开关和退避参数 |
| Broker 事件时间线格式 | 影响 Server 和 Client 共用查看器 | 先统一为 MqttMessage + 来源枚举，再确定颜色和文案 |

一旦用户或后续实现决定了未决项，必须先更新本文件和功能矩阵，再修改实现。

## 13. 完成定义

MQTT 重写阶段只有同时满足以下条件才可标为完成：

1. 参考项目 MQTT Server/Client 的目标功能在矩阵中有明确的“已实现”或“明确不支持”结论。
2. Domain/App/Infrastructure/Tool 依赖边界没有被 UI 或具体 MQTT 库绕过。
3. 配置、消息、凭据、TLS、取消和有界资源规则有测试。
4. 本机 Docker 集成测试和 UI headless 测试通过；涉及用户可见功能的真实 Windows 窗口证据已单独记录，未完成部分明确标注。
5. fmt、Clippy、功能测试和 git diff --check 针对最终待提交内容通过。
6. 每个独立功能只有一个范围清晰的提交，并立即推送当前功能分支；不合并或推送 main。
