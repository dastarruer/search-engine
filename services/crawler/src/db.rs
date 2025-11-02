use reqwest::Url;
use sqlx::Row;
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

use async_trait::async_trait;

use crate::{page::{CrawledPage, Page, PageQueue}, QUEUE_LIMIT};

// Remove Send trait bound, since CrawledPage cannot implement Send
#[async_trait(?Send)]
pub trait DbManager {
    async fn init_queue(self: Arc<Self>, starting_pages: Vec<Page>) -> PageQueue;
    async fn init_crawled(self: Arc<Self>) -> HashSet<Page>;

    async fn add_page_to_db(&self, page: &Page);
    async fn add_crawled_page_to_db(&self, page: &CrawledPage);

    async fn fetch_pages_from_db(&self) -> (VecDeque<Page>, HashSet<Page>);
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

        queue.refresh_queue(self.clone()).await;

        // Queue each page in starting_pages
        for page in starting_pages {
            queue.queue_page(page, self.clone()).await;
        }

        queue
    }

    /// Initialize the hashset of visited [`Page`]'s and the Postgres pool.
    /// Will return an empty hashset if the database is empty.
    async fn init_crawled(self: Arc<Self>) -> HashSet<Page> {
        let visited_query = format!(
            "SELECT * FROM pages WHERE is_crawled = TRUE LIMIT {}",
            QUEUE_LIMIT
        );
        let mut visited = HashSet::new();

        let query = sqlx::query(visited_query.as_str());

        let rows = (query.fetch_all(&self.pool).await).ok();

        if rows.is_none() {
            return visited;
        }

        rows.expect("`rows` var can only be `Some`.")
            .iter()
            .for_each(|row| {
                let url = row.get("url");

                let err_msg = format!(
                    "URL retrieved from the database should always be valid: {}",
                    url
                );
                let url = Url::parse(url).expect(&err_msg);
                let page = Page::from(url);

                visited.insert(page);
            });

        visited
    }

    /// Add a [`Page`] instance to a database.
    async fn add_page_to_db(&self, page: &Page) {
        let query = r#"
            INSERT INTO pages (url, is_crawled, is_indexed)
            VALUES ($1, FALSE, FALSE)
            ON CONFLICT (url) DO NOTHING"#;

        // Usually this will throw an error if the url is too large to store in
        // the db. However, a large url usually redirects to somewhere else, so we
        // can just ignore this error.
        let _ = sqlx::query(query)
            .bind(page.url.to_string())
            .execute(&self.pool)
            .await;
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

        // Usually this will throw an error if the url is too large to store in
        // the db. However, a large url usually redirects to somewhere else, so we
        // can just ignore this error.
        let _ = sqlx::query(query)
            .bind(page.html.html())
            .bind(page.title.clone())
            .bind(page.url.to_string())
            .execute(&self.pool)
            .await;
    }

    async fn fetch_pages_from_db(&self) -> (VecDeque<Page>, HashSet<Page>) {
        let mut queue = VecDeque::new();
        let mut hashset = HashSet::new();

        let query = format!(
            r#"
            SELECT url
            FROM pages
            WHERE is_crawled = FALSE
            LIMIT {};"#,
            QUEUE_LIMIT
        );
        let query = query.as_str();

        sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .expect("Fetching rows from the database should not throw an error.")
            .iter()
            .for_each(|row| {
                let url: String = row.get("url");
                let err_msg = format!("Url {} should be a valid url.", url);
                let page = Page::new(Url::parse(url.as_str()).expect(&err_msg));

                queue.push_back(page.clone());
                hashset.insert(page);
            });

        (queue, hashset)
    }
}

#[cfg(any(test, feature = "bench-utils"))]
pub struct MockDbManager {}

#[cfg(any(test, feature = "bench-utils"))]
impl MockDbManager {
    pub fn new() -> Self {
        MockDbManager {}
    }
}

#[cfg(any(test, feature = "bench-utils"))]
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

    async fn init_crawled(self: Arc<Self>) -> HashSet<Page> {
        HashSet::new()
    }

    async fn add_page_to_db(&self, _page: &Page) {
        // Do nothing
    }

    async fn add_crawled_page_to_db(&self, _page: &CrawledPage) {
        // Do nothing
    }

    async fn fetch_pages_from_db(&self) -> (VecDeque<Page>, HashSet<Page>) {
        (VecDeque::new(), HashSet::new())
    }
}
