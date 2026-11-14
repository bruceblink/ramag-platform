use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use super::*;
use crate::PluginPackageCatalogEntry;
use base64::Engine;
use ramag_domain::{PluginApiVersion, PluginCapability, PluginId, PluginPackageRuntime};
use ring::signature::{Ed25519KeyPair, KeyPair};
use tempfile::tempdir;

fn catalog(id: &str) -> PluginPackageCatalog {
    PluginPackageCatalog {
        format_version: crate::CURRENT_PLUGIN_CATALOG_FORMAT_VERSION,
        entries: vec![PluginPackageCatalogEntry {
            id: PluginId::new(id).expect("valid plugin ID"),
            name: "测试插件".into(),
            version: "1.0.0".into(),
            api_version: PluginApiVersion::new(1, 0),
            runtime: PluginPackageRuntime::Process,
            package_url: "https://plugins.example.test/package.tar.gz".into(),
            archive_sha256: "a".repeat(crate::PLUGIN_PACKAGE_SHA256_HEX_BYTES),
            capabilities: vec![PluginCapability::new("ui.entry")],
        }],
    }
}

fn signed_document() -> (
    PluginPackageCatalogDocument,
    PluginPackageCatalogTrustPolicy,
) {
    let catalog = catalog("signed.fetcher");
    let seed = [9_u8; 32];
    let pair = Ed25519KeyPair::from_seed_unchecked(&seed).expect("test key pair");
    let policy =
        PluginPackageCatalogTrustPolicy::new([crate::PluginPackageCatalogTrustedKey::new(
            "fetcher-test-key",
            pair.public_key().as_ref(),
            "https://plugins.example.test/",
        )
        .expect("trusted key")])
        .expect("trust policy");
    let message = crate::canonical_plugin_catalog_signature_message(&catalog)
        .expect("canonical signature message");
    let envelope = crate::PluginPackageCatalogSignedEnvelope {
        format_version: crate::CURRENT_PLUGIN_CATALOG_SIGNED_FORMAT_VERSION,
        key_id: "fetcher-test-key".into(),
        payload: catalog,
        signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(pair.sign(&message).as_ref()),
    };
    let bytes = serde_json::to_vec(&envelope).expect("signed envelope JSON");
    let document = PluginPackageCatalogDocument::parse_json(
        &bytes,
        "https://plugins.example.test/catalog.json",
        &policy,
    )
    .expect("signed document");
    (document, policy)
}

#[test]
fn source_url_policy_requires_https_without_userinfo_query_or_fragment() {
    assert!(validate_source_url("https://plugins.example.test/catalog.json").is_ok());
    for value in [
        "http://plugins.example.test/catalog.json",
        "https://user:pass@plugins.example.test/catalog.json",
        "https://plugins.example.test/catalog.json?channel=stable",
        "https://plugins.example.test/catalog.json#latest",
    ] {
        assert!(matches!(
            validate_source_url(value),
            Err(PluginPackageCatalogFetchError::InvalidSourceUrl { .. })
        ));
    }
    let oversized = format!(
        "https://plugins.example.test/{}",
        "a".repeat(crate::MAX_PLUGIN_CATALOG_URL_BYTES)
    );
    assert!(matches!(
        validate_source_url(&oversized),
        Err(PluginPackageCatalogFetchError::InvalidSourceUrl { .. })
    ));
}

#[test]
fn content_length_is_bounded_before_body_read() {
    assert!(validate_content_length(Some(MAX_PLUGIN_CATALOG_MANIFEST_BYTES as u64)).is_ok());
    assert!(matches!(
        validate_content_length(Some(MAX_PLUGIN_CATALOG_MANIFEST_BYTES as u64 + 1)),
        Err(PluginPackageCatalogFetchError::TooLarge { .. })
    ));
    assert!(validate_content_length(None).is_ok());
}

#[test]
fn response_status_and_redirect_are_rejected_without_body_details() {
    let expected = Url::parse("http://no.url.provided.local").expect("test URL");
    let response = Response::from(
        http::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::<u8>::new())
            .expect("HTTP response"),
    );
    assert!(matches!(
        validate_response(&response, &expected),
        Err(PluginPackageCatalogFetchError::HttpStatus { status: 404 })
    ));

    let response = Response::from(
        http::Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, "https://plugins.example.test/new.json")
            .body(Vec::<u8>::new())
            .expect("redirect response"),
    );
    assert!(matches!(
        validate_response(&response, &expected),
        Err(PluginPackageCatalogFetchError::Redirect {
            location: Some(_),
            ..
        })
    ));
}

#[test]
fn stream_body_limit_is_enforced_without_content_length() {
    let body = vec![0_u8; MAX_PLUGIN_CATALOG_MANIFEST_BYTES + 1];
    let response = Response::from(http::Response::builder().body(body).expect("body response"));
    let result = futures::executor::block_on(read_bounded_body(response));
    assert!(matches!(
        result,
        Err(PluginPackageCatalogFetchError::TooLarge { .. })
    ));
}

#[test]
fn managed_runtime_propagates_validation_errors_without_panicking() {
    // GPUI 后台执行器没有 Tokio reactor；即使请求在 URL 校验阶段失败，也必须经过受控 runtime。
    let _ = rustls::crypto::ring::default_provider().install_default();
    let root = tempdir().expect("temporary catalog root");
    let store = PluginPackageCatalogStore::new(root.path()).expect("absolute root");
    let fetcher = PluginPackageCatalogFetcher::new().expect("create fetcher");
    let result = fetcher
        .fetch_and_store_on_managed_runtime("http://plugins.example.test/catalog.json", &store);
    assert!(matches!(
        result,
        Err(PluginPackageCatalogFetchError::InvalidSourceUrl { .. })
    ));
}

#[test]
fn managed_runtime_keeps_reqwest_request_inside_tokio_context() {
    // 连接本机未使用端口，验证真实 reqwest 请求不会在 GPUI 风格调用线程触发 reactor panic。
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_millis(100))
        .timeout(Duration::from_millis(500))
        .build()
        .expect("create test client");
    let root = tempdir().expect("temporary catalog root");
    let store = PluginPackageCatalogStore::new(root.path()).expect("absolute root");
    let fetcher = PluginPackageCatalogFetcher::with_client(client);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fetcher.fetch_and_store_on_managed_runtime("https://127.0.0.1:1/catalog.json", &store)
    }));
    assert!(result.is_ok(), "managed runtime request must not panic");
    assert!(result.expect("no panic").is_err());
}

#[test]
#[ignore = "requires the local Docker HTTPS plugin catalog fixture"]
fn fetches_catalog_over_local_https_fixture() {
    // 本测试只信任自签 fixture 证书；生产构造器仍使用系统严格证书校验。
    let _ = rustls::crypto::ring::default_provider().install_default();
    let root = PathBuf::from(
        std::env::var_os("RAMAG_PLUGIN_CATALOG_TEST_ROOT")
            .expect("test script must set fixture root"),
    );
    let port: u16 = std::env::var("RAMAG_PLUGIN_CATALOG_TEST_PORT")
        .expect("test script must set HTTPS port")
        .parse()
        .expect("HTTPS port must be a number");
    let client = Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(5))
        .resolve(
            "github.com",
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
        )
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .redirect(redirect::Policy::none())
        .build()
        .expect("build test-only HTTPS client");
    let fetcher = PluginPackageCatalogFetcher::with_client(client);
    let store = PluginPackageCatalogStore::new(root.join("state"))
        .expect("fixture state root must be absolute");
    let catalog_url = "https://github.com/plugins/catalog.json";

    let catalog = fetcher
        .fetch_and_store_on_managed_runtime(catalog_url, &store)
        .expect("fetch valid catalog over local HTTPS");
    assert_eq!(catalog.entries().len(), 1);
    assert_eq!(store.load().expect("load stored catalog"), Some(catalog));
    let stored_bytes = fs::read(store.path()).expect("read stored catalog");

    let not_found = fetcher
        .fetch_and_store_on_managed_runtime("https://github.com/plugins/not-found.json", &store);
    assert!(matches!(
        not_found,
        Err(PluginPackageCatalogFetchError::HttpStatus { status: 404 })
    ));
    assert_eq!(
        fs::read(store.path()).expect("read catalog after 404"),
        stored_bytes
    );

    let redirect_result = fetcher
        .fetch_and_store_on_managed_runtime("https://github.com/plugins/redirect.json", &store);
    assert!(matches!(
        redirect_result,
        Err(PluginPackageCatalogFetchError::Redirect { .. })
    ));
    assert_eq!(
        fs::read(store.path()).expect("read catalog after redirect"),
        stored_bytes
    );

    let oversized = fetcher
        .fetch_and_store_on_managed_runtime("https://github.com/plugins/oversized.json", &store);
    assert!(matches!(
        oversized,
        Err(PluginPackageCatalogFetchError::TooLarge { .. })
    ));
    assert_eq!(
        fs::read(store.path()).expect("read catalog after oversized response"),
        stored_bytes
    );
}

#[test]
fn store_round_trips_catalog_and_replaces_atomically() {
    let root = tempdir().expect("temporary catalog root");
    let store = PluginPackageCatalogStore::new(root.path()).expect("absolute root");
    let first = catalog("remote.first");
    store.save(&first).expect("save first catalog");
    assert_eq!(store.load().expect("load first catalog"), Some(first));

    let second = catalog("remote.second");
    store.save(&second).expect("replace catalog");
    assert_eq!(store.load().expect("load second catalog"), Some(second));
    assert!(
        !root
            .path()
            .read_dir()
            .expect("read catalog root")
            .any(|entry| entry
                .expect("catalog entry")
                .file_name()
                .to_string_lossy()
                .starts_with(CATALOG_TEMP_FILE_PREFIX))
    );
}

#[test]
fn store_round_trips_signed_document_and_rechecks_source_binding() {
    let root = tempdir().expect("temporary catalog root");
    let store = PluginPackageCatalogStore::new(root.path()).expect("absolute root");
    let (document, policy) = signed_document();
    let serialized = document.serialized_json().to_vec();
    store
        .save_document(&document)
        .expect("save signed document");
    assert_eq!(
        fs::read(store.path()).expect("read signed catalog"),
        serialized
    );
    let loaded = store
        .load_document("https://plugins.example.test/catalog.json", &policy)
        .expect("load signed document")
        .expect("signed document exists");
    assert!(loaded.trust().is_verified());
    assert_eq!(loaded.serialized_json(), serialized.as_slice());
    assert!(matches!(
        store.load_document("https://other.example.test/catalog.json", &policy),
        Err(PluginPackageCatalogStoreError::Signature(
            PluginPackageCatalogSignatureError::UntrustedSource
        ))
    ));
}

#[test]
fn store_load_document_accepts_legacy_catalog_as_unverified() {
    let root = tempdir().expect("temporary catalog root");
    let store = PluginPackageCatalogStore::new(root.path()).expect("absolute root");
    let legacy = catalog("legacy.fetcher");
    store.save(&legacy).expect("save legacy catalog");
    let document = store
        .load_document(
            "https://plugins.example.test/catalog.json",
            &PluginPackageCatalogTrustPolicy::default(),
        )
        .expect("load legacy document")
        .expect("legacy document exists");
    assert!(!document.trust().is_verified());
    assert_eq!(document.catalog, legacy);
}

#[test]
fn store_rejects_relative_root_and_symlink_root() {
    assert!(matches!(
        PluginPackageCatalogStore::new("relative/catalog"),
        Err(PluginPackageCatalogStoreError::RelativeRoot { .. })
    ));

    #[cfg(unix)]
    {
        let parent = tempdir().expect("temporary parent");
        let real = parent.path().join("real");
        fs::create_dir(&real).expect("create real root");
        let link = parent.path().join("link");
        std::os::unix::fs::symlink(&real, &link).expect("create root symlink");
        let store = PluginPackageCatalogStore::new(&link).expect("absolute symlink root");
        assert!(matches!(
            store.save(&catalog("remote.symlink")),
            Err(PluginPackageCatalogStoreError::InvalidRoot { .. })
        ));
    }
}
