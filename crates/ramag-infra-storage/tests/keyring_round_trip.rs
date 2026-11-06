//! 需要真实平台凭据服务的系统密钥往返验收。
//!
//! 该测试默认忽略，避免普通开发测试改动用户的 Keychain 或 Secret Service。
//! Linux 和 macOS CI 显式使用 `--ignored` 运行它。

#![cfg(any(target_os = "linux", target_os = "macos"))]

use ramag_infra_storage::keyring::{delete_master_key, get_or_create_master_key};

#[test]
#[ignore = "requires a real platform credential service"]
fn system_credential_store_round_trips_master_key() {
    delete_master_key().expect("清理旧的测试凭据失败");

    let created = get_or_create_master_key(true).expect("首次创建系统凭据失败");
    let loaded = get_or_create_master_key(false).expect("从系统凭据读取主密钥失败");
    assert_eq!(created, loaded, "系统凭据往返后主密钥发生变化");

    delete_master_key().expect("清理测试凭据失败");
    let missing =
        get_or_create_master_key(false).expect_err("删除系统凭据后不应静默重新创建主密钥");
    assert!(
        missing.to_string().contains("系统凭据库缺少主密钥"),
        "缺少主密钥时应返回明确错误：{missing}"
    );
}
