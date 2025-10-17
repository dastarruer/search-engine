use std::collections::{HashMap, HashSet};

use scraper::Html;
use tf_idf_indexer::*;

mod common;

#[tokio::test]
async fn test_refresh_queue() -> sqlx::Result<()> {
    let (_container, pool) = common::setup("refresh_queue").await;

    let mut indexer = Indexer::new(HashMap::new(), HashSet::new());
    let expected_page = Page::new(
        Html::parse_document("<body><p>hippopotamus hippopotamus hippopotamus</p></body>"),
        1,
    );

    indexer.refresh_queue(&pool).await;

    assert_eq!(indexer.num_pages(), 3);
    assert!(indexer.contains(&expected_page));

    Ok(())
}
