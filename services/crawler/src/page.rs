use reqwest::Url;
use scraper::Html;
use sqlx::Row;
use std::collections::{HashSet, VecDeque};
use utils::AddToDb;
use utils::QUEUE_LIMIT;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Page {
    pub url: Url,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CrawledPage {
    pub url: Url,
    pub title: Option<String>,
    pub html: Html,
}

impl std::hash::Hash for CrawledPage {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // The url will always be unique, so hash this
        self.url.hash(state);
    }
}

impl Page {
    pub fn new(url: Url) -> Self {
        Page { url }
    }

    /// 'Crawl' a Page, which turns it into a [`CrawledPage`].
    pub(crate) fn into_crawled(self, title: Option<String>, html: Html) -> CrawledPage {
        CrawledPage::new(self, title, html)
    }
}

impl AddToDb for Page {
    /// Add a [`Page`] instance to a database.
    async fn add_to_db(&self, pool: &sqlx::PgPool) {
        let query = r#"
            INSERT INTO pages (url, is_crawled, is_indexed)
            VALUES ($1, FALSE, FALSE)
            ON CONFLICT (url) DO NOTHING"#;

        sqlx::query(query)
            .bind(self.url.to_string())
            .execute(pool)
            .await
            .unwrap();
    }
}

impl From<Url> for Page {
    fn from(value: Url) -> Self {
        Page { url: value }
    }
}

impl PartialEq<CrawledPage> for Page {
    fn eq(&self, other: &CrawledPage) -> bool {
        self.url == other.url
    }
}

impl CrawledPage {
    pub fn new(page: Page, title: Option<String>, html: Html) -> Self {
        CrawledPage {
            url: page.url,
            title,
            html,
        }
    }
}

impl AddToDb for CrawledPage {
    /// Update the database entry for this [`CrawledPage`].
    ///
    /// This will update the row in the `pages` table that matches the
    /// [`CrawledPage`]'s URL, setting its `html`, `title`, and marking
    /// `is_crawled` as `TRUE`.
    async fn add_to_db(&self, pool: &sqlx::PgPool) {
        let query = r#"
            UPDATE pages
            SET html = $1,
                title = $2,
                is_crawled = TRUE
            WHERE url = $3"#;

        sqlx::query(query)
            .bind(self.html.html())
            .bind(self.title.clone())
            .bind(self.url.to_string())
            .execute(pool)
            .await
            .unwrap();
    }
}
impl PartialEq<Page> for CrawledPage {
    fn eq(&self, other: &Page) -> bool {
        self.url == other.url
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageQueue {
    queue: VecDeque<Page>,
    hashset: HashSet<Page>,
}

impl Default for PageQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PageQueue {
    pub fn new() -> Self {
        let queue = VecDeque::new();
        let hashset = HashSet::new();

        PageQueue { queue, hashset }
    }

    /// Adds a queued [`Page`] to the database.
    pub async fn queue_page(&mut self, page: Page, pool: Option<&sqlx::PgPool>) {
        if let Some(pool) = pool {
            page.add_to_db(pool).await;
        } else {
            self.queue.push_back(page.clone());
            self.hashset.insert(page);
        }
    }

    /// Pushes a [`Page`] into the [`PageQueue`].
    ///
    /// # Note
    /// Even though this is public, this method is meant to be used for benchmarks and tests only.
    pub fn queue_page_test(&mut self, page: Page) {
        self.queue.push_back(page.clone());
        self.hashset.insert(page);
    }

    /// Pop a queued [`Page`] from the [`PageQueue`].
    ///
    /// If the queue is empty, refreshes the queue by querying the database for
    /// uncrawled pages.
    ///
    /// # Returns
    /// - Returns `Some(Page)` if the queue is not empty, or there are still
    ///   uncrawled pages in the database.
    /// - Returns `None` if the database has no more uncrawled pages left.
    pub async fn pop(&mut self, pool: Option<&sqlx::PgPool>) -> Option<Page> {
        if let Some(page) = self.queue.front() {
            self.hashset.remove(page);
            self.queue.pop_front()
        } else {
            log::info!("Queue is empty, refreshing...");
            if let Some(pool) = pool {
                self.refresh_queue(pool).await;
            } else {
                return None;
            }

            if let Some(page) = self.queue.front() {
                self.hashset.remove(page);
                self.queue.pop_front()
            } else {
                None
            }
        }
    }

    /// Add as many uncrawled [`Page`]s from the database to the queue as is defined by [`utils::QUEUE_LIMIT`].
    ///
    /// Should be called whenever the queue is empty and needs more pages.
    pub async fn refresh_queue(&mut self, pool: &sqlx::PgPool) {
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
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .for_each(|row| {
                let url: String = row.get("url");
                let err_msg = format!("Url {} should be a valid url.", url);
                let page = Page::new(Url::parse(url.as_str()).expect(&err_msg));

                self.queue.push_back(page.clone());
                self.hashset.insert(page);
            });
    }

    pub fn contains_page(&self, page: &Page) -> bool {
        self.hashset.contains(page)
    }
}

impl PartialEq<VecDeque<Page>> for PageQueue {
    fn eq(&self, other: &VecDeque<Page>) -> bool {
        self.queue == *other
    }
}
