"""受限 HTTP CONNECT 测试代理。

该程序只在本机 Docker 集成测试中使用，不是产品代理实现。它要求 Basic 认证，并把
`localhost`/loopback 的固定测试端口映射到 Compose 网络中的单个服务；任何其他主机或
端口都会返回失败。CONNECT 成功后仅双向转发字节流，因此 HTTPS 与 gRPC 的 TLS/mTLS
握手仍由测试客户端和目标服务完成，代理不会解密或记录凭据、请求正文和响应正文。
"""

import base64
import hmac
import os
import select
import socket
import socketserver
from urllib.parse import urlsplit


AUTHORIZATION = "Basic " + base64.b64encode(
    f"{os.environ.get('PROXY_USERNAME', 'proxy-user')}:{os.environ.get('PROXY_PASSWORD', 'proxy-secret')}".encode()
).decode()
TARGET_SERVICE = os.environ.get("PROXY_TARGET_SERVICE", "")
PORT_MAP = {}
for item in os.environ.get("PROXY_TARGET_PORTS", "").split(","):
    if not item:
        continue
    source, target = item.split(":", 1)
    PORT_MAP[int(source)] = int(target)


def target_address(host, port):
    """将允许的客户端目标映射为 Compose 服务地址，拒绝开放代理行为。"""
    if host not in {"localhost", "127.0.0.1", "::1"}:
        raise ValueError("proxy target host is not allowed")
    if port not in PORT_MAP:
        raise ValueError("proxy target port is not allowed")
    if not TARGET_SERVICE:
        raise ValueError("proxy target service is not configured")
    return TARGET_SERVICE, PORT_MAP[port]


def relay(client, target):
    """在两个已认证的套接字之间有界转发，任一端关闭或空闲超时即结束连接。"""
    sockets = [client, target]
    while True:
        readable, _, _ = select.select(sockets, [], [], 30)
        if not readable:
            return
        for source in readable:
            payload = source.recv(64 * 1024)
            if not payload:
                return
            destination = target if source is client else client
            destination.sendall(payload)


class ProxyHandler(socketserver.StreamRequestHandler):
    """解析单个代理请求，先校验认证，再选择 CONNECT 隧道或明文 HTTP 转发。"""

    def handle(self):
        """读取受限大小的请求头；认证失败时只返回 407，不回显认证信息。"""
        self.connection.settimeout(15)
        request_line = self.rfile.readline(8193)
        if not request_line or len(request_line) > 8192:
            return
        headers = {}
        while True:
            line = self.rfile.readline(8193)
            if not line or len(line) > 8192:
                return
            if line in {b"\r\n", b"\n"}:
                break
            name, value = line.decode("latin-1").rstrip("\r\n").split(":", 1)
            headers[name.lower()] = value.strip()
        if not hmac.compare_digest(headers.get("proxy-authorization", ""), AUTHORIZATION):
            self.wfile.write(
                b"HTTP/1.1 407 Proxy Authentication Required\r\n"
                b"Proxy-Authenticate: Basic realm=\"ramag-api-test\"\r\n"
                b"Content-Length: 0\r\n\r\n"
            )
            return

        method, target, version = request_line.decode("latin-1").rstrip("\r\n").split(" ", 2)
        if method.upper() == "CONNECT":
            self.handle_connect(target)
            return
        self.handle_http(method, target, version, headers)

    def handle_connect(self, target):
        """建立目标 TCP 隧道并转发字节，不解析其中的 TLS 或 HTTP/2 数据。"""
        host, port = split_host_port(target, 443)
        try:
            upstream = socket.create_connection(target_address(host, port), timeout=15)
        except (OSError, ValueError):
            self.wfile.write(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
            return
        self.wfile.write(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        relay(self.connection, upstream)
        upstream.close()

    def handle_http(self, method, target, version, headers):
        """仅转发绝对 HTTP URL，移除代理认证头并限制请求体为 4 MiB。"""
        parsed = urlsplit(target)
        if parsed.scheme not in {"http", ""} or not parsed.hostname:
            self.wfile.write(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
            return
        port = parsed.port or 80
        try:
            upstream = socket.create_connection(
                target_address(parsed.hostname, port), timeout=15
            )
        except (OSError, ValueError):
            self.wfile.write(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
            return
        path = parsed.path or "/"
        if parsed.query:
            path += "?" + parsed.query
        forwarded = [f"{method} {path} {version}\r\n".encode("latin-1")]
        for name, value in headers.items():
            if name in {"proxy-authorization", "proxy-connection"}:
                continue
            forwarded.append(f"{name}: {value}\r\n".encode("latin-1"))
        forwarded.append(b"\r\n")
        content_length = int(headers.get("content-length", "0"))
        if content_length > 4 * 1024 * 1024:
            upstream.close()
            self.wfile.write(b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n")
            return
        body = self.rfile.read(content_length) if content_length else b""
        upstream.sendall(b"".join(forwarded) + body)
        relay(self.connection, upstream)
        upstream.close()

    def log_message(self, *_args):
        return


def split_host_port(value, default_port):
    """拆分 CONNECT authority，并正确保留方括号 IPv6 主机的端口默认值。"""
    if value.startswith("["):
        host, _, port = value[1:].partition("]:")
        return host, int(port or default_port)
    host, separator, port = value.rpartition(":")
    if not separator:
        return value, default_port
    return host, int(port)


class ThreadingProxy(socketserver.ThreadingMixIn, socketserver.TCPServer):
    """每个连接独立线程；daemon 线程保证容器停止时不会阻塞进程退出。"""

    daemon_threads = True
    allow_reuse_address = True


if __name__ == "__main__":
    with ThreadingProxy(("0.0.0.0", 8081), ProxyHandler) as server:
        server.serve_forever()
