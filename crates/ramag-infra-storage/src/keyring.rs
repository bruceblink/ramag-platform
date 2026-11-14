//! 系统凭据库：macOS 使用 Keychain，Windows 使用 Credential Manager，Linux 使用 Secret Service。
//! 32 字节主密钥仅存系统凭据库，不落盘；redb 文件被拷走也无法解密。

use ramag_domain::error::{DomainError, Result};
use rand::TryRngCore;
use tracing::info;

const SERVICE: &str = "ramag";
const ACCOUNT: &str = "master-key";
const KEY_LEN: usize = 32;

/// 首次随机生成并写入系统凭据库；已有数据库时禁止静默重建丢失的密钥。
pub fn get_or_create_master_key(allow_create: bool) -> Result<[u8; KEY_LEN]> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| credential_store_error("初始化系统凭据库失败", e))?;
    get_or_create_at_entry(&entry, allow_create)
}

// 生产入口固定使用原凭据名称；平台测试只向此核心传入独立的测试条目。
fn get_or_create_at_entry(entry: &keyring::Entry, allow_create: bool) -> Result<[u8; KEY_LEN]> {
    match entry.get_password() {
        Ok(hex_str) => {
            let bytes = hex::decode(hex_str.trim())
                .map_err(|e| DomainError::Storage(format!("系统凭据库主密钥格式错误：{e}")))?;
            if bytes.len() != KEY_LEN {
                // 不能静默重建：旧 redb 仍由原密钥加密，覆盖后会永久失去解密能力。
                return Err(DomainError::Storage(format!(
                    "系统凭据库主密钥长度错误：应为 {KEY_LEN} 字节，实际为 {} 字节",
                    bytes.len()
                )));
            }
            let mut key = [0u8; KEY_LEN];
            key.copy_from_slice(&bytes);
            Ok(key)
        }
        Err(keyring::Error::NoEntry) if allow_create => {
        info!(operation = "storage_keyring_init", stage = "generate", "master key not found, generating new one");
            generate_and_save(entry)
        }
        Err(keyring::Error::NoEntry) => Err(DomainError::Storage(
            "检测到已有加密数据库，但系统凭据库缺少主密钥；为避免覆盖恢复线索，已停止启动。请先恢复系统凭据，或备份并移走旧数据库后重试".into(),
        )),
        Err(e) => Err(credential_store_error("读取系统凭据库失败", e)),
    }
}

fn credential_store_error(action: &str, error: impl std::fmt::Display) -> DomainError {
    let message = format!("{action}：{error}");
    #[cfg(target_os = "linux")]
    let message = format!(
        "{message}。Linux 需要可用的 Secret Service；Ubuntu/WSL 可运行 `sudo apt install gnome-keyring`。WSL 没有桌面登录自动创建 D-Bus 会话时，请使用 `dbus-run-session -- <启动命令>` 启动应用，并确认会话能启动 `org.freedesktop.secrets`。"
    );
    DomainError::Storage(message)
}

fn generate_and_save(entry: &keyring::Entry) -> Result<[u8; KEY_LEN]> {
    let mut key = [0u8; KEY_LEN];
    rand::rngs::OsRng
        .try_fill_bytes(&mut key)
        .map_err(|e| DomainError::Storage(format!("OS 随机源不可用：{e}")))?;
    entry
        .set_password(&hex::encode(key))
        .map_err(|e| DomainError::Storage(format!("写入系统凭据库失败：{e}")))?;
    info!(
        operation = "storage_keyring_init",
        stage = "stored",
        "master key created and stored in credential store"
    );
    Ok(key)
}

#[cfg(test)]
#[path = "keyring_tests.rs"]
mod tests;
