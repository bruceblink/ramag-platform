#!/usr/bin/env bash
# 在本机 Docker 中启动短生命周期 HTTPS fixture，运行插件包下载真实链路验收。

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
fixture_root=$(mktemp -d "${TMPDIR:-/tmp}/ramag-plugin-download.XXXXXX")
container_name="ramag-plugin-download-$(basename "$fixture_root")"
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

python3 - "$fixture_root/html/plugins/remote-fixture.tar.gz" <<'PY'
import io
import sys
import tarfile

archive_path = sys.argv[1]
manifest = b'{"id":"download.fixture","version":"1.0.0"}\n'
with tarfile.open(archive_path, "w:gz") as archive:
    entry = tarfile.TarInfo("plugin.json")
    entry.size = len(manifest)
    entry.mtime = 0
    archive.addfile(entry, io.BytesIO(manifest))
PY

package_sha256=$(python3 - "$fixture_root/html/plugins/remote-fixture.tar.gz" <<'PY'
import hashlib
import sys

digest = hashlib.sha256()
with open(sys.argv[1], "rb") as archive:
    for chunk in iter(lambda: archive.read(1024 * 1024), b""):
        digest.update(chunk)
print(digest.hexdigest())
PY
)
dd if=/dev/zero of="$fixture_root/html/plugins/oversized.tar.gz" bs=1M count=65 \
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

    location = /plugins/package-not-found.tar.gz {
        return 404;
    }

    location = /plugins/package-redirect.tar.gz {
        return 302 https://github.com/plugins/remote-fixture.tar.gz;
    }

    location = /plugins/package-oversized.tar.gz {
        alias /usr/share/nginx/html/plugins/oversized.tar.gz;
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

echo "插件包下载 fixture：$fixture_root"
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
RAMAG_PLUGIN_DOWNLOAD_TEST_ROOT="$fixture_root" \
RAMAG_PLUGIN_DOWNLOAD_TEST_PORT="$port" \
RAMAG_PLUGIN_DOWNLOAD_TEST_SHA256="$package_sha256" \
cargo test --locked -p ramag-app \
    --lib plugin_package_downloader::tests::downloads_plugin_package_over_local_https_fixture \
    -- --ignored --test-threads=1
