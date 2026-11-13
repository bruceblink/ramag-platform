#!/usr/bin/env bash
# 在本机 Docker 中启动短生命周期 HTTPS fixture，运行插件目录真实链路验收。

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
fixture_root=$(mktemp -d "${TMPDIR:-/tmp}/ramag-plugin-catalog.XXXXXX")
container_name="ramag-plugin-catalog-$(basename "$fixture_root")"
image="nginx:1.27-alpine"
container_started=0

cleanup() {
    if [[ "$container_started" -eq 1 ]]; then
        docker rm --force "$container_name" >/dev/null 2>&1 || true
    fi
    rm -rf -- "$fixture_root"
}
trap cleanup EXIT

for command in docker openssl curl python3 cargo; do
    command -v "$command" >/dev/null 2>&1 || {
        echo "缺少本地依赖：$command" >&2
        exit 1
    }
done

mkdir -p "$fixture_root/html/plugins" "$fixture_root/certs" "$fixture_root/conf" "$fixture_root/logs"

openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
    -keyout "$fixture_root/certs/github.key" \
    -out "$fixture_root/certs/github.crt" \
    -subj "/CN=github.com" \
    -addext "subjectAltName=DNS:github.com" \
    >/dev/null 2>&1

cat >"$fixture_root/html/plugins/catalog.json" <<'JSON'
{
  "format_version": 1,
  "entries": [
    {
      "id": "remote.fixture",
      "name": "远程测试插件",
      "version": "1.0.0",
      "api_version": { "major": 1, "minor": 0 },
      "runtime": "process",
      "package_url": "https://github.com/plugins/remote-fixture.tar.gz",
      "archive_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
      "capabilities": ["ui.entry"]
    }
  ]
}
JSON

dd if=/dev/zero of="$fixture_root/html/plugins/oversized.json" bs=1024 count=1025 \
    >/dev/null 2>&1

cat >"$fixture_root/conf/default.conf" <<'NGINX'
server {
    listen 443 ssl;
    server_name github.com;

    ssl_certificate /etc/nginx/certs/github.crt;
    ssl_certificate_key /etc/nginx/certs/github.key;
    access_log /var/log/nginx/access.log combined;
    error_log /var/log/nginx/error.log warn;

    location = /health {
        default_type text/plain;
        return 200 "ok\n";
    }

    location = /plugins/not-found.json {
        return 404;
    }

    location = /plugins/redirect.json {
        return 302 https://github.com/plugins/catalog.json;
    }

    location / {
        root /usr/share/nginx/html;
        try_files $uri =404;
    }
}
NGINX

port=$(python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
)

if ! docker image inspect "$image" >/dev/null 2>&1; then
    echo "拉取本地 HTTPS fixture 镜像：$image"
    docker pull "$image"
fi

echo "插件目录 fixture：$fixture_root"
echo "HTTPS 端口：127.0.0.1:$port"
echo "Nginx 镜像：$image"

docker run --detach \
    --restart unless-stopped \
    --name "$container_name" \
    --publish "127.0.0.1:${port}:443" \
    --mount "type=bind,source=$fixture_root/html,target=/usr/share/nginx/html,readonly" \
    --mount "type=bind,source=$fixture_root/logs,target=/var/log/nginx" \
    --mount "type=bind,source=$fixture_root/conf/default.conf,target=/etc/nginx/conf.d/default.conf,readonly" \
    --mount "type=bind,source=$fixture_root/certs,target=/etc/nginx/certs,readonly" \
    "$image" >/dev/null
container_started=1

container_status=$(docker inspect --format '{{.State.Status}}' "$container_name")
[[ "$container_status" == "running" ]] || {
    docker logs "$container_name" >&2 || true
    echo "本地 HTTPS fixture 容器未处于运行状态：$container_status" >&2
    exit 1
}
published_port=$(docker port "$container_name" 443/tcp)
[[ "$published_port" == *"127.0.0.1:${port}"* ]] || {
    echo "本地 HTTPS fixture 端口映射异常：$published_port" >&2
    exit 1
}
echo "容器状态：$container_status"
echo "端口映射：$published_port"

healthy=0
for _ in $(seq 1 60); do
    if curl --noproxy '*' --insecure --silent --show-error \
        "https://127.0.0.1:${port}/health" | grep -qx 'ok'; then
        healthy=1
        break
    fi
    sleep 0.25
done
if [[ "$healthy" -ne 1 ]]; then
    docker logs "$container_name" >&2 || true
    echo "本地 HTTPS fixture 未能通过健康检查" >&2
    exit 1
fi

cd "$repo_root"
RAMAG_PLUGIN_CATALOG_TEST_ROOT="$fixture_root" \
RAMAG_PLUGIN_CATALOG_TEST_PORT="$port" \
cargo test --locked -p ramag-app \
    --lib plugin_package_catalog_fetcher::tests::fetches_catalog_over_local_https_fixture \
    -- --ignored --test-threads=1
