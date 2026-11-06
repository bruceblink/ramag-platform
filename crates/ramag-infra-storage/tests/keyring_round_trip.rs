//! 需要真实平台凭据服务的系统密钥往返验收。
//!
//! 该测试默认忽略，避免普通开发测试改动用户的 Keychain 或 Secret Service。
//! Linux 和 macOS CI 显式使用 `--ignored` 运行它。

#![cfg(any(target_os = "linux", target_os = "macos"))]

use ramag_infra_storage::keyring::{delete_master_key, get_or_create_master_key};

#[test]
#[ignore = "requires a real platform credential service"]
fn system_credential_store_round_trips_master_key() -> Result<(), Box<dyn std::error::Error>> {
    let created = get_or_create_master_key(true)?;
    let loaded = get_or_create_master_key(false)?;
    assert_eq!(created, loaded, "系统凭据往返后主密钥发生变化");

    delete_master_key()?;
    let missing = match get_or_create_master_key(false) {
        Ok(_) => return Err("删除系统凭据后不应静默重新创建主密钥".into()),
        Err(error) => error,
    };
    assert!(
        missing.to_string().contains("系统凭据库缺少主密钥"),
        "缺少主密钥时应返回明确错误：{missing}"
    );

    Ok(())
}
