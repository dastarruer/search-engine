use async_trait::async_trait;

use crate::Term;

#[async_trait]
pub trait DbManager {
    async fn add_term_to_db(&self, term: Term);
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
    async fn add_term_to_db(&self, term: Term) {}
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
    async fn add_term_to_db(&self, _term: Term) {
        // Do nothing
    }
}
