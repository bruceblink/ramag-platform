//! MySQL Server Objects integration coverage against a local Docker database.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use ramag_domain::entities::{ConnectionConfig, ServerObjectGroup};
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
        ..ConnectionConfig::new_mysql("server-objects-test", host, port, user)
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn list_server_objects_returns_datagrip_groups() {
    let Some(config) = config_from_env() else {
        eprintln!("[SKIP] integration test skipped: 设置 RAMAG_TEST_MYSQL_* 环境变量后运行");
        return;
    };
    let driver = MysqlDriver::new();
    let groups: Vec<ServerObjectGroup> = driver
        .list_server_objects(&config)
        .await
        .expect("list_server_objects 失败");
    assert!(groups.iter().any(|group| group.name == "collations"));
    assert!(groups.iter().any(|group| group.name == "users"));
    assert!(groups.iter().all(|group| !group.items.is_empty()));
}
