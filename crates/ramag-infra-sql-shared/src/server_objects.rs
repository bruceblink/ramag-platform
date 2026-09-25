//! SQL 驱动共享的 Server Objects 元数据入口。

use ramag_domain::entities::{ConnectionConfig, ServerObjectGroup};
use ramag_domain::error::Result;
use sqlx::{Database, Executor, IntoArguments, Pool};

use crate::backend::{SqlBackend, get_pool};

/// 从共享连接池取得真实数据库元数据，并把方言细节留给具体驱动。
pub async fn list_server_objects_impl<B>(
    backend: &B,
    config: &ConnectionConfig,
) -> Result<Vec<ServerObjectGroup>>
where
    B: SqlBackend,
    for<'q> <B::Db as Database>::Arguments<'q>: IntoArguments<'q, B::Db>,
    for<'c> &'c Pool<B::Db>: Executor<'c, Database = B::Db>,
    for<'c> &'c mut <B::Db as Database>::Connection: Executor<'c, Database = B::Db>,
{
    let pool = get_pool(backend, config).await?;
    backend.list_server_objects_impl(&pool).await
}
