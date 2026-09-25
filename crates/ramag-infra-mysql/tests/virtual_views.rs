//! MySQL Virtual views integration coverage against a local Docker database.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use ramag_domain::entities::{ConnectionConfig, Query};
use ramag_domain::traits::Driver;
use ramag_infra_mysql::MysqlDriver;

fn config_from_env() -> Option<ConnectionConfig> {
    let host = std::env::var("RAMAG_TEST_MYSQL_HOST").ok()?;
    let port: u16 = std::env::var("RAMAG_TEST_MYSQL_PORT").ok()?.parse().ok()?;
    let user = std::env::var("RAMAG_TEST_MYSQL_USER").ok()?;
    let password = std::env::var("RAMAG_TEST_MYSQL_PASSWORD").ok()?;
    let database = std::env::var("RAMAG_TEST_MYSQL_DB").ok();
    Some(ConnectionConfig {
        password,
        database,
        ..ConnectionConfig::new_mysql("virtual-views-test", host, port, user)
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn sessions_virtual_view_reads_mysql_processlist() {
    let Some(config) = config_from_env() else {
        eprintln!("[SKIP] integration test skipped: 设置 RAMAG_TEST_MYSQL_* 环境变量后运行");
        return;
    };
    let driver = MysqlDriver::new();
    let views = driver
        .list_virtual_views(&config)
        .await
        .expect("list_virtual_views 失败");
    assert!(
        views
            .iter()
            .any(|view| view.name == "sessions" && view.read_only)
    );

    let query = driver
        .virtual_view_query("sessions")
        .expect("sessions 查询应存在");
    let result = driver
        .execute(&config, &Query::new(query))
        .await
        .expect("读取 MySQL PROCESSLIST 失败");
    assert!(
        result
            .columns
            .iter()
            .any(|column| column.eq_ignore_ascii_case("ID"))
    );
}
