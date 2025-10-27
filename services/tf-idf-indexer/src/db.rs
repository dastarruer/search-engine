use async_trait::async_trait;

use crate::Term;

#[async_trait]
pub trait DbManager {
    async fn add_term_to_db(&self, term: &Term);
}

pub struct RealDbManager {
    pool: sqlx::PgPool,
}

impl RealDbManager {
    pub fn new(pool: sqlx::PgPool) -> Self {
        RealDbManager { pool }
    }
}

#[async_trait]
impl DbManager for RealDbManager {
    /// Add a [`Term`] instance to a database.
    ///
    /// If the term already exists in the database, then update the existing
    /// term with the values from the new term.
    async fn add_term_to_db(&self, term: &Term) {
        // This query tries to insert the term and its values into a new row.
        // But if the term already exists, then it updates the existing term's
        // values instead.
        let query = r#"
            INSERT INTO terms (term, idf, page_frequency, tf_scores, tf_idf_scores)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (term)
            DO UPDATE SET
                idf = EXCLUDED.idf,
                page_frequency = EXCLUDED.page_frequency,
                tf_scores = EXCLUDED.tf_scores,
                tf_idf_scores = EXCLUDED.tf_idf_scores
        "#;

        sqlx::query(query)
            .bind(&term.term)
            .bind(*term.idf) // Dereferencing gives the inner f32 value
            .bind(term.page_frequency as i32)
            .bind(&term.tf_scores)
            .bind(&term.tf_idf_scores)
            .execute(&self.pool)
            .await
            .unwrap();
    }
}

#[cfg(test)]
pub struct MockDbManager {
}

#[cfg(test)]
impl MockDbManager {
    pub fn new() -> Self {
        MockDbManager {  }
    }
}

#[cfg(test)]
#[async_trait]
impl DbManager for MockDbManager {
    async fn add_term_to_db(&self, _term: &Term) {
        // Do nothing
    }
}
