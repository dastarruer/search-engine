use std::collections::{HashMap, HashSet};

use scraper::Html;
use tf_idf_indexer::*;

mod common;

#[tokio::test]
async fn test_refresh_queue() -> sqlx::Result<()> {
    let (_container, pool) = common::setup("refresh_queue").await;

    let mut indexer = Indexer::new(HashMap::new(), HashSet::new());

    let mut expected_pages = vec![];
    for i in 0..3 {
        let expected_page = Page::new(
            Html::parse_document("<body><p>hippopotamus hippopotamus hippopotamus</p></body>"),
            i + 1,
        );

        expected_pages.push(expected_page);
    }

    indexer.refresh_queue(&pool).await;

    assert_eq!(indexer.num_pages(), expected_pages.len() as i32);

    for page in expected_pages {
        assert!(indexer.contains(&page));
    }

    Ok(())
}
