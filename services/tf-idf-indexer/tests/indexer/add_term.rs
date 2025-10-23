use crate::common;
use tf_idf_indexer::Term;
use utils::AddToDb;

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
