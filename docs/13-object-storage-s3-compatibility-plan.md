# S3 兼容对象存储与本机集成测试

> 切片：`A-POST-0.5-S3-COMPAT-001`
> 日期：2026-10-16
> 基线：`244dccda`；当前分支 `main`
> 状态：已完成；目标测试、全工作区检查、本机 Docker 和实际 Windows 窗口验证通过

## 问题与范围

对象存储账号目前只支持腾讯云 COS 和阿里云 OSS。Endpoint 必须匹配对应厂商的 HTTPS 主机。本机 Docker 中的 RustFS 使用 S3 兼容协议和本地 Endpoint，因此现有账号流程无法连接它。仓库已有 RustFS 镜像和专用测试容器，但当前 live 测试只覆盖需要云账号凭据的 COS/OSS。

本切片增加 S3 兼容账号类型。用户可以设置服务 Endpoint、Region、Bucket 和凭据。Endpoint 只允许 HTTPS；HTTP 只允许 `localhost`、`127.0.0.1` 和 `[::1]`。Endpoint 不允许用户信息、路径、查询参数或片段。S3 请求使用路径式寻址。旧账号缺少新字段时继续读取为默认值。

本机集成只使用一个新建、短时运行的 RustFS 容器和测试数据。不得启动或修改已有数据库容器、已有 RustFS 容器或数据卷。测试凭据只在当前 PowerShell 进程和临时容器环境中传递，不输出到日志或仓库文件。

本切片不更改 COS/OSS Endpoint 规则，不增加动态服务商发现，不保存额外账号凭据，不操作用户的远端 Bucket 或对象。

## 验收条件

- 设置页可创建和编辑 S3 兼容账号。账号和 Bucket 验证错误会显示给用户。
- 旧 COS/OSS 账号 JSON 在缺少新 Endpoint 字段时可读取并通过验证。
- Endpoint 校验拒绝非 HTTPS 远程地址、HTTP 非回环地址、用户信息、路径、查询参数和片段。
- S3 兼容驱动通过路径式寻址连接指定 Endpoint，并支持列出、读取元数据、上传、下载和删除对象。
- 只读账号不能上传或删除。测试对象使用唯一前缀；即使操作失败，也执行删除和驱动关闭，并报告残留对象键。
- 完整程序在实际 Windows 窗口中可选择 S3 兼容服务、显示 Endpoint 输入并使用 `us-east-1` 默认地域；Endpoint 输入可接受本机回环地址。
- 本机 Docker RustFS live 测试通过。测试记录 Engine、镜像 digest、端口、健康状态、测试结果与容器清理结果。

## 验证

- 先运行 `ramag-domain`、`ramag-app`、`ramag-infra-object-storage` 和 `ramag-tool-object-storage` 目标测试。
- 运行本机 Docker RustFS live 测试，再运行完整 workspace 测试。
- 运行 `cargo fmt --all -- --check`、workspace 严格 Clippy、Windows 源码尺寸检查、`git diff --check` 和 `cargo build --locked -p ramag-bin`。
- Computer Use 使用真实 Windows 窗口检查账号表单；Headless 测试不能代替该检查。保存账号后的列表行和宽度状态留给队列中的对象存储列表验收切片。
- Docker 预定服务：`rustfs/rustfs:latest`，本机已有镜像 digest `sha256:1803faef57627e2d9c2e7d89d655d712ddded5389040054987163043fecb6a3c`；新容器使用 `127.0.0.1:19010`（S3 API）和 `127.0.0.1:19011`（Console）。启动和健康检查后运行测试，再删除本次创建的容器。使用临时容器可写层，不创建命名数据卷。

## 验证结果

- `cargo test --locked --workspace --all-targets -- --test-threads=1 --quiet`：所有已执行测试摘要均通过；忽略项保留各自的凭据或性能测试条件。
- 四个受影响 crate 的定向测试：584 passed、0 failed；COS、OSS 和 S3 live 测试由独立运行流程控制。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。`cargo build --locked -p ramag-bin`：通过。
- `cargo fmt --all -- --check`、`scripts/windows/check-source-size.ps1` 和 `git diff --check`：通过。
- Computer Use：在 1202×812 的实际 Windows 窗口打开对象存储工具和新建账号表单；选择 S3 后显示 Endpoint 输入并预填 `us-east-1`；输入回环地址后取消，未保存账号。
- Docker Engine 29.7.2；镜像 `rustfs/rustfs:latest`，digest `sha256:1803faef57627e2d9c2e7d89d655d712ddded5389040054987163043fecb6a3c`；S3 API `127.0.0.1:19010`，Console `127.0.0.1:19011`。就绪检查返回 HTTP 200；使用一次性 Bucket 的 S3 live 测试通过列表、上传、重复上传拒绝、元数据、下载内容校验、只读上传和删除拒绝、删除清理及 driver shutdown；测试容器已删除。
- 测试期间未启动、停止或修改已有数据库容器、旧 RustFS 容器或数据卷。真实账号列表持久化和对象行宽度验收留给后续对象存储列表切片。

## 回滚边界

回滚 S3 兼容账号枚举值、Endpoint 字段与验证、OpenDAL S3 适配、设置页输入、对应测试和本计划记录。保留现有 COS/OSS 账号、用户数据、凭据和数据库容器。
