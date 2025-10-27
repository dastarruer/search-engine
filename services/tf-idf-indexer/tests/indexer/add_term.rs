use crate::common;
use tf_idf_indexer::{db::{DbManager, RealDbManager}, Term};

#[tokio::test]
async fn test_add_term_to_db() {
    let (_container, pool) = common::setup("dummy_data").await;

    let db_manager = RealDbManager::new(pool.clone());

    let unique_term_str = String::from("america");
    let unique_term = Term::try_from(unique_term_str.clone()).unwrap();

    db_manager.add_term_to_db(&unique_term).await;

    let unique_term_err_msg = format!("Term {} should exist in the database.", unique_term_str);

    let query = r#"SELECT * FROM terms WHERE term = $1"#;
    assert_eq!(
        Term::try_from(
            &sqlx::query(query)
                .bind(unique_term_str)
                .fetch_one(&pool)
                .await
                .expect(&unique_term_err_msg)
        )
        .unwrap(),
        unique_term
    );
}

#[tokio::test]
async fn test_add_existing_term_to_db() {
    let (_container, pool) = common::setup("dummy_data").await;

    let db_manager = RealDbManager::new(pool.clone());

    let conflicting_term_str = String::from("hippopotamus");
    let conflicting_term = Term::try_from(conflicting_term_str.clone()).unwrap();

    db_manager.add_term_to_db(&conflicting_term).await;

    let conflicting_term_err_msg = format!(
        "Term {} should exist in the database.",
        conflicting_term_str
    );

    let query = r#"SELECT * FROM terms WHERE term = $1"#;

    // Adding the conflicting term should update the term in the database
    assert_eq!(
        Term::try_from(
            &sqlx::query(query)
                .bind(conflicting_term_str)
                .fetch_one(&pool)
                .await
                .expect(&conflicting_term_err_msg)
        )
        .unwrap(),
        conflicting_term
    );
}
