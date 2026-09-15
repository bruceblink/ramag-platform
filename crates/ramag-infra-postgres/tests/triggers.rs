//! PostgreSQL 触发器元数据的 Docker 集成测试。

use ramag_domain::entities::{ConnectionConfig, DriverKind, Query};
use ramag_domain::error::{DomainError, Result};
use ramag_domain::traits::Driver;
use ramag_infra_postgres::PostgresDriver;

/// 从本机 Docker PostgreSQL 测试环境读取连接配置；环境变量不完整时跳过测试。
fn config_from_env() -> Option<ConnectionConfig> {
    let host = std::env::var("RAMAG_TEST_PG_HOST").ok()?;
    let port: u16 = std::env::var("RAMAG_TEST_PG_PORT").ok()?.parse().ok()?;
    let user = std::env::var("RAMAG_TEST_PG_USER").ok()?;
    let password = std::env::var("RAMAG_TEST_PG_PASSWORD").ok()?;
    let database = std::env::var("RAMAG_TEST_PG_DB").ok()?;

    Some(ConnectionConfig {
        driver: DriverKind::Postgres,
        password,
        database: Some(database),
        ..ConnectionConfig::new_mysql("integration-test", host, port, user)
    })
}

/// 创建临时表、函数和触发器，再确认表属性所需元数据可从 PostgreSQL 读取。
#[tokio::test(flavor = "multi_thread")]
async fn list_triggers_for_temporary_table() -> Result<()> {
    let Some(config) = config_from_env() else {
        eprintln!("[SKIP] integration test skipped: 设置 RAMAG_TEST_PG_* 环境变量后运行");
        return Ok(());
    };
    let driver = PostgresDriver::new();
    let schema = "public";
    let suffix = std::process::id();
    let table = format!("ramag_trigger_probe_{suffix}");
    let audit_table = format!("ramag_trigger_audit_{suffix}");
    let function = format!("ramag_trigger_fn_{suffix}");
    let trigger_name = format!("ramag_trigger_probe_after_insert_{suffix}");

    driver
        .execute(
            &config,
            &Query::new(format!(
                "DROP TABLE IF EXISTS {schema}.\"{table}\"; \
                 DROP TABLE IF EXISTS {schema}.\"{audit_table}\"; \
                 DROP FUNCTION IF EXISTS {schema}.\"{function}\"(); \
                 CREATE TABLE {schema}.\"{audit_table}\" (subject_id BIGINT NOT NULL); \
                 CREATE TABLE {schema}.\"{table}\" (id BIGINT NOT NULL PRIMARY KEY)"
            )),
        )
        .await?;
    driver
        .execute(
            &config,
            &Query::new(format!(
                "CREATE FUNCTION {schema}.\"{function}\"() RETURNS trigger \
                 LANGUAGE plpgsql AS $$ BEGIN INSERT INTO {schema}.\"{audit_table}\" (subject_id) \
                 VALUES (NEW.id); RETURN NEW; END; $$"
            )),
        )
        .await?;
    driver
        .execute(
            &config,
            &Query::new(format!(
                "CREATE TRIGGER \"{trigger_name}\" AFTER INSERT ON {schema}.\"{table}\" \
                 FOR EACH ROW EXECUTE FUNCTION {schema}.\"{function}\"()"
            )),
        )
        .await?;

    let triggers = driver.list_triggers(&config, schema, &table).await?;
    let trigger = triggers
        .iter()
        .find(|candidate| candidate.name == trigger_name)
        .ok_or_else(|| DomainError::QueryFailed("PostgreSQL 应返回刚创建的触发器".into()))?;
    assert_eq!(trigger.timing, "AFTER");
    assert_eq!(trigger.event, "INSERT");
    assert!(trigger.definition.contains(&function));

    driver
        .execute(
            &config,
            &Query::new(format!(
                "DROP TABLE IF EXISTS {schema}.\"{table}\"; \
                 DROP TABLE IF EXISTS {schema}.\"{audit_table}\"; \
                 DROP FUNCTION IF EXISTS {schema}.\"{function}\"()"
            )),
        )
        .await?;
    Ok(())
}
