//! MySQL 8.4 Docker 验收：结果编辑器 UPDATE 的自动提交和事务回读。

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use ramag_domain::entities::{ConnectionConfig, Query, QueryResult, Value};
use ramag_domain::traits::Driver;
use ramag_infra_mysql::MysqlDriver;

fn config_from_env() -> Option<ConnectionConfig> {
    let host = std::env::var("RAMAG_TEST_MYSQL_HOST").ok()?;
    let port = std::env::var("RAMAG_TEST_MYSQL_PORT").ok()?.parse().ok()?;
    let user = std::env::var("RAMAG_TEST_MYSQL_USER").ok()?;
    let password = std::env::var("RAMAG_TEST_MYSQL_PASSWORD").ok()?;
    let database = std::env::var("RAMAG_TEST_MYSQL_DB").ok();
    Some(ConnectionConfig {
        password,
        database,
        ..ConnectionConfig::new_mysql("result-editor-dml", host, port, user)
    })
}

fn assert_value(result: &QueryResult, expected: &str) {
    assert!(matches!(
        result.rows.first().and_then(|row| row.values.first()),
        Some(Value::Text(value)) if value == expected
    ));
}

/// Exercises the same UPDATE and transaction visibility rules used by result-cell submission.
#[tokio::test(flavor = "multi_thread")]
async fn result_editor_update_round_trip_uses_mysql_dml_and_transactions() {
    let Some(config) = config_from_env() else {
        eprintln!("[SKIP] MySQL Docker DML 验收跳过：设置 RAMAG_TEST_MYSQL_* 环境变量后运行");
        return;
    };
    let driver = MysqlDriver::new();
    let observer = MysqlDriver::new();
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间应晚于 Unix epoch")
        .as_nanos();
    let table = format!(
        "ramag_result_editor_probe_{}_{}",
        std::process::id(),
        suffix
    );
    let quoted_table = format!("`{table}`");

    driver
        .execute(
            &config,
            &Query::new(format!(
                "CREATE TABLE {quoted_table} (id BIGINT PRIMARY KEY, value VARCHAR(64) NOT NULL)"
            )),
        )
        .await
        .expect("创建 MySQL 结果编辑测试表失败");
    driver
        .execute(
            &config,
            &Query::new(format!(
                "INSERT INTO {quoted_table} (id, value) VALUES (1, 'before'), (2, 'stable')"
            )),
        )
        .await
        .expect("写入 MySQL 结果编辑测试数据失败");

    let updated = driver
        .execute(
            &config,
            &Query::new(format!(
                "UPDATE {quoted_table} SET value = 'auto' WHERE id = 1 LIMIT 1"
            )),
        )
        .await
        .expect("MySQL 自动提交 UPDATE 失败");
    assert_eq!(updated.affected_rows, 1);
    let visible = observer
        .execute(
            &config,
            &Query::new(format!("SELECT value FROM {quoted_table} WHERE id = 1")),
        )
        .await
        .expect("读取 MySQL 自动提交结果失败");
    assert_value(&visible, "auto");

    let rollback_id = driver
        .begin_transaction(&config)
        .await
        .expect("开启 MySQL 结果编辑回滚事务失败");
    let updated = driver
        .execute_in_transaction(
            &config,
            &rollback_id,
            &Query::new(format!(
                "UPDATE {quoted_table} SET value = 'rolled-back' WHERE id = 2 LIMIT 1"
            )),
        )
        .await
        .expect("MySQL 事务内 UPDATE 失败");
    assert_eq!(updated.affected_rows, 1);
    let inside = driver
        .execute_in_transaction(
            &config,
            &rollback_id,
            &Query::new(format!("SELECT value FROM {quoted_table} WHERE id = 2")),
        )
        .await
        .expect("读取 MySQL 事务内 UPDATE 结果失败");
    assert_value(&inside, "rolled-back");
    let outside = observer
        .execute(
            &config,
            &Query::new(format!("SELECT value FROM {quoted_table} WHERE id = 2")),
        )
        .await
        .expect("读取 MySQL 未提交 UPDATE 失败");
    assert_value(&outside, "stable");
    driver
        .rollback_transaction(&config, &rollback_id)
        .await
        .expect("MySQL 结果编辑回滚失败");

    let commit_id = driver
        .begin_transaction(&config)
        .await
        .expect("开启 MySQL 结果编辑提交事务失败");
    let updated = driver
        .execute_in_transaction(
            &config,
            &commit_id,
            &Query::new(format!(
                "UPDATE {quoted_table} SET value = 'committed' WHERE id = 2 LIMIT 1"
            )),
        )
        .await
        .expect("MySQL 提交事务内 UPDATE 失败");
    assert_eq!(updated.affected_rows, 1);
    driver
        .commit_transaction(&config, &commit_id)
        .await
        .expect("MySQL 结果编辑提交事务失败");
    let committed = observer
        .execute(
            &config,
            &Query::new(format!("SELECT value FROM {quoted_table} WHERE id = 2")),
        )
        .await
        .expect("读取 MySQL 提交后的 UPDATE 失败");
    assert_value(&committed, "committed");

    driver
        .execute(&config, &Query::new(format!("DROP TABLE {quoted_table}")))
        .await
        .expect("清理 MySQL 结果编辑测试表失败");
}
