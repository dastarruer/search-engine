use crate::page::{Page, PageQueue};

trait DbManager {
    async fn init_queue(&self, starting_pages: Vec<Page>) -> PageQueue;
}

pub struct RealDbManager {
    pool: sqlx::PgPool,
}

impl RealDbManager {
    fn new(pool: sqlx::PgPool) -> Self {
        RealDbManager { pool }
    }
}

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
