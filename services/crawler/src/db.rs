use reqwest::Url;
use sqlx::Row;
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};
use utils::QUEUE_LIMIT;

use async_trait::async_trait;

use crate::page::{CrawledPage, Page, PageQueue};

// Remove Send trait bound, since CrawledPage cannot implement Send
#[async_trait(?Send)]
pub trait DbManager {
    async fn init_queue(self: Arc<Self>, starting_pages: Vec<Page>) -> PageQueue;
    async fn add_page_to_db(&self, page: &Page);
    async fn add_crawled_page_to_db(&self, page: &CrawledPage);
}

pub struct RealDbManager {
    pool: sqlx::PgPool,
}

impl RealDbManager {
    pub fn new(pool: sqlx::PgPool) -> Self {
        RealDbManager { pool }
    }
}

#[async_trait(?Send)]
impl DbManager for RealDbManager {
    async fn init_queue(self: Arc<Self>, starting_pages: Vec<Page>) -> PageQueue {
        let mut queue = PageQueue::default();

        queue.refresh_queue(&self.pool).await;

        // Queue each page in starting_pages
        for page in starting_pages {
            queue.queue_page(page, self.clone()).await;
        }

        queue
    }

    async fn add_page_to_db(&self, page: &Page) {
        let query = r#"
            INSERT INTO pages (url, is_crawled, is_indexed)
            VALUES ($1, FALSE, FALSE)
            ON CONFLICT (url) DO NOTHING"#;

        sqlx::query(query)
            .bind(page.url.to_string())
            .execute(&self.pool)
            .await
            .unwrap();
    }

    /// Update the database entry for this [`CrawledPage`].
    ///
    /// This will update the row in the `pages` table that matches the
    /// [`CrawledPage`]'s URL, setting its `html`, `title`, and marking
    /// `is_crawled` as `TRUE`.
    async fn add_crawled_page_to_db(&self, page: &CrawledPage) {
        let query = r#"
            UPDATE pages
            SET html = $1,
                title = $2,
                is_crawled = TRUE
            WHERE url = $3"#;

        sqlx::query(query)
            .bind(page.html.html())
            .bind(page.title.clone())
            .bind(page.url.to_string())
            .execute(&self.pool)
            .await
            .unwrap();
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
#[async_trait(?Send)]
impl DbManager for MockDbManager {
    async fn init_queue(self: Arc<Self>, starting_pages: Vec<Page>) -> PageQueue {
        let mut queue = PageQueue::default();

        // Queue each page in starting_pages
        for page in starting_pages {
            queue.queue_page(page, self.clone()).await;
        }

        queue
    }

    async fn add_page_to_db(&self, _page: &Page) {
        // Do nothing
    }

    async fn add_crawled_page_to_db(&self, _page: &CrawledPage) {
        // Do nothing
    }
}
