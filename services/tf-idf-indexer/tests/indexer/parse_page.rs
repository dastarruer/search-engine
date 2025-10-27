// NOTE: These tests do not work, since refresh_queue is not properly implemented for tests
use sqlx::postgres::types::PgHstore;
use tf_idf_indexer::{Indexer, Term};

use crate::common::{self};

#[tokio::test]
async fn test_parse_page() {
    let (_container, pool) = common::setup("dummy_pages").await;

    let mut indexer = Indexer::new(&pool).await;
    indexer.run(&pool).await;

    let actual_terms_query = r#"SELECT * FROM terms;"#;
    let actual_terms: Vec<Term> = sqlx::query(actual_terms_query)
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .flat_map(Term::try_from)
        .collect();

    let expected_terms = test_parse_page_dummy_terms();

    for term in &expected_terms {
        assert!(actual_terms.contains(term))
    }
}

#[tokio::test]
async fn test_parse_page_with_existing_terms() {
    let (_container, pool) = common::setup("test_parse_page_with_existing_terms").await;

    let mut indexer = Indexer::new(&pool).await;
    indexer.run(&pool).await;

    let actual_terms_query = r#"SELECT * FROM terms;"#;
    let actual_terms: Vec<Term> = sqlx::query(actual_terms_query)
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .flat_map(Term::try_from)
        .collect();
    let expected_terms = test_parse_page_with_existing_terms_dummy_terms();

    for term in &expected_terms {
        assert!(actual_terms.contains(term))
    }
}

pub fn test_parse_page_dummy_terms() -> Vec<Term> {
    // ladder
    let expected_ladder_tf = PgHstore::from_iter([
        ("1".to_string(), Some("2".to_string())),
        ("2".to_string(), Some("1".to_string())),
        ("3".to_string(), Some("1".to_string())),
    ]);
    let expected_ladder_tf_idf = PgHstore::from_iter([
        ("1".to_string(), Some("0".to_string())),
        ("2".to_string(), Some("0".to_string())),
        ("3".to_string(), Some("0".to_string())),
    ]);

    // hippopotamus
    let expected_hippo_tf = PgHstore::from_iter([
        ("2".to_string(), Some("2".to_string())),
        ("3".to_string(), Some("2".to_string())),
    ]);
    let expected_hippo_tf_idf = PgHstore::from_iter([
        ("2".to_string(), Some("0.3521825".to_string())),
        ("3".to_string(), Some("0.3521825".to_string())),
    ]);

    // pipe
    let expected_pipe_tf = PgHstore::from_iter([("1".to_string(), Some("1".to_string()))]);
    let expected_pipe_tf_idf =
        PgHstore::from_iter([("1".to_string(), Some("0.47712123".to_string()))]);

    vec![
        Term::new(
            "ladder".into(),
            ordered_float::OrderedFloat(0.0),
            3,
            expected_ladder_tf,
            expected_ladder_tf_idf,
        )
        .unwrap(),
        Term::new(
            "hippopotamus".into(),
            ordered_float::OrderedFloat(0.17609125),
            2,
            expected_hippo_tf,
            expected_hippo_tf_idf,
        )
        .unwrap(),
        Term::new(
            "pipe".into(),
            ordered_float::OrderedFloat(0.47712123),
            1,
            expected_pipe_tf,
            expected_pipe_tf_idf,
        )
        .unwrap(),
    ]
}

fn test_parse_page_with_existing_terms_dummy_terms() -> Vec<Term> {
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

    vec![
        Term::new(
            "ladder".into(),
            ordered_float::OrderedFloat(std::f32::consts::LOG10_2),
            2,
            expected_ladder_tf,
            expected_ladder_tf_idf,
        )
        .unwrap(),
        Term::new(
            "hippopotamus".into(),
            ordered_float::OrderedFloat(std::f32::consts::LOG10_2),
            2,
            expected_hippo_tf,
            expected_hippo_tf_idf,
        )
        .unwrap(),
        Term::new(
            "pipe".into(),
            // log4
            ordered_float::OrderedFloat(0.60206),
            1,
            expected_pipe_tf,
            expected_pipe_tf_idf,
        )
        .unwrap(),
        Term::new(
            "seagull".into(),
            ordered_float::OrderedFloat(std::f32::consts::LOG10_2),
            2,
            expected_seagull_tf,
            expected_seagull_tf_idf,
        )
        .unwrap(),
    ]
}
