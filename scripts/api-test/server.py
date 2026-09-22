import base64
import json
import os
import ssl
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse


AUTHORIZATION = "Basic " + base64.b64encode(b"api-user:api-pass").decode("ascii")
OAUTH_CLIENT_AUTHORIZATION = "Basic " + base64.b64encode(b"oauth-client:oauth-secret").decode("ascii")
OAUTH_ACCESS_TOKEN = "docker-oauth-token"
OAUTH_RESOURCE_REQUESTS = 0
OAUTH_RESOURCE_REQUESTS_LOCK = threading.Lock()


class ApiHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, format_string, *args):
        return

    def send_json(self, status, value, headers=None):
        body = json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.send_header("X-Ramag-Docker", "api-http")
        for name, header_value in (headers or {}).items():
            self.send_header(name, header_value)
        self.end_headers()
        try:
            self.wfile.write(body)
            self.wfile.flush()
        except BrokenPipeError:
            pass

    def read_body(self):
        length = int(self.headers.get("Content-Length", "0"))
        return self.rfile.read(max(0, min(length, 4 * 1024 * 1024)))

    def do_GET(self):
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query, keep_blank_values=True)
        if parsed.path == "/health":
            self.send_json(200, {"status": "healthy"})
        elif parsed.path == "/json":
            self.send_json(200, {"ok": True, "service": "ramag-api-http-test"})
        elif parsed.path == "/error":
            self.send_json(418, {"error": "teapot"}, {"X-Error-Case": "non-2xx"})
        elif parsed.path == "/auth":
            if self.headers.get("Authorization") != AUTHORIZATION:
                self.send_json(
                    401,
                    {"authenticated": False},
                    {"WWW-Authenticate": 'Basic realm="ramag-api"'},
                )
            else:
                self.send_json(200, {"authenticated": True})
        elif parsed.path == "/oauth-protected":
            global OAUTH_RESOURCE_REQUESTS
            authorized = self.headers.get("Authorization") == f"Bearer {OAUTH_ACCESS_TOKEN}"
            first_request = False
            if authorized:
                with OAUTH_RESOURCE_REQUESTS_LOCK:
                    first_request = OAUTH_RESOURCE_REQUESTS == 0
                    OAUTH_RESOURCE_REQUESTS += 1
            if first_request or not authorized:
                self.send_json(
                    401,
                    {"authenticated": False},
                    {"WWW-Authenticate": "Bearer"},
                )
            else:
                self.send_json(200, {"authenticated": True, "oauth2": True})
        elif parsed.path == "/delay":
            milliseconds = int(query.get("ms", ["0"])[0])
            time.sleep(max(0, min(milliseconds, 5_000)) / 1_000)
            self.send_json(200, {"delayed_millis": milliseconds})
        elif parsed.path == "/stream":
            self.send_stream()
        elif parsed.path.startswith("/echo/"):
            self.send_echo(parsed.path, query, b"")
        else:
            self.send_json(404, {"error": "not-found"})

    def do_POST(self):
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query, keep_blank_values=True)
        if parsed.path == "/oauth/token":
            self.send_oauth_token(self.read_body())
        elif parsed.path.startswith("/echo/"):
            self.send_echo(parsed.path, query, self.read_body())
        elif parsed.path == "/multipart":
            self.send_multipart(self.read_body())
        else:
            self.send_json(404, {"error": "not-found"})

    def send_oauth_token(self, body):
        form = parse_qs(body.decode("utf-8", errors="replace"), keep_blank_values=True)
        valid = (
            self.headers.get("Authorization") == OAUTH_CLIENT_AUTHORIZATION
            and form.get("grant_type") == ["client_credentials"]
            and form.get("scope") == ["api.read"]
        )
        if not valid:
            self.send_json(401, {"error": "invalid_client"}, {"WWW-Authenticate": "Basic"})
            return
        self.send_json(
            200,
            {
                "access_token": OAUTH_ACCESS_TOKEN,
                "token_type": "Bearer",
                "expires_in": 300,
                "scope": "api.read",
            },
        )

    def send_multipart(self, body):
        content_type = self.headers.get("Content-Type", "")
        valid = (
            content_type.lower().startswith("multipart/form-data; boundary=")
            and b'name="title"' in body
            and b"hello" in body
            and b'name="upload"' in body
            and b"file-content" in body
        )
        self.send_json(
            200 if valid else 400,
            {"multipart": valid, "body_bytes": len(body)},
        )

    def send_echo(self, path, query, body):
        query_values = {
            name: values[-1] if len(values) == 1 else values
            for name, values in query.items()
        }
        self.send_json(
            200,
            {
                "path": path,
                "query": query_values,
                "request_token": self.headers.get("X-Request-Token"),
                "authorization_received": self.headers.get("Authorization") == AUTHORIZATION,
                "body": body.decode("utf-8", errors="replace"),
            },
        )

    def send_stream(self):
        chunks = [b"first", b"-second", b"-third"]
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(sum(len(chunk) for chunk in chunks)))
        self.send_header("Connection", "close")
        self.send_header("X-Ramag-Docker", "api-http")
        self.end_headers()
        for chunk in chunks:
            try:
                self.wfile.write(chunk)
                self.wfile.flush()
            except BrokenPipeError:
                return
            time.sleep(0.2)


if __name__ == "__main__":
    plain_server = ThreadingHTTPServer(("0.0.0.0", 8080), ApiHandler)
    plain_thread = threading.Thread(target=plain_server.serve_forever, daemon=True)
    plain_thread.start()

    tls_cert = os.environ.get("API_TLS_CERT", "/app/tls/server.cert.pem")
    tls_key = os.environ.get("API_TLS_KEY", "/app/tls/server.key.pem")
    tls_ca = os.environ.get("API_TLS_CA", "/app/tls/ca.cert.pem")
    if not all(os.path.isfile(path) for path in (tls_cert, tls_key, tls_ca)):
        plain_server.serve_forever()

    tls_server = ThreadingHTTPServer(("0.0.0.0", 8443), ApiHandler)
    tls_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls_context.verify_mode = ssl.CERT_REQUIRED
    tls_context.load_cert_chain(tls_cert, tls_key)
    tls_context.load_verify_locations(cafile=tls_ca)
    tls_server.socket = tls_context.wrap_socket(tls_server.socket, server_side=True)
    tls_server.serve_forever()
