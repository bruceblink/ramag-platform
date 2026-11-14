use super::{KEY_LEN, credential_store_error, get_or_create_at_entry};
use keyring::Entry;
use rand::TryRngCore;
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const TEST_SERVICE: &str = "ramag-test";

/// 只持有本轮随机测试条目。清理目标不能由生产名称或外部参数指定。
struct IsolatedCredential {
    entry: Arc<Entry>,
    owned: bool,
}

impl IsolatedCredential {
    fn create() -> Result<Self, Box<dyn std::error::Error>> {
        let mut random = [0_u8; 16];
        rand::rngs::OsRng.try_fill_bytes(&mut random)?;
        let account = format!("master-key-test-{}", hex::encode(random));
        let entry = Entry::new(TEST_SERVICE, &account)?;
        match entry.get_password() {
            Err(keyring::Error::NoEntry) => Ok(Self {
                entry: Arc::new(entry),
                owned: false,
            }),
            Ok(_) => Err("测试凭据名称已存在，拒绝修改或清理".into()),
            Err(error) => Err(error.into()),
        }
    }

    fn initialize(&mut self) -> Result<[u8; KEY_LEN], Box<dyn std::error::Error>> {
        // 写入失败也可能已产生条目；从此处开始由本轮清理其独占名称。
        self.owned = true;
        Ok(get_or_create_at_entry(&self.entry, true)?)
    }

    fn delete(&mut self) -> TestResult {
        if self.owned {
            match self.entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => self.owned = false,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

impl Drop for IsolatedCredential {
    fn drop(&mut self) {
        if self.delete().is_err() {
            tracing::warn!(
                operation = "storage_keyring_test_cleanup",
                "isolated test credential cleanup failed"
            );
        }
    }
}

#[test]
fn credential_store_errors_explain_linux_secret_service_requirement() {
    let message = credential_store_error("读取系统凭据库失败", "service unavailable").to_string();
    #[cfg(target_os = "linux")]
    {
        assert!(message.contains("gnome-keyring"));
        assert!(message.contains("org.freedesktop.secrets"));
    }
    assert!(message.contains("service unavailable"));
}

#[test]
fn master_key_round_trip_uses_the_supplied_entry_only() -> TestResult {
    let entry = Entry::new_with_credential(Box::new(keyring::mock::MockCredential::default()));
    let sentinel = Entry::new_with_credential(Box::new(keyring::mock::MockCredential::default()));
    let sentinel_key = get_or_create_at_entry(&sentinel, true)?;
    let created = get_or_create_at_entry(&entry, true)?;
    assert!(created == get_or_create_at_entry(&entry, false)?);
    entry.delete_credential()?;
    assert!(get_or_create_at_entry(&entry, false).is_err());
    assert!(sentinel_key == get_or_create_at_entry(&sentinel, false)?);
    Ok(())
}

#[test]
fn missing_or_malformed_master_keys_are_not_replaced() -> TestResult {
    let entry = Entry::new_with_credential(Box::new(keyring::mock::MockCredential::default()));
    assert!(get_or_create_at_entry(&entry, false).is_err());
    assert!(matches!(entry.get_password(), Err(keyring::Error::NoEntry)));
    for malformed in ["not-hex", "00"] {
        entry.set_password(malformed)?;
        assert!(get_or_create_at_entry(&entry, true).is_err());
        assert_eq!(entry.get_password()?, malformed);
    }
    Ok(())
}

#[test]
fn isolated_cleanup_does_not_touch_another_entry() -> TestResult {
    assert_ne!(TEST_SERVICE, super::SERVICE);
    let sentinel = Entry::new_with_credential(Box::new(keyring::mock::MockCredential::default()));
    let sentinel_key = get_or_create_at_entry(&sentinel, true)?;
    {
        let mut target = IsolatedCredential {
            entry: Arc::new(Entry::new_with_credential(Box::new(
                keyring::mock::MockCredential::default(),
            ))),
            owned: false,
        };
        target.initialize()?;
        target.delete()?;
        assert!(matches!(
            target.entry.get_password(),
            Err(keyring::Error::NoEntry)
        ));
        target.delete()?;
    }
    assert!(sentinel_key == get_or_create_at_entry(&sentinel, false)?);
    Ok(())
}

#[test]
fn isolated_cleanup_runs_when_test_returns_an_error() -> TestResult {
    let entry = Arc::new(Entry::new_with_credential(Box::new(
        keyring::mock::MockCredential::default(),
    )));
    let outcome = (|| -> TestResult {
        let mut target = IsolatedCredential {
            entry: entry.clone(),
            owned: false,
        };
        target.initialize()?;
        Err("测试操作失败".into())
    })();
    assert!(outcome.is_err());
    assert!(matches!(entry.get_password(), Err(keyring::Error::NoEntry)));
    Ok(())
}

#[test]
#[ignore = "requires a real platform credential service; uses isolated test entries only"]
fn system_credential_store_round_trips_isolated_master_key() -> TestResult {
    let mut sentinel = IsolatedCredential::create()?;
    let sentinel_key = sentinel.initialize()?;
    let mut target = IsolatedCredential::create()?;
    let created = target.initialize()?;
    assert!(created == get_or_create_at_entry(&target.entry, false)?);
    assert!(sentinel_key == get_or_create_at_entry(&sentinel.entry, false)?);

    target.delete()?;
    let missing = get_or_create_at_entry(&target.entry, false)
        .err()
        .ok_or("删除测试凭据后不应静默重新创建主密钥")?;
    assert!(missing.to_string().contains("系统凭据库缺少主密钥"));
    assert!(sentinel_key == get_or_create_at_entry(&sentinel.entry, false)?);
    sentinel.delete()?;
    Ok(())
}
