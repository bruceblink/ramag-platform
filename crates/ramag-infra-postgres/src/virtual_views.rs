//! PostgreSQL Virtual views 元数据能力。

use ramag_domain::entities::VirtualView;
use ramag_domain::error::Result;
use sqlx::PgPool;

pub async fn list_virtual_views(_pool: &PgPool) -> Result<Vec<VirtualView>> {
    Ok(vec![VirtualView {
        name: "sessions".into(),
        detail: Some("只读会话快照".into()),
        read_only: true,
    }])
}

pub fn virtual_view_query(name: &str) -> Option<String> {
    (name == "sessions").then(|| {
        "SELECT pid, usename, datname, client_addr, state, query_start, query \
         FROM pg_catalog.pg_stat_activity ORDER BY pid"
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::virtual_view_query;

    #[test]
    fn sessions_virtual_view_is_read_only_query() {
        let sql = virtual_view_query("sessions").unwrap_or_default();
        assert!(sql.contains("pg_catalog.pg_stat_activity"));
        assert!(virtual_view_query("unknown").is_none());
    }
}
