use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use ramag_domain::entities::{
    ApiAuth, ApiBody, ApiCancellation, ApiKeyLocation, ApiMultipartPart, ApiParameter, ApiProtocol,
    ApiRequestSpec, ApiResponseStatus, ApiTlsVerify, HttpRequestSpec, MAX_API_MULTIPART_FILE_BYTES,
    MAX_API_RESPONSE_BODY_BYTES,
};
use ramag_domain::error::DomainError;
use ramag_domain::traits::ApiDriver;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::HttpApiDriver;

struct TestServer {
    address: SocketAddr,
    request: Option<oneshot::Receiver<Vec<u8>>>,
    task: JoinHandle<io::Result<()>>,
}

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

impl TestServer {
    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }

    async fn wait_for_request(&mut self) -> Vec<u8> {
        self.request
            .take()
            .expect("test server request receiver is available")
            .await
            .expect("test server received a request")
    }

    async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

async fn spawn_server(response: Vec<u8>, delay: Option<Duration>) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local HTTP test server");
    let address = listener.local_addr().expect("read local HTTP address");
    let (request_tx, request_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let request = read_request(&mut stream).await?;
        let _ = request_tx.send(request);
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        stream.write_all(&response).await?;
        stream.shutdown().await
    });
    TestServer {
        address,
        request: Some(request_rx),
        task,
    }
}

async fn spawn_streaming_server() -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind streaming HTTP test server");
    let address = listener.local_addr().expect("read streaming HTTP address");
    let (request_tx, request_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let request = read_request(&mut stream).await?;
        let _ = request_tx.send(request);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nfirst")
            .await?;
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok(())
    });
    TestServer {
        address,
        request: Some(request_rx),
        task,
    }
}

fn response(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    )
    .into_bytes();
    for (name, value) in headers {
        bytes.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    bytes.extend_from_slice(b"\r\n");
    bytes.extend_from_slice(body);
    bytes
}

async fn read_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
    let mut request = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            return Ok(request);
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > MAX_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "test request is too large",
            ));
        }
        let Some(header_end) = find_bytes(&request, b"\r\n\r\n") else {
            continue;
        };
        let body_length = content_length(&request[..header_end]);
        let expected_length = header_end + 4 + body_length;
        if request.len() >= expected_length {
            request.truncate(expected_length);
            return Ok(request);
        }
    }
}

fn content_length(headers: &[u8]) -> usize {
    String::from_utf8_lossy(headers)
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn cancellation() -> ApiCancellation {
    Arc::new(AtomicBool::new(false))
}

fn request(url: String) -> ApiRequestSpec {
    ApiRequestSpec::Http(HttpRequestSpec::new("GET", url))
}

#[tokio::test]
async fn preserves_non_success_status_headers_and_body() {
    let body = br#"{"error":"teapot"}"#;
    let server = spawn_server(
        response("418 I'm a teapot", &[("X-Trace", "trace-42")], body),
        None,
    )
    .await;
    let driver = HttpApiDriver::new().expect("HTTP driver initializes");
    let snapshot = driver
        .execute(
            &request(server.url("/status")),
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .expect("HTTP response is returned");

    assert_eq!(snapshot.protocol, ApiProtocol::Http);
    assert_eq!(snapshot.status, ApiResponseStatus::Http { code: 418 });
    assert_eq!(snapshot.body, body);
    assert!(snapshot
        .headers
        .iter()
        .any(|header| header.name.eq_ignore_ascii_case("x-trace") && header.value == "trace-42"));
    assert_eq!(snapshot.size_bytes, body.len() as u64);
    assert!(!snapshot.truncated);
    server.stop().await;
}

#[tokio::test]
async fn expands_variables_query_headers_auth_and_body() {
    let mut server = spawn_server(response("200 OK", &[], b"ok"), None).await;
    let mut variables = BTreeMap::new();
    variables.insert("path".into(), "orders".into());
    variables.insert("query".into(), "hello world".into());
    variables.insert("header".into(), "header-secret".into());
    variables.insert("token".into(), "bearer-secret".into());
    variables.insert("body".into(), "body-secret".into());

    let mut spec = HttpRequestSpec::new("POST", format!("{}/api/{{{{path}}}}", server.url("")));
    spec.query.push(ApiParameter::new("q", "{{query}}", false));
    spec.headers
        .push(ApiParameter::new("X-Request-Token", "{{header}}", false));
    spec.auth = ApiAuth::Bearer {
        token: "{{token}}".into(),
    };
    spec.body = Some(ApiBody::text("body={{body}}", Some("text/plain".into())));
    let driver = HttpApiDriver::new().expect("HTTP driver initializes");
    driver
        .execute(&ApiRequestSpec::Http(spec), &variables, cancellation())
        .await
        .expect("variable-expanded request succeeds");

    let request = String::from_utf8(server.wait_for_request().await).expect("request is UTF-8");
    let request_lower = request.to_ascii_lowercase();
    assert!(request_lower.contains("/api/orders?q=hello+world http/1.1"));
    assert!(request_lower.contains("x-request-token: header-secret"));
    assert!(request_lower.contains("authorization: bearer bearer-secret"));
    assert!(request.ends_with("body=body-secret"));
    assert!(request_lower.contains("content-type: text/plain"));
    server.stop().await;
}

#[tokio::test]
async fn supports_basic_and_api_key_auth_locations() {
    execute_auth_case(
        ApiAuth::Basic {
            username: "user".into(),
            password: "pass".into(),
        },
        "authorization: basic dxnlcjpwyxnz",
    )
    .await;
    execute_auth_case(
        ApiAuth::ApiKey {
            name: "X-Api-Key".into(),
            value: "header-key".into(),
            location: ApiKeyLocation::Header,
        },
        "x-api-key: header-key",
    )
    .await;
    execute_auth_case(
        ApiAuth::ApiKey {
            name: "api_key".into(),
            value: "query-key".into(),
            location: ApiKeyLocation::Query,
        },
        "api_key=query-key http/1.1",
    )
    .await;
}

async fn execute_auth_case(auth: ApiAuth, expected: &str) {
    let mut server = spawn_server(response("200 OK", &[], b"ok"), None).await;
    let mut spec = HttpRequestSpec::new("GET", server.url("/auth"));
    spec.auth = auth;
    let driver = HttpApiDriver::new().expect("HTTP driver initializes");
    driver
        .execute(
            &ApiRequestSpec::Http(spec),
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .expect("authenticated request succeeds");
    let request = String::from_utf8(server.wait_for_request().await).expect("request is UTF-8");
    assert!(request.to_ascii_lowercase().contains(expected));
    server.stop().await;
}

#[tokio::test]
async fn caps_response_body_and_records_original_size() {
    let body = vec![b'x'; MAX_API_RESPONSE_BODY_BYTES + 1024];
    let server = spawn_server(response("200 OK", &[], &body), None).await;
    let driver = HttpApiDriver::new().expect("HTTP driver initializes");
    let snapshot = driver
        .execute(
            &request(server.url("/large")),
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .expect("large HTTP response is bounded");

    assert_eq!(snapshot.body.len(), MAX_API_RESPONSE_BODY_BYTES);
    assert_eq!(snapshot.size_bytes, body.len() as u64);
    assert!(snapshot.truncated);
    server.stop().await;
}

#[tokio::test]
async fn maps_timeout_and_cancellation_without_leaking_transport_errors() {
    let timeout_server = spawn_server(
        response("200 OK", &[], b"late"),
        Some(Duration::from_millis(250)),
    )
    .await;
    let mut timeout_spec = HttpRequestSpec::new("GET", timeout_server.url("/timeout"));
    timeout_spec.timeout_millis = 50;
    let timeout_error = HttpApiDriver::new()
        .expect("HTTP driver initializes")
        .execute(
            &ApiRequestSpec::Http(timeout_spec),
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .expect_err("request must time out");
    assert!(matches!(timeout_error, DomainError::ConnectionFailed(_)));
    timeout_server.stop().await;

    let mut cancellation_server = spawn_streaming_server().await;
    let cancelled = cancellation();
    let mut cancellation_spec = HttpRequestSpec::new("GET", cancellation_server.url("/cancel"));
    cancellation_spec.timeout_millis = 5_000;
    let driver = HttpApiDriver::new().expect("HTTP driver initializes");
    let task = tokio::spawn({
        let cancelled = cancelled.clone();
        async move {
            driver
                .execute(
                    &ApiRequestSpec::Http(cancellation_spec),
                    &BTreeMap::new(),
                    cancelled,
                )
                .await
        }
    });
    let _ = tokio::time::timeout(
        Duration::from_secs(1),
        cancellation_server.wait_for_request(),
    )
    .await
    .expect("streaming server received request");
    tokio::time::sleep(Duration::from_millis(50)).await;
    cancelled.store(true, Ordering::Relaxed);
    let cancellation_error = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("cancelled request returns promptly")
        .expect("cancelled request task joins")
        .expect_err("cancelled request returns an error");
    assert!(matches!(cancellation_error, DomainError::Cancelled(_)));
    cancellation_server.stop().await;
}

#[tokio::test]
async fn rejects_unreadable_tls_configuration_before_connecting() {
    let mut spec = HttpRequestSpec::new("GET", "https://127.0.0.1:1/tls");
    spec.tls.verify = ApiTlsVerify::Ca;
    spec.tls.ca_cert_path = Some(format!(
        "ramag-api-http-test-missing-{}.pem",
        std::process::id()
    ));
    let error = HttpApiDriver::new()
        .expect("HTTP driver initializes")
        .execute(
            &ApiRequestSpec::Http(spec),
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .expect_err("missing CA must fail before network access");
    assert!(matches!(error, DomainError::InvalidConfig(message) if message.contains("CA")));
}

#[tokio::test]
async fn sends_multipart_text_and_file_parts_with_generated_boundary() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!(
        "ramag-api-multipart-{}-{}.txt",
        std::process::id(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, b"file-content").map_err(|error| error.to_string())?;
    let mut server = spawn_server(response("200 OK", &[], b"ok"), None).await;
    let mut spec = HttpRequestSpec::new("POST", server.url("/multipart"));
    spec.body = Some(ApiBody::multipart(vec![
        ApiMultipartPart::text("title", "hello", false),
        ApiMultipartPart::file(
            "upload",
            path.to_string_lossy().into_owned(),
            None,
            Some("text/plain".into()),
        ),
    ]));

    let result = HttpApiDriver::new()
        .map_err(|error| error.to_string())?
        .execute(
            &ApiRequestSpec::Http(spec),
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .map_err(|error| error.to_string());
    let request = server.wait_for_request().await;
    server.stop().await;
    let _ = std::fs::remove_file(&path);
    result?;

    let header_end = find_bytes(&request, b"\r\n\r\n").ok_or("Multipart 请求缺少 Header 结束符")?;
    let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
    let body = String::from_utf8_lossy(&request[header_end + 4..]);
    let body_lower = body.to_ascii_lowercase();
    assert!(headers.contains("content-type: multipart/form-data; boundary="));
    assert!(body.contains("name=\"title\""));
    assert!(body.contains("hello"));
    assert!(body.contains("name=\"upload\""));
    assert!(body.contains("filename=\"ramag-api-multipart-"));
    assert!(body_lower.contains("content-type: text/plain"));
    assert!(body.contains("file-content"));
    Ok(())
}

#[tokio::test]
async fn rejects_oversized_multipart_file_before_connecting() -> Result<(), String> {
    let path = std::env::temp_dir().join(format!(
        "ramag-api-multipart-large-{}-{}",
        std::process::id(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|error| error.to_string())?;
    file.set_len(MAX_API_MULTIPART_FILE_BYTES as u64 + 1)
        .map_err(|error| error.to_string())?;
    let mut spec = HttpRequestSpec::new("POST", "http://127.0.0.1:1/multipart");
    spec.body = Some(ApiBody::multipart(vec![ApiMultipartPart::file(
        "upload",
        path.to_string_lossy().into_owned(),
        Some("large.bin".into()),
        None,
    )]));

    let error = HttpApiDriver::new()
        .map_err(|error| error.to_string())?
        .execute(
            &ApiRequestSpec::Http(spec),
            &BTreeMap::new(),
            cancellation(),
        )
        .await
        .expect_err("超大 Multipart 文件应在连接前失败");
    let _ = std::fs::remove_file(&path);
    assert!(matches!(error, DomainError::InvalidConfig(message) if message.contains("单个文件")));
    Ok(())
}
