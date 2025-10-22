use scraper::Html;
use sqlx::postgres::types::PgHstore;
use std::collections::{HashMap, HashSet};
use tf_idf_indexer::*;
use utils::{AddToDb};

use crate::common::{dummy_terms};

mod common;

#[tokio::test]
async fn test_refresh_queue() -> sqlx::Result<()> {
    let (_container, pool) = common::setup("dummy_data").await;

    let mut indexer = Indexer::new(HashMap::new(), HashSet::new()).await;

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

    assert_eq!(indexer.num_pages(), expected_pages.len() as i64);

    for page in expected_pages {
        assert!(indexer.contains_page(&page));
    }

    Ok(())
}

#[tokio::test]
async fn test_parse_page() {
    let (_container, pool) = common::setup("dummy_pages").await;

    let mut indexer = Indexer::new_with_pool(&pool).await;
    indexer.run(&pool).await;

    let actual_terms_query = r#"SELECT * FROM terms;"#;
    let actual_terms: Vec<Term> = sqlx::query(actual_terms_query)
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(Term::from)
        .collect();

    let expected_terms = dummy_terms();

    for term in &expected_terms {
        assert!(actual_terms.contains(term))
    }
}

#[tokio::test]
async fn test_parse_page_with_existing_terms() {
    let (_container, pool) = common::setup("test_parse_page_with_existing_terms").await;

    let mut indexer = Indexer::new_with_pool(&pool).await;
    indexer.run(&pool).await;

    let actual_terms_query = r#"SELECT * FROM terms;"#;
    let actual_terms: Vec<Term> = sqlx::query(actual_terms_query)
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(Term::from)
        .collect();

    let expected_seagull_tf = PgHstore::from_iter([
        ("3".to_string(), Some("1".to_string())),
        ("4".to_string(), Some("1".to_string())),
    ]);
    let expected_seagull_tf_idf = PgHstore::from_iter([
        ("3".to_string(), Some(std::f32::consts::LOG10_2.to_string())),
        ("4".to_string(), Some(std::f32::consts::LOG10_2.to_string())),
    ]);

    let expected_ladder_tf = PgHstore::from_iter([
        ("1".to_string(), Some("2".to_string())),
        ("2".to_string(), Some("1".to_string())),
    ]);
    let expected_ladder_tf_idf = PgHstore::from_iter([
        (
            "1".to_string(),
            Some((2.0 * std::f32::consts::LOG10_2).to_string()),
        ),
        ("2".to_string(), Some(std::f32::consts::LOG10_2.to_string())),
    ]);

    // hippopotamus
    let expected_hippo_tf = PgHstore::from_iter([
        ("2".to_string(), Some("2".to_string())),
        ("3".to_string(), Some("2".to_string())),
    ]);
    let expected_hippo_tf_idf = PgHstore::from_iter([
        (
            "2".to_string(),
            Some((2.0 * std::f32::consts::LOG10_2).to_string()),
        ),
        (
            "3".to_string(),
            Some((2.0 * std::f32::consts::LOG10_2).to_string()),
        ),
    ]);

    // pipe
    let expected_pipe_tf = PgHstore::from_iter([("1".to_string(), Some("1".to_string()))]);
    let expected_pipe_tf_idf = PgHstore::from_iter(
        // log4
        [("1".to_string(), Some(0.60206.to_string()))],
    );

    let expected_terms = vec![
        Term::new(
            "ladder".into(),
            ordered_float::OrderedFloat(std::f32::consts::LOG10_2),
            2,
            expected_ladder_tf,
            expected_ladder_tf_idf,
        ),
        Term::new(
            "hippopotamus".into(),
            ordered_float::OrderedFloat(std::f32::consts::LOG10_2),
            2,
            expected_hippo_tf,
            expected_hippo_tf_idf,
        ),
        Term::new(
            "pipe".into(),
            // log4
            ordered_float::OrderedFloat(0.60206),
            1,
            expected_pipe_tf,
            expected_pipe_tf_idf,
        ),
        Term::new(
            "seagull".into(),
            ordered_float::OrderedFloat(std::f32::consts::LOG10_2),
            2,
            expected_seagull_tf,
            expected_seagull_tf_idf,
        ),
    ];

    for term in &expected_terms {
        assert!(actual_terms.contains(term))
    }
}

#[tokio::test]
async fn test_add_term_to_db() {
    let (_container, pool) = common::setup("dummy_data").await;

    let unique_term_str = String::from("america");
    let unique_term = Term::from(unique_term_str.clone());

    let conflicting_term_str = String::from("hippopotamus");
    let conflicting_term = Term::from(conflicting_term_str.clone());

    unique_term.add_to_db(&pool).await;
    conflicting_term.add_to_db(&pool).await;

    let unique_term_err_msg = format!("Term {} should exist in the database.", unique_term_str);
    let conflicting_term_err_msg =
        format!("Term {} should exist in the database.", unique_term_str);

    let query = r#"SELECT * FROM terms WHERE term = $1"#;
    assert_eq!(
        Term::from(
            &sqlx::query(query)
                .bind(unique_term_str)
                .fetch_one(&pool)
                .await
                .expect(&unique_term_err_msg)
        ),
        unique_term
    );

    // Adding the conflicting term should update the term in the database
    assert_eq!(
        Term::from(
            &sqlx::query(query)
                .bind(conflicting_term_str)
                .fetch_one(&pool)
                .await
                .expect(&conflicting_term_err_msg)
        ),
        conflicting_term
    );
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
}
