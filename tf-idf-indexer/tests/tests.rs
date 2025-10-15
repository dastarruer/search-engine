use std::collections::{HashMap, HashSet};

use sqlx::{Pool, Postgres};
use tf_idf_indexer::*;

mod common;

#[sqlx::test(migrations = "../migrations")]
async fn test_refresh_queue(pool: Pool<Postgres>) -> sqlx::Result<()> {
    common::setup().await;

    let mut indexer = Indexer::new(HashMap::new(), HashSet::new());

    indexer.refresh_queue(&pool).await;

    // assert_eq!(indexer);

    Ok(())
}
