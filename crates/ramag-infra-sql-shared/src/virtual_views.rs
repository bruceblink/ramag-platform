//! SQL 驱动共享的 Virtual views 元数据入口。

use ramag_domain::entities::{ConnectionConfig, VirtualView};
use ramag_domain::error::Result;
use sqlx::{Database, Executor, IntoArguments, Pool};

use crate::backend::{SqlBackend, get_pool};

/// 取得连接池后调用具体驱动的虚拟视图能力，统一连接代际和配置校验。
pub async fn list_virtual_views_impl<B>(
    backend: &B,
    config: &ConnectionConfig,
) -> Result<Vec<VirtualView>>
where
    B: SqlBackend,
    for<'q> <B::Db as Database>::Arguments<'q>: IntoArguments<'q, B::Db>,
    for<'c> &'c Pool<B::Db>: Executor<'c, Database = B::Db>,
    for<'c> &'c mut <B::Db as Database>::Connection: Executor<'c, Database = B::Db>,
{
    let pool = get_pool(backend, config).await?;
    backend.list_virtual_views_impl(&pool).await
}
