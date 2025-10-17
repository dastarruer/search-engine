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

    let mut ladder_tf = HashMap::new();
    ladder_tf.insert(0, ordered_float::OrderedFloat(0.0));
    ladder_tf.insert(1, ordered_float::OrderedFloat(0.0));
    ladder_tf.insert(2, ordered_float::OrderedFloat(0.0));

    let ladder_tf_idf = ladder_tf.clone();

    let mut hippo_tf = HashMap::new();
    hippo_tf.insert(0, ordered_float::OrderedFloat(0.0));
    hippo_tf.insert(1, ordered_float::OrderedFloat(0.6667));
    hippo_tf.insert(2, ordered_float::OrderedFloat(0.6667));

    let mut hippo_tf_idf = HashMap::new();
    hippo_tf_idf.insert(0, ordered_float::OrderedFloat(0.0));
    hippo_tf_idf.insert(1, ordered_float::OrderedFloat(0.2703));
    hippo_tf_idf.insert(2, ordered_float::OrderedFloat(0.2703));

    let mut pipe_tf = HashMap::new();
    pipe_tf.insert(0, ordered_float::OrderedFloat(0.3333));
    pipe_tf.insert(1, ordered_float::OrderedFloat(0.0));
    pipe_tf.insert(2, ordered_float::OrderedFloat(0.0));

    let mut pipe_tf_idf = HashMap::new();
    pipe_tf_idf.insert(0, ordered_float::OrderedFloat(0.3662));
    pipe_tf_idf.insert(1, ordered_float::OrderedFloat(0.0));
    pipe_tf_idf.insert(2, ordered_float::OrderedFloat(0.0));

    let expected_terms = vec![
        Term::new(
            "ladder".into(),
            ordered_float::OrderedFloat(0.0),
            3,
            ladder_tf,
            ladder_tf_idf,
        ),
        Term::new(
            "hippopotamus".into(),
            ordered_float::OrderedFloat(0.405465),
            2,
            hippo_tf,
            hippo_tf_idf,
        ),
        Term::new(
            "pipe".into(),
            ordered_float::OrderedFloat(1.098612),
            1,
            pipe_tf,
            pipe_tf_idf,
        ),
    ];

    for term in expected_terms {
        assert!(indexer.contains_term(&term));
    }
}
