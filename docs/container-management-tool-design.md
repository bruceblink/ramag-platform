# Docker 与 Kubernetes 可视化管理工具设计

> 状态：CMT-001、CMT-002 已完成，CMT-003 开发中。本文件定义容器管理工具的产品范围、技术边界和分期；当前实现已接入 Docker Engine 只读适配器、Registry v2 查询适配器、应用服务和工作台列表/详情 UI，真实 Windows 原生窗口证据仍单独记录。
>
> 适用范围：本机 Docker Engine、受控远程 Docker Engine，以及通过 kubeconfig 接入的 Kubernetes 集群。工具作为 Ramag 静态工具插件实现，暂定 crate 名称为 ramag-tool-container。

## 术语与命名规则

| 规范中文名 | English / Acronym | 在本方案中的职责边界 | 不表示什么 |
|---|---|---|---|
| 容器管理工具 | Container Management Tool | Ramag 内置的 Docker 与 Kubernetes 资源查看和受控操作界面 | 不表示 Docker Desktop 的替代安装程序或 Docker Daemon |
| Docker 引擎 | Docker Engine | 执行镜像、容器、网络和卷操作的服务端 API | 不表示 Docker Desktop 产品，也不表示任意远程主机均可连接 |
| Docker 连接配置 | Docker Endpoint Profile | 保存一个 Docker Engine 的地址、TLS 要求、显示名称和只读策略 | 不表示镜像仓库账号或 Kubernetes 上下文 |
| 镜像 | Image | 由名称、标签或 digest 标识的容器文件系统和运行配置 | 不表示正在运行的容器，也不表示镜像仓库 |
| 镜像仓库 | Container Registry | 提供镜像清单与层数据读写的服务 | 不表示本机镜像缓存或 Docker Engine |
| 容器 | Container | Docker Engine 管理的运行实例，具有生命周期、日志、端口和挂载 | 不表示 Kubernetes 工作负载或 Pod |
| 容器网络 | Docker Network | Docker Engine 提供的网络、IPAM、容器连接和 DNS 范围 | 不表示 Kubernetes Service 或 NetworkPolicy |
| 数据卷 | Docker Volume | Docker Engine 管理的持久化数据位置 | 不表示宿主机任意目录或 Kubernetes PersistentVolume |
| 日志流 | Log Stream | 可取消、可限流、按时间范围读取的容器或 Pod 日志输出 | 不表示无限缓存的终端会话或审计日志 |
| Kubernetes 集群 | Kubernetes Cluster | 由 API Server 管理的资源范围 | 不表示单个 Docker Engine 或本机 kind 节点 |
| Kubernetes 上下文 | Kubernetes Context | kubeconfig 中选择集群、用户和默认命名空间的连接记录 | 不表示当前 UI 选择的 Docker 连接配置 |
| 命名空间 | Namespace | Kubernetes 资源的逻辑隔离范围 | 不表示 Docker Network、Windows 用户目录或 Ramag 工具工作区 |
| 工作负载 | Workload | Deployment、StatefulSet、DaemonSet、Job 或 CronJob 等控制器资源 | 不表示单个 Pod，也不表示 Docker Container |
| Pod | Pod | Kubernetes 调度的一个或多个容器运行单元 | 不表示 Docker Container 的同义词 |
| 生产保护模式 | Production Protection | 对被标记为生产的连接提高确认、只读和审计要求 | 不表示自动判断业务风险或绕过 Kubernetes RBAC |

首次出现时使用上表的中文名和英文名；后续只使用中文规范名。Docker Engine 的资源与 Kubernetes 资源必须分别命名，界面不得把容器和 Pod 混成同一种对象。

## 产品定位与目标

容器管理工具服务于需要在一个桌面工作台中完成日常容器排查、镜像核对、服务启动和 Kubernetes 观察的开发者与运维人员。它借鉴 Docker Desktop 中镜像、容器、日志和网络的可发现性，但不依赖 Docker Desktop，也不复制其安装、更新、订阅或集群创建功能。

第一阶段必须实现以下结果：

1. 用户可选择一个 Docker 连接配置，查看 Docker Engine 版本、容器、镜像、容器网络和数据卷，并在只读模式下完成筛选、排序和详情查看。
2. 用户可从本地镜像或已配置镜像仓库选择镜像，填写端口、环境变量、挂载、网络、资源限制和重启策略后启动容器。工具在提交前显示完整参数摘要。
3. 用户可查看、搜索、暂停、继续、导出和取消日志流。日志读取必须有行数、字节数和时间范围上限。
4. 用户可管理 Docker 容器网络：查看、创建、删除、连接容器、断开容器和查看 IPAM 配置；危险操作需要明确确认。
5. 用户可选择 Kubernetes 上下文与命名空间，查看工作负载、Pod、Service、Ingress、事件和 Pod 日志。写入类 Kubernetes 操作在后续阶段逐项开放。
6. 所有写入操作显示执行目标、连接配置、资源标识、请求参数摘要、开始时间、结果和失败原因；生产保护模式默认拒绝高风险操作。

本工具暂不承担以下工作：

- 安装或升级 Docker Desktop、Docker Engine、Kubernetes 集群或节点。
- 创建云账号、云厂商集群、CI/CD 流水线或镜像漏洞扫描服务。
- 默认开放任意 Shell、Docker exec、kubectl exec、任意宿主机路径挂载或任意 YAML apply。
- 读取 Kubernetes Secret 明文、镜像仓库密码、TLS 私钥或其他凭据并显示在界面、日志或审计记录中。
- 用 Docker Container 术语描述 Kubernetes Pod，或用 Kubernetes 工作负载代替 Docker 容器启动向导。

## 连接与权限模型

容器管理工具将连接配置、用户选择和远程资源身份分开保存。每条资源记录都必须带有连接配置 ID、平台类型和远程资源 ID，避免同名容器、同名命名空间或同名镜像在不同端点之间混淆。

| 配置项 | Docker 连接配置 | Kubernetes 上下文 |
|---|---|---|
| 身份 | 稳定的本地配置 ID | 稳定的本地配置 ID 与 kubeconfig context 名称 |
| 地址 | 本机 socket、Windows named pipe，或经 TLS 保护的 TCP 地址 | kubeconfig 指向的 API Server |
| 凭据 | TLS 客户端证书、token 或本机 socket 权限 | kubeconfig 支持的认证信息或外部认证插件结果 |
| 保存位置 | Ramag 配置与秘密存储，日志只保留已脱敏的引用 | Ramag 配置只保留文件引用、context 名称和策略，敏感内容不复制到普通配置 |
| 读取能力 | Engine version、资源列表、详情、日志 | 以实际 RBAC 返回的 API 资源和日志权限为准 |
| 写入能力 | 由连接只读标记、生产保护模式和每项确认共同决定 | 由 Kubernetes RBAC、连接策略和每项确认共同决定 |

Docker 连接配置在第一次使用时执行 API 版本协商和只读健康检查。Kubernetes 上下文在第一次使用时读取 API Server 版本、可访问命名空间和资源发现信息。认证失败、TLS 失败、权限不足、版本不兼容和取消请求必须显示为不同原因，不能统一显示为“连接失败”。

生产保护模式由用户在连接配置中显式设置，也可以由团队设置文件下发。处于生产保护模式的连接默认只读；启用写入前，用户需要重新确认连接配置和资源范围。工具不能根据名称包含 prod 就自动声称连接是生产环境。

## 总体结构

容器管理工具遵循现有 Ramag 分层与静态工具插件路径。界面只提交结构化操作请求，不通过命令行拼接 docker 或 kubectl 指令。应用层负责策略、取消、审计和错误分类；基础设施层分别调用 Docker Engine API 与 Kubernetes API。

~~~mermaid
flowchart LR
    User[用户] -->|选择资源与确认操作| ToolUI[容器管理工具界面]
    ToolUI -->|结构化查询或操作请求| ContainerApp[容器管理应用服务]
    ContainerApp -->|Docker Engine API 请求| DockerBackend[Docker 基础设施适配器]
    ContainerApp -->|Kubernetes API 请求与 Watch| KubernetesBackend[Kubernetes 基础设施适配器]
    ContainerApp -->|连接策略、脱敏审计与取消| PlatformCore[平台底座]
    DockerBackend -->|资源、日志与操作结果| DockerEngine[Docker 引擎]
    KubernetesBackend -->|资源、事件、日志与操作结果| KubernetesApi[Kubernetes API Server]
    DockerEngine -->|镜像拉取与推送| Registry[镜像仓库]
~~~

图中的 Docker 基础设施适配器和 Kubernetes 基础设施适配器是并列实现；容器管理应用服务根据连接配置类型选择其中之一。镜像仓库读写由 Docker 引擎执行；应用层不会把仓库 token 直接传给界面。平台底座只提供受控的配置、秘密存储、任务、通知和审计服务，不拥有 Docker 或 Kubernetes 业务逻辑。

### 建议 crate 边界

| 位置 | 负责内容 | 不负责内容 |
|---|---|---|
| ramag-domain | 连接配置 ID、平台类型、镜像引用、资源摘要、日志查询、操作请求和结果模型 | Docker HTTP 类型、kubeconfig 解析、GPUI 视图 |
| ramag-app | 查询用例、操作前策略检查、取消、状态转换、审计摘要和错误映射 | 直接访问 socket、HTTP 客户端或 UI 元素 |
| ramag-infra-container-docker | Docker Engine API 版本协商、资源查询、镜像和容器操作、日志流 | Kubernetes API、用户确认状态 |
| ramag-infra-container-kubernetes | kubeconfig 上下文读取、Kubernetes 资源查询、Watch、Pod 日志和 RBAC 错误映射 | Docker Engine API、任意 Shell 执行 |
| ramag-tool-container | Activity Bar 入口、资源列表、详情、操作向导、日志视图和 headless UI 测试 | 存储明文凭据、绕过应用层直接调用基础设施适配器 |
| ramag-bin | 静态工具注册、启动和关闭顺序 | 具体容器管理业务规则 |

这些 crate 名称是实施建议。开始 CMT-001 前，先确认现有 workspace 命名和依赖图，避免把候选实现 crate 预先加入 Cargo workspace。

## 功能范围

### 概览与资源导航

概览页显示当前连接配置、连接状态、只读或生产保护状态、Docker Engine 或 Kubernetes 版本、资源数量、最近操作和失败提示。资源数量只作为导航信息，不能触发一次性加载全部详情或全部日志。

Docker 连接配置使用以下导航项：

1. 概览
2. 容器
3. 镜像
4. 容器网络
5. 数据卷
6. 日志
7. 操作记录

Kubernetes 上下文使用以下导航项：

1. 概览
2. 命名空间
3. 工作负载
4. Pod
5. Service 与 Ingress
6. 配置与事件
7. 日志
8. 操作记录

界面在连接切换时取消旧请求、清空旧资源选择并递增连接上下文版本。迟到返回的结果只能更新发起请求时对应的连接配置，不能覆盖新连接的列表或日志。

### 镜像查询与镜像管理

镜像页分为本地镜像和镜像仓库两个来源。两者都显示完整名称、标签、digest、创建时间、大小、平台架构、标签和引用计数；同一名称的不同 digest 必须分别展示。

| 操作 | 第一阶段行为 | 保护规则 |
|---|---|---|
| 查询本地镜像 | 分页、筛选名称与标签、查看详情和历史 | 不加载镜像层文件内容 |
| 查询镜像仓库 | 仅访问用户已配置且已授权的仓库 | 不把仓库 token 返回给界面 |
| 拉取镜像 | 选择名称与明确 tag 或 digest，显示进度与可取消状态 | 默认拒绝浮动 latest 作为生产保护模式的启动来源 |
| 标记与推送 | 选择已有本地镜像与目标引用 | 二次确认目标仓库、命名空间和 tag |
| 删除本地镜像 | 显示受影响容器和标签 | 被运行中容器引用时默认拒绝；强制删除需要额外确认 |
| 导入与导出 | 在后续阶段提供受控文件选择 | 不允许插件或后台任务读取任意路径 |
| 清理未引用镜像 | 后续阶段按预览列表批量执行 | 必须先展示将删除的 digest 和可释放空间估算 |

漏洞、签名和 SBOM 是镜像供应链功能，不能用镜像查询结果冒充。它们在具备可信扫描器、签名验证器和清晰的数据来源后另行设计。

`CMT-003` 分页切片记录（2026-09-17）：Registry v2 仓库目录和 Tag 查询已处理 `Link: rel="next"` 分页，并限制分页次数、返回数量和响应大小；分页 URL 只允许访问当前 Registry 的同源地址。目标 crate 测试使用本机 TCP HTTP fixture 验证多页结果和跨域链接拒绝。

`CMT-003` 认证与 digest 切片记录（2026-09-17）：本机 TCP HTTP fixture 验证 Registry 返回 401 时映射为不可重试的认证错误，响应正文和密码不会进入错误文本；清单读取使用 `HEAD /v2/{repository}/manifests/{reference}`，从 `Docker-Content-Digest` 回读并校验 digest，同时读取 `Content-Length` 和媒体类型。该切片不代表取消、资源清理或镜像拉取/标记/推送/删除的本机服务验收已完成。

### 容器启动与生命周期管理

容器列表必须支持状态、名称、镜像、创建时间、端口、网络、健康状态、标签和资源使用摘要的筛选与排序。详情页显示 inspect 数据的结构化子集；超长标签、环境变量和挂载路径在可滚动区域展示并带有复制操作。

启动容器向导按以下顺序执行：

1. 选择 Docker 连接配置和明确的本地镜像或远程镜像引用。
2. 填写容器名称、命令、工作目录、端口映射、环境变量、数据卷、容器网络、DNS、重启策略、资源限制和健康检查。
3. 验证端口冲突、容器名称冲突、数据卷存在性、容器网络可用性和只读或生产保护限制。
4. 显示不可编辑的请求摘要，包括镜像 digest、环境变量键名、宿主机端口、挂载来源和目标、网络、资源限制以及重启策略。
5. 用户确认后创建容器并启动，操作记录保存服务端返回的容器 ID。

运行中的容器可执行启动、停止、重启、暂停、恢复、删除、查看详情、复制为新配置和打开日志。停止、删除、强制停止和重新创建必须显示影响范围；生产保护模式默认只允许查看日志和详情。Docker exec 与文件浏览不进入初始阶段，因为它们需要单独的终端权限、输入脱敏和宿主机路径安全设计。

### 容器网络与数据卷管理

容器网络页显示 driver、scope、subnet、gateway、IPAM 设置、附加容器、标签和创建时间。创建网络向导只允许平台支持的 driver 和有界 IPAM 输入。连接容器与断开容器必须显示容器当前网络、目标网络、别名和静态 IP；操作完成后通过 Docker Engine 回读确认最终状态。

数据卷页显示 driver、挂载点摘要、标签、大小估算和引用容器。初期只提供查看、创建、删除和挂载选择；浏览卷内文件、迁移卷和绑定任意宿主机目录属于后续功能。删除数据卷时，默认拒绝删除仍被容器引用的卷。

Docker 容器网络和数据卷不会映射为 Kubernetes Service、NetworkPolicy、PersistentVolume 或 PersistentVolumeClaim。Kubernetes 资源在 Kubernetes 导航中独立展示。

### 日志管理

日志视图以连接配置、资源类型、资源 ID、容器名或 Pod 名、容器名、时间范围、tail 行数和 follow 状态构成查询。每个日志流都有独立的取消句柄；切换资源、关闭视图、超过缓冲上限或用户点击停止时立即取消读取。

| 能力 | Docker 容器日志 | Kubernetes Pod 日志 |
|---|---|---|
| 选择对象 | 单个容器 | 命名空间、Pod 与其中一个容器 |
| 时间条件 | since、until、timestamps、tail | sinceTime 或 sinceSeconds、timestamps、tailLines |
| 历史日志 | 当前容器日志 | 当前日志与 previous 容器日志 |
| 实时跟随 | follow 并可暂停、恢复和取消 | API 日志流并可暂停、恢复和取消 |
| 搜索与导出 | 本地有界缓冲内搜索、复制和导出 | 本地有界缓冲内搜索、复制和导出 |

日志缓冲必须限制行数和字节数，超出时删除最早数据并显示已丢弃数量。日志中的 token、密码、Authorization header、私钥样式内容和连接配置秘密应在进入 UI、剪贴板和操作记录前经过可配置脱敏；脱敏失败时宁可隐藏该字段，也不能显示原文。

### Kubernetes 资源管理

Kubernetes 第一阶段以读取、筛选、详情、事件和日志为主。用户选择 Kubernetes 上下文后，再选择命名空间；集群级资源和命名空间级资源在列表上明确区分。资源读取使用 API discovery 识别 API 版本与可用资源，不通过调用本机 kubectl 实现。

| 资源 | 初始能力 | 后续受控操作 |
|---|---|---|
| Namespace | 列表、选择、标签、状态 | 创建和删除在独立阶段评估 |
| Deployment、StatefulSet、DaemonSet | 副本数、镜像、条件、事件、Pod 关联 | 扩缩容、暂停、恢复、滚动重启 |
| Job、CronJob | 状态、计划、最近执行和事件 | 创建、暂停、立即执行和删除 |
| Pod | 状态、容器、重启次数、事件和日志 | 删除仅在明确策略和权限确认后开放 |
| Service、Ingress | 端口、selector、endpoint 摘要和规则 | 编辑前先提供只读 diff 和验证 |
| ConfigMap | 元数据和非敏感键摘要 | 结构化编辑与回读在后续阶段开放 |
| Secret | 仅名称、类型、键名和元数据 | 默认不显示值，也不提供编辑器 |
| NetworkPolicy | 列表、规则摘要与命名空间关系 | 创建和修改在专门网络策略阶段评估 |

YAML 查看器只能显示从 API Server 读取并经过秘密字段隐藏的内容。YAML apply、kubectl exec、port-forward、节点管理、CRD 编辑和 Helm release 管理都不是初始范围；每一项都需要独立的操作模型、权限说明和本机 kind 测试。

## UI 信息架构与交互规则

容器管理工具作为一个 Activity Bar 工具入口。桌面宽度下使用连接选择器、左侧资源导航、中心资源列表和右侧详情区；窄窗口下保留连接选择器和当前资源列表，详情以可关闭页面或对话框显示。不能通过固定三栏宽度挤压日志、镜像名、资源 ID 或危险操作按钮。

| 区域 | 必须显示 | 紧凑窗口行为 |
|---|---|---|
| 顶部连接栏 | 当前连接、平台类型、只读或生产保护状态、刷新状态 | 连接名称可省略，状态图标保留并提供提示 |
| 资源导航 | 当前平台可用的资源类别和数量摘要 | 变为可打开的导航面板，不能覆盖主要确认对话框 |
| 资源列表 | 筛选、排序、加载、空状态、错误与分页或虚拟列表 | 优先保留资源名、状态与危险状态；次要列折叠到详情 |
| 详情区 | 身份、状态、标签、关联资源、最近操作和允许操作 | 改为独立可返回页面，长字段可复制和滚动 |
| 操作向导 | 参数校验、请求摘要、确认与执行结果 | 单列布局，底部取消与执行操作始终可见 |
| 日志区 | 查询范围、follow、停止、搜索、缓冲丢弃提示 | 工具栏换行，日志内容保留可滚动区域 |

所有资源操作有 Pending、Running、Succeeded、Failed、Cancelled 和 Stale 状态。按钮在 Pending 或 Running 时保持稳定尺寸并阻止重复提交。失败后保留可安全重试的表单输入，但不回填已经脱敏的秘密字段。

危险操作分为三类：

1. 可恢复操作：停止容器、暂停容器、断开网络。显示一次确认，操作记录保留恢复提示。
2. 不可恢复的局部操作：删除容器、镜像、数据卷、Docker Network、Kubernetes Job 或 Pod。显示资源身份、连接配置、影响数量和输入确认。
3. 高影响操作：批量清理、镜像推送、生产工作负载扩缩容、滚动重启和未来的 YAML apply。要求生产保护策略允许、显示完整目标清单并逐项记录结果。

## 应用层接口与运行规则

应用层不暴露基础设施 SDK 类型。建议以平台无关的模型组织请求：

~~~text
ContainerEndpointId
ContainerPlatform = Docker | Kubernetes
ImageReference = registry / repository / tag-or-digest
ContainerOperationRequest = create / start / stop / restart / remove
NetworkOperationRequest = create / remove / connect / disconnect
LogQuery = target / time-range / tail-limit / follow
KubernetesTarget = context / namespace / resource-kind / resource-name
OperationReceipt = operation-id / target / state / timestamps / summary
~~~

基础设施适配器提供分页查询、详情查询、可取消日志流和有界 Watch。应用层为每个写入请求生成操作 ID，并在请求开始、完成、失败或取消时记录连接配置 ID、资源身份、动作、耗时和经过脱敏的参数摘要。重试只针对明确标识为暂时错误的读取或 Watch 重连；删除、推送和启动容器不能因为网络超时自动重试。

日志流和 Kubernetes Watch 使用有界 channel。消费者变慢时，基础设施适配器停止继续积压数据，应用层报告丢弃数量或重新同步，而不是在内存中无限保留事件。所有后台任务都绑定连接上下文版本，连接切换或工具关闭后拒绝迟到结果。

## 安全与审计要求

| 风险 | 必须的设计措施 | 验收证据 |
|---|---|---|
| Docker TCP 端点被误连 | 明确显示地址、TLS 状态和服务器版本；未使用 TLS 的远程端点默认拒绝 | 连接测试分别覆盖本机 socket、TLS 成功、TLS 失败和拒绝明文远程 TCP |
| kubeconfig 或仓库凭据泄露 | 普通配置只保存引用；秘密存储管理敏感值；日志、错误和导出内容统一脱敏 | 单元测试检查错误、审计摘要、日志导出和复制内容不含秘密 |
| 生产资源被误操作 | 默认只读、显式写入授权、二次确认、目标回读和操作记录 | UI 测试检查确认前没有写入；集成测试回读目标状态 |
| 命令或标签注入 | 不拼接 shell；用 SDK 或 HTTP 结构化请求；限制名称、标签、环境变量和日志查询大小 | 恶意名称、换行、控制字符和超长输入测试被拒绝或安全编码 |
| 无界日志与 Watch 占用内存 | 行数、字节数、并发流数、超时和取消上限 | 压力测试验证达到上限后仍可取消且界面保持响应 |
| Kubernetes 权限理解错误 | 将 API 的 401、403、资源未发现和命名空间不存在分开显示 | kind 集成测试分别覆盖以上响应 |
| 供应链来源不清 | 镜像操作显示 registry、digest 与平台；不把 tag 当作不可变身份 | 镜像拉取与启动测试断言记录的 digest 和实际镜像一致 |

操作记录是 Ramag 本地的经过脱敏的审计摘要，不是 Kubernetes Audit Log 或 Docker Engine 原始日志。用户可以导出操作记录，但导出前再次执行秘密字段隐藏。

## 本机 Docker 与 UI 验收计划

每个涉及 Docker 或 Kubernetes API 的集成测试只能使用本机 Docker 环境。Docker 操作测试使用专用标签 ramag.test-suite=true 创建容器、镜像标签、容器网络和数据卷；测试结束后按资源 ID 与标签删除它们，并报告服务、镜像版本、端口、启动状态和清理结果。禁止使用开发者真实集群、远程 Docker Engine、生产镜像仓库或生产 Kubernetes 集群代替本机测试环境。

| 测试层 | 本机环境 | 核对内容 |
|---|---|---|
| Docker Engine 集成测试 | 当前机器的 Docker Engine，测试创建唯一名称与专用标签资源 | 连接、列表、详情、启动、停止、网络连接、日志、取消、错误映射和清理 |
| 镜像仓库集成测试 | 本机 Docker Compose 启动的专用 Registry，端口与镜像版本写入测试记录 | 查询、拉取、标记、推送、认证失败、digest 回读和清理 |
| Kubernetes 集成测试 | 本机 Docker 上运行的 kind 集群，测试专用 kubeconfig 与命名空间 | 上下文、命名空间、工作负载、Pod、事件、日志、RBAC 拒绝和命名空间清理 |
| 应用层测试 | 无网络的模拟基础设施适配器 | 策略、取消、连接上下文隔离、脱敏、重试边界和操作状态 |
| headless UI 测试 | GPUI TestAppContext | 360、800、1024、1440 宽度下的列表、详情、日志、操作向导和确认对话框边界 |
| 真实 Windows UI 验收 | 本机 Debug 构建与上述 Docker 服务 | 连接选择、镜像查询、启动容器、网络操作、日志停止和危险操作确认；截图与输入记录单独保存 |

Docker 或 kind 不可用时，相关集成测试必须标记为未完成。模拟基础设施适配器、编译成功或 headless UI 结果不能证明 Docker Engine 或 Kubernetes API 集成完成。真实 Windows UI 无法启动时，至少保留 headless 渲染与交互测试，并明确真实窗口证据缺失。

## 分期与独立提交

| 任务 | 范围 | 交付物 | 验收条件 |
|---|---|---|---|
| CMT-001 | 工具身份、术语、领域模型与静态入口 | 容器管理工具空工作台、连接配置模型、设计更新 | 工具注册不影响现有工具；连接配置模型、平台区分和列表空状态有单元与 headless UI 测试 |
| CMT-002 | Docker 只读查询 | Docker 连接测试、概览、容器、镜像、容器网络和数据卷查看 | 本机 Docker Engine 集成测试覆盖版本、分页、详情和权限错误；真实或 headless UI 验收通过 |
| CMT-003 | 镜像仓库与镜像操作 | 本地镜像、仓库查询、拉取、标签、推送和删除预览 | 本机 Registry 测试覆盖成功、认证失败、digest 回读、取消和清理 |
| CMT-004 | 容器启动与生命周期 | 创建向导、参数校验、启动、停止、重启、暂停和删除 | 本机 Docker 测试回读端口、环境变量键、网络、卷和最终状态；确认前不写入 |
| CMT-005 | Docker 日志 | 查询范围、follow、停止、搜索、导出和缓冲上限 | 本机容器持续写日志；测试验证取消、截断提示、脱敏和窗口响应 |
| CMT-006 | 容器网络与数据卷 | 创建、详情、连接、断开、删除与影响预览 | 本机 Docker 测试验证 IPAM、容器关联、引用保护和标签清理 |
| CMT-007 | Kubernetes 只读观察 | kubeconfig 上下文、命名空间、工作负载、Pod、Service、Ingress、事件和日志 | 本机 kind 测试覆盖资源读取、日志、RBAC 拒绝和命名空间清理 |
| CMT-008 | Kubernetes 受控操作 | 扩缩容、滚动重启、Job 操作和受限 YAML diff | kind 测试验证确认前不写入、RBAC、状态回读、失败保留输入和操作记录 |
| CMT-009 | 安全与运维收尾 | 生产保护、脱敏、操作记录、性能限制和恢复 | 威胁场景、长日志、连接切换、取消、错误恢复、真实 Windows UI 和完整 workspace 检查通过 |

每个任务只完成一个可验收功能并对应一个提交。开始下一任务前，前一任务的本机 Docker 或 kind 集成测试、风险匹配的 UI 验收、cargo fmt --all -- --check、cargo clippy --workspace --all-targets -- -D warnings、源码尺寸检查和 git diff --check 必须通过。

## 实施前待确认事项

1. Docker Engine 和 Kubernetes API Rust 客户端依赖需在 CMT-001 的技术验证中比较维护状态、Windows named pipe 支持、TLS、流式日志、API 版本协商、取消能力和许可证；本文不预先锁定某个 crate。
2. Kubernetes 认证需要明确支持范围。初期应先支持静态 kubeconfig 凭据、客户端证书和已存在的 token；执行外部认证插件前需要单独评估子进程权限与秘密传递。
3. 镜像仓库范围需要先限定为 Docker Registry v2 兼容服务和用户明确配置的地址；云厂商专用登录流程不进入初始阶段。
4. 团队策略的存储位置、生产保护模式的默认值和操作记录保留时间需要与 Ramag 平台设置设计共同确定。
5. Docker exec、kubectl exec、port-forward、文件浏览、Helm 和 CRD 编辑应各自形成设计与测试计划后再进入实现，不应随容器启动或日志功能一并开放。

## 与现有平台的关系

容器管理工具使用现有静态工具插件、ToolRegistry、Shell、共享通知、设置和任务能力，不引入第三方动态插件加载。平台插件边界见 [plugin-platform-roadmap.md](plugin-platform-roadmap.md)，分层边界见 [architecture.md](architecture.md)。本文新增的 Docker 与 Kubernetes 范围不会改变数据库、Kafka、SSH、对象存储或现有插件生命周期的责任。
