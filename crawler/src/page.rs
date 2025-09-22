use std::collections::{HashSet, VecDeque};
use url::Url;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Page {
    pub url: Url,
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct CrawledPage {
    pub url: Url,
    pub title: Option<String>,

    // This is a `String` instead of `Html` because `Html` does not implement the `sqlx::Encode` trait
    pub html: String,
}

impl Page {
    pub fn new(url: Url) -> Self {
        Page { url }
    }

    /// 'Crawl' a Page, which turns it into a [`CrawledPage`].
    pub(crate) fn into_crawled(self, title: Option<String>, html: String) -> CrawledPage {
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
    pub fn new(page: Page, title: Option<String>, html: String) -> Self {
        CrawledPage {
            url: page.url,
            title,
            html,
        }
    }

    /// Add a [`CrawledPage`] to the database.
    ///
    /// # Errors
    /// This function returns an error if:
    /// - The [`CrawledPage`] is already in the database.
    pub async fn add_to_db(&self, pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let query =
            "INSERT INTO public.pages (url, html, is_indexed, title) VALUES ($1, $2, $3, $4)";

        sqlx::query(query)
            .bind(self.url.to_string())
            .bind(self.html.as_str())
            .bind(false)
            .bind(self.title.clone())
            .execute(pool)
            .await?;

        Ok(())
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

impl PageQueue {
    pub fn new() -> Self {
        let queue = VecDeque::new();
        let hashset = HashSet::new();

        PageQueue { queue, hashset }
    }

    pub fn push(&mut self, page: Page) {
        self.queue.push_back(page.clone());
        self.hashset.insert(page);
    }

    pub fn pop(&mut self) -> Option<Page> {
        let page = self.queue.front();

        if let Some(page) = page {
            self.hashset.remove(page);
            self.queue.pop_front()
        } else {
            None
        }
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
