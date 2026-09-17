#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use ramag_domain::{
    ContainerErrorCategory, ContainerRegistryDriver, ContainerRegistryProfile, DomainError,
};
use ramag_infra_container_registry::RegistryHttpDriver;

#[test]
fn follows_catalog_pagination_within_the_registry_origin() {
    let (endpoint, requests, server) = spawn_http_fixture(vec![
        json_response(
            r#"{"repositories":["library/app","library/api"]}"#,
            Some(r#"</v2/_catalog?n=1000&last=library%2Fapi>; rel="next""#),
        ),
        json_response(r#"{"repositories":["library/web"]}"#, None),
    ]);
    let profile = ContainerRegistryProfile::new("本机 Registry", endpoint).with_insecure_http(true);
    let driver = RegistryHttpDriver::new().expect("Registry HTTP 客户端应创建成功");
    let repositories =
        smol::block_on(driver.list_repositories(&profile, None)).expect("分页仓库列表应成功读取");
    server.join().expect("Registry fixture 应正常退出");

    assert_eq!(
        repositories
            .into_iter()
            .map(|repository| repository.name)
            .collect::<Vec<_>>(),
        ["library/app", "library/api", "library/web"]
    );
    let requests = requests.lock().expect("请求记录不应被锁定");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], "GET /v2/_catalog?n=1000 HTTP/1.1");
    assert_eq!(
        requests[1],
        "GET /v2/_catalog?n=1000&last=library%2Fapi HTTP/1.1"
    );
}

#[test]
fn follows_tag_pagination_and_keeps_repository_identity() {
    let (endpoint, requests, server) = spawn_http_fixture(vec![
        json_response(
            r#"{"name":"library/app","tags":["stable"]}"#,
            Some(r#"</v2/library/app/tags/list?n=1000&last=stable>; rel="next""#),
        ),
        json_response(r#"{"tags":["release"]}"#, None),
    ]);
    let profile = ContainerRegistryProfile::new("本机 Registry", endpoint).with_insecure_http(true);
    let driver = RegistryHttpDriver::new().expect("Registry HTTP 客户端应创建成功");
    let tags = smol::block_on(driver.list_tags(&profile, None, "library/app"))
        .expect("分页 Tag 列表应成功读取");
    server.join().expect("Registry fixture 应正常退出");

    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].repository, "library/app");
    assert_eq!(tags[1].repository, "library/app");
    assert_eq!(tags[1].name, "release");
    let requests = requests.lock().expect("请求记录不应被锁定");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], "GET /v2/library/app/tags/list?n=1000 HTTP/1.1");
}

#[test]
fn rejects_cross_origin_registry_pagination_links() {
    let (endpoint, _requests, server) = spawn_http_fixture(vec![json_response(
        r#"{"repositories":["library/app"]}"#,
        Some(r#"<https://registry-attacker.example/v2/_catalog?n=1000>; rel="next""#),
    )]);
    let profile = ContainerRegistryProfile::new("本机 Registry", endpoint).with_insecure_http(true);
    let driver = RegistryHttpDriver::new().expect("Registry HTTP 客户端应创建成功");
    let result = smol::block_on(driver.list_repositories(&profile, None));
    server.join().expect("Registry fixture 应正常退出");

    assert!(matches!(
        result,
        Err(DomainError::Container(error))
            if error.category == ContainerErrorCategory::Protocol
    ));
}

fn json_response(body: &str, link: Option<&str>) -> String {
    let link_header = link.map_or_else(String::new, |link| format!("Link: {link}\r\n"));
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
        body.len(),
        link_header,
        body
    )
}

fn spawn_http_fixture(responses: Vec<String>) -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<()>) {
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
        for response in responses {
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
            let request_line = String::from_utf8_lossy(&buffer)
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned();
            observed
                .lock()
                .expect("请求记录应可写入")
                .push(request_line);
            stream
                .write_all(response.as_bytes())
                .expect("应写入 Registry 响应");
        }
    });
    (endpoint, requests, server)
}
