use reqwest::Url;
use scraper::Html;
use sqlx::Row;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use utils::AddToDb;
use utils::QUEUE_LIMIT;

use crate::db::DbManager;

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
    pub async fn queue_page(&mut self, page: Page, db_manager: Arc<dyn DbManager>) {
        // First add the page to the database
        db_manager.add_page_to_db(&page).await;

        // Then store the page in memory
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
    pub async fn pop(&mut self, db_manager: Arc<dyn DbManager>) -> Option<Page> {
        if let Some(page) = self.queue.front() {
            self.hashset.remove(page);
            self.queue.pop_front()
        } else {
            // TODO: Move this logic out of the pop method
            log::info!("Queue is empty, refreshing...");
            self.refresh_queue(db_manager).await;

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
    pub async fn refresh_queue(&mut self, db_manager: Arc<dyn DbManager>) {
        let (queue, hashset) = db_manager.fetch_pages_from_db().await;

        self.queue.extend(queue);
        self.hashset.extend(hashset);
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
