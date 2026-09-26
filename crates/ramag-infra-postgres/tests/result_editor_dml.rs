//! PostgreSQL 17 Docker 验收：结果编辑器 UPDATE 的自动提交和事务回读。

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use ramag_domain::entities::{ConnectionConfig, DriverKind, Query, QueryResult, Value};
use ramag_domain::traits::Driver;
use ramag_infra_postgres::PostgresDriver;

fn config_from_env() -> Option<ConnectionConfig> {
    let host = std::env::var("RAMAG_TEST_PG_HOST").ok()?;
    let port = std::env::var("RAMAG_TEST_PG_PORT").ok()?.parse().ok()?;
    let user = std::env::var("RAMAG_TEST_PG_USER").ok()?;
    let password = std::env::var("RAMAG_TEST_PG_PASSWORD").ok()?;
    let database = std::env::var("RAMAG_TEST_PG_DB").ok()?;
    Some(ConnectionConfig {
        driver: DriverKind::Postgres,
        password,
        database: Some(database),
        ..ConnectionConfig::new_mysql("result-editor-dml", host, port, user)
    })
}

fn assert_value(result: &QueryResult, expected: &str) {
    assert!(matches!(
        result.rows.first().and_then(|row| row.values.first()),
        Some(Value::Text(value)) if value == expected
    ));
}

/// Exercises result-cell UPDATE semantics without relying on a SQL transaction mock.
#[tokio::test(flavor = "multi_thread")]
async fn result_editor_update_round_trip_uses_postgres_dml_and_transactions() {
    let Some(config) = config_from_env() else {
        eprintln!("[SKIP] PostgreSQL Docker DML 验收跳过：设置 RAMAG_TEST_PG_* 环境变量后运行");
        return;
    };
    let driver = PostgresDriver::new();
    let observer = PostgresDriver::new();
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间应晚于 Unix epoch")
        .as_nanos();
    let table = format!(
        "ramag_result_editor_probe_{}_{}",
        std::process::id(),
        suffix
    );
    let qualified_table = format!("public.\"{table}\"");

    driver
        .execute(
            &config,
            &Query::new(format!(
                "CREATE TABLE {qualified_table} (id BIGINT PRIMARY KEY, value VARCHAR(64) NOT NULL)"
            )),
        )
        .await
        .expect("创建 PostgreSQL 结果编辑测试表失败");
    driver
        .execute(
            &config,
            &Query::new(format!(
                "INSERT INTO {qualified_table} (id, value) VALUES (1, 'before'), (2, 'stable')"
            )),
        )
        .await
        .expect("写入 PostgreSQL 结果编辑测试数据失败");

    let updated = driver
        .execute(
            &config,
            &Query::new(format!(
                "UPDATE {qualified_table} SET value = 'auto' WHERE id = 1"
            )),
        )
        .await
        .expect("PostgreSQL 自动提交 UPDATE 失败");
    assert_eq!(updated.affected_rows, 1);
    let visible = observer
        .execute(
            &config,
            &Query::new(format!("SELECT value FROM {qualified_table} WHERE id = 1")),
        )
        .await
        .expect("读取 PostgreSQL 自动提交结果失败");
    assert_value(&visible, "auto");

    let rollback_id = driver
        .begin_transaction(&config)
        .await
        .expect("开启 PostgreSQL 结果编辑回滚事务失败");
    let updated = driver
        .execute_in_transaction(
            &config,
            &rollback_id,
            &Query::new(format!(
                "UPDATE {qualified_table} SET value = 'rolled-back' WHERE id = 2"
            )),
        )
        .await
        .expect("PostgreSQL 事务内 UPDATE 失败");
    assert_eq!(updated.affected_rows, 1);
    let inside = driver
        .execute_in_transaction(
            &config,
            &rollback_id,
            &Query::new(format!("SELECT value FROM {qualified_table} WHERE id = 2")),
        )
        .await
        .expect("读取 PostgreSQL 事务内 UPDATE 结果失败");
    assert_value(&inside, "rolled-back");
    let outside = observer
        .execute(
            &config,
            &Query::new(format!("SELECT value FROM {qualified_table} WHERE id = 2")),
        )
        .await
        .expect("读取 PostgreSQL 未提交 UPDATE 失败");
    assert_value(&outside, "stable");
    driver
        .rollback_transaction(&config, &rollback_id)
        .await
        .expect("PostgreSQL 结果编辑回滚失败");

    let commit_id = driver
        .begin_transaction(&config)
        .await
        .expect("开启 PostgreSQL 结果编辑提交事务失败");
    let updated = driver
        .execute_in_transaction(
            &config,
            &commit_id,
            &Query::new(format!(
                "UPDATE {qualified_table} SET value = 'committed' WHERE id = 2"
            )),
        )
        .await
        .expect("PostgreSQL 提交事务内 UPDATE 失败");
    assert_eq!(updated.affected_rows, 1);
    driver
        .commit_transaction(&config, &commit_id)
        .await
        .expect("PostgreSQL 结果编辑提交事务失败");
    let committed = observer
        .execute(
            &config,
            &Query::new(format!("SELECT value FROM {qualified_table} WHERE id = 2")),
        )
        .await
        .expect("读取 PostgreSQL 提交后的 UPDATE 失败");
    assert_value(&committed, "committed");

    driver
        .execute(
            &config,
            &Query::new(format!("DROP TABLE {qualified_table}")),
        )
        .await
        .expect("清理 PostgreSQL 结果编辑测试表失败");
}

/// Confirms that a lost PostgreSQL endpoint returns a visible driver error instead of a false success.
#[tokio::test(flavor = "multi_thread")]
async fn result_editor_dml_reports_unreachable_postgres_connection() {
    let Some(mut config) = config_from_env() else {
        eprintln!("[SKIP] PostgreSQL 失效连接验收跳过：设置 RAMAG_TEST_PG_* 环境变量后运行");
        return;
    };
    config.port = u16::MAX;
    let driver = PostgresDriver::new();
    let error = driver
        .execute(&config, &Query::new("UPDATE missing_table SET value = 1"))
        .await
        .expect_err("不可达 PostgreSQL 端点必须返回错误");
    assert!(
        !error.to_string().trim().is_empty(),
        "PostgreSQL 连接错误应包含可展示的摘要"
    );
}
