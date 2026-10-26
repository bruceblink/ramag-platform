# WSL Docker 开发环境

> 状态：已配置；计划任务运行检查通过，实际登录触发尚未验证
> 更新日期：2026-10-26
> 执行入口：[最新开发计划](01-development-plan.md)

## 1. 范围与验收

本环境使用 Windows 上的 `Ubuntu-24.04` WSL 2 发行版和发行版内的 Docker Engine。
`docker.service` 和 `containerd.service` 由 systemd 管理。Windows 当前用户登录时，计划任务启动 WSL 并保持发行版运行。
任务在用户登录后运行；本次没有重启 Windows，所以没有验证重启后的实际启动过程。

本次验收条件如下：

- Docker 两项 systemd 服务已启用并处于运行状态。
- 当前用户登录任务已启用，任务在终端退出后仍处于运行状态。
- 仓库脚本固定连接 WSL 的 `/var/run/docker.sock`，并保留 Docker 参数边界。
- PostgreSQL 测试容器健康，Windows 可连接 `127.0.0.1:15432` 并执行只读 SQL。
- 卸载 Docker Desktop 后，WSL Docker Engine 仍可使用。

回滚范围是 Windows 计划任务、WSL 中两个 Docker systemd 服务的启用状态和本仓库的脚本、文档。计划任务通过下列命令移除：

```powershell
Unregister-ScheduledTask -TaskName 'Ramag-WSL-Docker-Ubuntu-24.04' -Confirm:$false
```

回滚时可在 WSL 内执行 `sudo systemctl disable --now docker.service containerd.service`。
卸载 Docker Desktop 会移除 Desktop 管理的容器、镜像和卷；Docker 官方文档明确说明此数据影响。
本次没有执行清理 Desktop 数据目录的操作。WSL 的 `/var/lib/docker` 属于独立数据目录。

## 2. 使用方法

在仓库根目录的 PowerShell 7.2+ 中执行：

```powershell
.\scripts\windows\invoke-wsl-docker.ps1 info
.\scripts\windows\invoke-wsl-docker.ps1 ps
.\scripts\windows\invoke-wsl-docker.ps1 compose version
.\scripts\windows\invoke-wsl-docker.ps1 ps --format '{{.Names}}|{{.Status}}|{{.Ports}}'
```

Docker 命令的单短横线参数（例如 `-d`）可能被 PowerShell 当作脚本参数。此时先创建参数数组，再传给 `-DockerArguments`：

```powershell
$dockerArguments = @('ps', '-a')
.\scripts\windows\invoke-wsl-docker.ps1 -DockerArguments $dockerArguments
```

首次配置或修复登录启动任务时执行：

```powershell
.\scripts\windows\invoke-wsl-docker.ps1 -ConfigureStartup
```

脚本默认使用 `Ubuntu-24.04`。其他发行版可通过 `-Distribution` 指定，但该发行版必须已配置 Docker Engine 和 systemd。
脚本显式连接 `unix:///var/run/docker.sock`，不依赖 Docker Desktop context。容器挂载路径应使用 `/mnt/f/...` 等 WSL 路径。

停止当前保持任务但保留下次登录触发器：

```powershell
.\scripts\windows\invoke-wsl-docker.ps1 -StopKeepAlive
```

该入口不会执行 `wsl --shutdown`。如果任务停止，下一次调用仓库脚本会重新启动它。
WSL 中的测试容器使用原有的重启策略；此任务只负责 WSL 和 Docker Engine，不会改动各容器的启动策略。

## 3. 当前环境与网络

2026-10-26 检查结果：Ubuntu `24.04.4 LTS`、WSL `2.7.3.0`、Linux `6.6.114.1`、Docker Engine `29.1.3`、
Docker Compose `2.40.3`。现有 Docker 由 Ubuntu `docker.io` 软件包提供。当前用户已在 `docker` 组内。

WSL 中已有 `ramag-db-test` 网络和四个项目测试容器。PostgreSQL 容器保存了 `127.0.0.1:15432` 发布配置，
但缺少实际网络端点。执行下列命令恢复网络连接后，Docker 显示了已发布的端口，Windows TCP 检查通过：

```powershell
.\scripts\windows\invoke-wsl-docker.ps1 network connect ramag-db-test ramag-db-test-postgres
```

如果容器已经连接该网络，不必重复执行。该容器使用 `postgres:17-alpine`，实际 PostgreSQL 版本为 `17.11`。
首次启动创建了 WSL 独立测试卷；该卷不包含 Docker Desktop 环境中的测试数据。

为避免卸载 Docker Desktop 后，WSL Docker CLI 继续调用 Desktop 凭据程序，本次从 WSL Docker 配置中移除了
`credsStore: desktop.exe` 字段。更新过程保留 JSON 中其他字段，不输出或复制凭据。

## 4. 验证记录

- `systemctl enable --now docker.service containerd.service` 已完成。卸载 Docker Desktop 后，两项服务仍为 `enabled` 和 `active`。
- 计划任务 `Ramag-WSL-Docker-Ubuntu-24.04` 使用当前 Windows 用户的登录触发器，任务运行级别为普通用户。
- 任务调用 Windows PowerShell 启动 `wsl.exe --distribution Ubuntu-24.04 --cd / --exec /usr/bin/sleep 2147483647`，
  并抑制启动命令的输出。工作目录为 `%WINDIR%\System32`。
  任务状态保持为 `Running`，且其状态不依赖调用它的 PowerShell 终端。
- Docker Desktop 官方卸载程序返回退出码 `0`。卸载后，Docker Desktop 的卸载项、安装程序和进程均不存在。
- 卸载程序关闭了 WSL。手动启动登录任务后，Docker Engine `29.1.3` 和 Compose `2.40.3` 均可用。
- PostgreSQL 测试容器状态为 `healthy`。只读查询返回 `ramag_test|17.11`，Windows 对 `127.0.0.1:15432` 的 TCP 检查通过。
- 测试容器保留原有启动策略。卸载程序关闭 WSL 后，首次验收需要手动执行 `start ramag-db-test-postgres`。
- Windows 重启、用户注销后由触发器自动启动以及真实应用数据库流程尚未验证。

## 5. 官方参考

- [Docker Engine 的 Ubuntu 安装说明](https://docs.docker.com/engine/install/ubuntu/)
- [Docker Engine 的 systemd 启动说明](https://docs.docker.com/engine/daemon/start/)
- [Docker Desktop 卸载说明](https://docs.docker.com/desktop/uninstall/)
- [WSL 网络与 localhost 访问](https://learn.microsoft.com/windows/wsl/networking)
- [WSL 高级配置与 localhostForwarding](https://learn.microsoft.com/windows/wsl/wsl-config)
