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

    /// Add a [`Page`] to the database.
    ///
    /// # Errors
    /// This function returns an error if:
    /// - The [`Page`] is already in the database.
    pub async fn add_to_db(&self, pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let query =
            "INSERT INTO public.pages (url, is_crawled, is_indexed) VALUES ($1, FALSE, FALSE)";

        sqlx::query(query)
            .bind(self.url.to_string())
            .execute(pool)
            .await?;

        Ok(())
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

    /// Update the database entry for this [`CrawledPage`].
    ///
    /// This will update the row in the `pages` table that matches the
    /// [`CrawledPage`]'s URL, setting its `html`, `title`, and marking
    /// `is_crawled` as `TRUE`.
    ///
    /// # Errors
    /// This function returns an error if:
    /// - The [`CrawledPage`] is already in the database.
    pub async fn add_to_db(&self, pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let query = r#"
            UPDATE public.pages
                SET html = $1,
                title = $2,
                is_crawled = TRUE
            WHERE url = $3"#;

        sqlx::query(query)
            .bind(self.title.clone())
            .bind(self.html.as_str())
            .bind(self.url.to_string())
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

    /// Pushes a [`Page`] into the [`PageQueue`], and adds it to the database.
    ///
    /// # Errors
    /// This function returns an error if:
    /// - The [`Page`] is already in the database.
    pub async fn queue_page(
        &mut self,
        page: Page,
        pool: &sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        page.add_to_db(pool).await?;

        self.queue.push_back(page.clone());
        self.hashset.insert(page);

        Ok(())
    }

    /// Pushes a [`Page`] into the [`PageQueue`].
    ///
    /// # Note
    /// Even though this is public, this method is meant to be used for benchmarks and tests only.
    pub fn queue_page_test(&mut self, page: Page) {
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
