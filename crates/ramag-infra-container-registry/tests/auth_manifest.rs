#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use ramag_domain::{
    ContainerErrorCategory, ContainerRegistryCredential, ContainerRegistryDriver,
    ContainerRegistryProfile, DomainError,
};
use ramag_infra_container_registry::RegistryHttpDriver;

#[test]
fn maps_authentication_failure_without_leaking_registry_response_or_password() {
    let (endpoint, requests, server) = spawn_http_fixture(
        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=registry\r\nContent-Type: application/json\r\nContent-Length: 62\r\nConnection: close\r\n\r\n{\"errors\":[{\"message\":\"super-secret\"}]}".into(),
    );
    let profile = ContainerRegistryProfile::new("本机 Registry", endpoint).with_insecure_http(true);
    let credential = ContainerRegistryCredential {
        username: "alice".into(),
        password: "super-secret".into(),
    };
    let driver = RegistryHttpDriver::new().expect("Registry HTTP 客户端应创建成功");
    let result = smol::block_on(driver.list_repositories(&profile, Some(&credential)));
    server.join().expect("Registry fixture 应正常退出");

    let error = result.expect_err("401 应返回认证错误");
    match error {
        DomainError::Container(error) => {
            assert_eq!(error.category, ContainerErrorCategory::Authentication);
            assert!(!error.retryable);
            assert!(!error.to_string().contains("super-secret"));
        }
        other => panic!("应返回容器认证错误，实际为 {other:?}"),
    }
    let requests = requests.lock().expect("请求记录不应被锁定");
    assert!(
        requests[0]
            .to_ascii_lowercase()
            .contains("authorization: basic ")
    );
}

#[test]
fn reads_and_validates_manifest_digest_from_head_response() {
    let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let response = format!(
        "HTTP/1.1 200 OK\r\nDocker-Content-Digest: {digest}\r\nContent-Type: application/vnd.oci.image.manifest.v1+json\r\nContent-Length: 256\r\nConnection: close\r\n\r\n"
    );
    let (endpoint, requests, server) = spawn_http_fixture(response);
    let profile = ContainerRegistryProfile::new("本机 Registry", endpoint).with_insecure_http(true);
    let driver = RegistryHttpDriver::new().expect("Registry HTTP 客户端应创建成功");
    let manifest = smol::block_on(driver.get_manifest(&profile, None, "library/app", "stable"))
        .expect("Registry 镜像清单应成功读取");
    server.join().expect("Registry fixture 应正常退出");

    assert_eq!(manifest.repository, "library/app");
    assert_eq!(manifest.reference, "stable");
    assert_eq!(manifest.digest, digest);
    assert_eq!(
        manifest.media_type.as_deref(),
        Some("application/vnd.oci.image.manifest.v1+json")
    );
    assert_eq!(manifest.size_bytes, Some(256));
    let requests = requests.lock().expect("请求记录不应被锁定");
    assert!(requests[0].starts_with("HEAD /v2/library/app/manifests/stable HTTP/1.1"));
}

fn spawn_http_fixture(response: String) -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("应分配 Registry fixture 端口");
    let endpoint = format!(
        "http://{}",
        listener
            .local_addr()
            .expect("Registry fixture 地址应可读取")
    );
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed = requests.clone();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("应接收 Registry 请求");
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream.read(&mut chunk).expect("应读取 Registry 请求");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        observed
            .lock()
            .expect("请求记录应可写入")
            .push(String::from_utf8_lossy(&buffer).into_owned());
        stream
            .write_all(response.as_bytes())
            .expect("应写入 Registry 响应");
    });
    (endpoint, requests, server)
}
