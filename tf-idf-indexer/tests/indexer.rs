use std::collections::{HashMap, HashSet};

use scraper::Html;
use sqlx::postgres::types::PgHstore;
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
        assert!(indexer.contains_page(&page));
    }

    Ok(())
}

#[tokio::test]
async fn test_new_with_pool() {
    let (_container, pool) = common::setup("dummy_data").await;

    let indexer = Indexer::new_with_pool(&pool).await;

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

    for page in expected_pages {
        assert!(indexer.contains_page(&page));
    }

    // ladder
    let expected_ladder_tf = PgHstore::from_iter([
        ("0".to_string(), "0".to_string()),
        ("1".to_string(), "0".to_string()),
        ("2".to_string(), "0".to_string()),
        ("3".to_string(), "0".to_string()),
    ]);

    let expected_ladder_tf_idf = PgHstore::from_iter([
        ("0".to_string(), "0".to_string()),
        ("1".to_string(), "0".to_string()),
        ("2".to_string(), "0".to_string()),
        ("3".to_string(), "0".to_string()),
    ]);

    // hippo
    let expected_hippo_tf = PgHstore::from_iter([
        ("0".to_string(), "0".to_string()),
        ("1".to_string(), "0".to_string()),
        ("2".to_string(), "0".to_string()),
        ("3".to_string(), "0".to_string()),
    ]);

    let expected_hippo_tf_idf = PgHstore::from_iter([
        ("0".to_string(), "0".to_string()),
        ("1".to_string(), "0".to_string()),
        ("2".to_string(), "0".to_string()),
        ("3".to_string(), "0".to_string()),
    ]);

    // pipe
    let expected_pipe_tf = PgHstore::from_iter([
        ("0".to_string(), "0.3333".to_string()),
        ("1".to_string(), "0".to_string()),
        ("2".to_string(), "0".to_string()),
        ("3".to_string(), "0".to_string()),
    ]);

    let expected_pipe_tf_idf = PgHstore::from_iter([
        ("0".to_string(), "0.3662".to_string()),
        ("1".to_string(), "0".to_string()),
        ("2".to_string(), "0".to_string()),
        ("3".to_string(), "0".to_string()),
    ]);

    let expected_terms = vec![
        Term::new(
            "ladder".into(),
            ordered_float::OrderedFloat(0.0),
            3,
            expected_ladder_tf,
            expected_ladder_tf_idf,
        ),
        Term::new(
            "hippopotamus".into(),
            ordered_float::OrderedFloat(0.405465),
            2,
            expected_hippo_tf,
            expected_hippo_tf_idf,
        ),
        Term::new(
            "pipe".into(),
            ordered_float::OrderedFloat(1.098612),
            1,
            expected_pipe_tf,
            expected_pipe_tf_idf,
        ),
    ];

    for term in expected_terms {
        assert!(indexer.contains_term(&term));
    }
}
