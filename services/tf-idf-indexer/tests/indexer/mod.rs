use scraper::Html;
use tf_idf_indexer::*;
use utils::AddToDb;

use crate::common;

mod parse_page;

#[tokio::test]
async fn test_refresh_queue() -> sqlx::Result<()> {
    let (_container, pool) = common::setup("dummy_data").await;

    let mut indexer = Indexer::new(&pool).await;

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

    let indexer = Indexer::new(&pool).await;

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
