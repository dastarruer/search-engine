use std::collections::{HashMap, HashSet};

use scraper::Html;
use tf_idf_indexer::*;

mod common;

#[tokio::test]
async fn test_refresh_queue() -> sqlx::Result<()> {
    let (_container, pool) = common::setup("dummy_data").await;

    let mut indexer = Indexer::new(HashMap::new(), HashSet::new());

    let expected_pages = vec![
        Page::new(
            Html::parse_document("<body><p>ladder ladder pipe</p></body>"),
            1,
        ),
        Page::new(
            Html::parse_document("<body><p>hippopotamus ladder hippopotamus</p></body>"),
            2,
        ),
        Page::new(
            Html::parse_document("<body><p>ladder hippopotamus hippopotamus</p></body>"),
            3,
        ),
    ];

    indexer.refresh_queue(&pool).await;

    assert_eq!(indexer.num_pages(), expected_pages.len() as i32);

    for page in expected_pages {
        assert!(indexer.contains(&page));
    }

    Ok(())
}
