//! MySQL 触发器元数据的 Docker 集成测试。

use ramag_domain::entities::{ConnectionConfig, Query};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::Driver;
use ramag_infra_mysql::MysqlDriver;

/// 从本机 Docker MySQL 测试环境读取连接配置；环境变量不完整时跳过测试。
fn config_from_env() -> Option<ConnectionConfig> {
    let host = std::env::var("RAMAG_TEST_MYSQL_HOST").ok()?;
    let port: u16 = std::env::var("RAMAG_TEST_MYSQL_PORT").ok()?.parse().ok()?;
    let user = std::env::var("RAMAG_TEST_MYSQL_USER").ok()?;
    let password = std::env::var("RAMAG_TEST_MYSQL_PASSWORD").ok()?;
    let database = std::env::var("RAMAG_TEST_MYSQL_DB").ok();

    Some(ConnectionConfig {
        password,
        database,
        ..ConnectionConfig::new_mysql("integration-test", host, port, user)
    })
}

/// 创建临时表和触发器，再确认表属性所需的名称、时机、事件和定义均可读取。
#[tokio::test(flavor = "multi_thread")]
async fn list_triggers_for_temporary_table() -> Result<()> {
    let Some(config) = config_from_env() else {
        eprintln!("[SKIP] integration test skipped: 设置 RAMAG_TEST_MYSQL_* 环境变量后运行");
        return Ok(());
    };
    let driver = MysqlDriver::new();
    let schema = config
        .database
        .clone()
        .unwrap_or_else(|| "midas_storage".into());
    let suffix = std::process::id();
    let table = format!("ramag_trigger_probe_{suffix}");
    let audit_table = format!("ramag_trigger_audit_{suffix}");
    let trigger_name = format!("ramag_trigger_probe_after_insert_{suffix}");

    driver
        .execute(
            &config,
            &Query::new(format!(
                "DROP TABLE IF EXISTS `{table}`; DROP TABLE IF EXISTS `{audit_table}`; \
                 CREATE TABLE `{audit_table}` (subject_id BIGINT NOT NULL); \
                 CREATE TABLE `{table}` (id BIGINT NOT NULL PRIMARY KEY)"
            )),
        )
        .await?;
    driver
        .execute(
            &config,
            &Query::new(format!(
                "CREATE TRIGGER `{trigger_name}` AFTER INSERT ON `{table}` \
                 FOR EACH ROW INSERT INTO `{audit_table}` (subject_id) VALUES (NEW.id)"
            )),
        )
        .await?;

    let triggers = driver.list_triggers(&config, &schema, &table).await?;
    let trigger = triggers
        .iter()
        .find(|candidate| candidate.name == trigger_name)
        .ok_or_else(|| DomainError::QueryFailed("MySQL 应返回刚创建的触发器".into()))?;
    assert_eq!(trigger.timing, "AFTER");
    assert_eq!(trigger.event, "INSERT");
    assert!(trigger.definition.contains(&audit_table));

    driver
        .execute(
            &config,
            &Query::new(format!(
                "DROP TABLE IF EXISTS `{table}`; DROP TABLE IF EXISTS `{audit_table}`"
            )),
        )
        .await?;
    Ok(())
}
