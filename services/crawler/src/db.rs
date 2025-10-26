use async_trait::async_trait;

use crate::page::{Page, PageQueue};

#[async_trait]
pub trait DbManager: Send + Sync + 'static {
    async fn init_queue(&self, starting_pages: Vec<Page>) -> PageQueue;
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
    async fn init_queue(&self, starting_pages: Vec<Page>) -> PageQueue {
        let mut queue = PageQueue::default();

        queue.refresh_queue(&self.pool).await;

        // Queue each page in starting_pages
        for page in starting_pages {
            queue.queue_page(page, Some(&self.pool)).await;
        }

        queue
    }
}

// #[cfg(test)]
pub struct MockDbManager {}

// #[cfg(test)]
impl MockDbManager {
    pub fn new() -> Self {
        MockDbManager {}
    }
}

// #[cfg(test)]
#[async_trait]
impl DbManager for MockDbManager {
    async fn init_queue(&self, starting_pages: Vec<Page>) -> PageQueue {
        let mut queue = PageQueue::default();

        // Queue each page in starting_pages
        for page in starting_pages {
            queue.queue_page(page, None).await;
        }

        queue
    }
}
