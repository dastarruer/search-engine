use std::collections::{HashMap, HashSet};

use scraper::Html;
use sqlx::postgres::types::PgHstore;
use tf_idf_indexer::*;

use crate::common::dummy_terms;

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

    assert_eq!(indexer.num_pages(), expected_pages.len() as i32);

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

    // expected: { term: "seagull", idf: 0.47712123, page_frequency: 1, tf_scores: PgHstore({"3": Some("1")}), tf_idf_scores: PgHstore({"3": Some("0.47712123")})
    // actual: term: "seagull", idf: 0.47712123, page_frequency: 1, tf_scores: PgHstore({"3": Some("1")}), tf_idf_scores: PgHstore({"3": Some("0.47712123")})
    let expected_seagull_tf = PgHstore::from_iter([("3".to_string(), Some("1".to_string()))]);
    let expected_seagull_tf_idf =
        PgHstore::from_iter([("3".to_string(), Some("0.47712123".to_string()))]);
    let expected_seagull = Term::new(
        "seagull".into(),
        ordered_float::OrderedFloat(0.47712123),
        1,
        expected_seagull_tf,
        expected_seagull_tf_idf,
    );

    let expected_ladder_tf = PgHstore::from_iter([
        ("1".to_string(), Some("2".to_string())),
        ("2".to_string(), Some("1".to_string())),
        ("3".to_string(), Some("0".to_string())),
    ]);
    let expected_ladder_tf_idf = PgHstore::from_iter([
        ("1".to_string(), Some("0.3521825".to_string())),
        ("2".to_string(), Some("0.17609125".to_string())),
        ("3".to_string(), Some("0".to_string())),
    ]);
    let expected_ladder = Term::new(
        "ladder".into(),
        ordered_float::OrderedFloat(0.17609125),
        2,
        expected_ladder_tf,
        expected_ladder_tf_idf,
    );

    let mut expected_terms = dummy_terms();
    expected_terms.push(expected_seagull);
    expected_terms[0] = expected_ladder;

    for term in &expected_terms {
        println!(
            "Expected terms: {:?}\nActual terms: {:?}",
            expected_terms, actual_terms
        );
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
