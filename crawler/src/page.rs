use url::Url;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Page {
    pub url: Url,
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct CrawledPage {
    pub url: Url,

    // This is a `String` instead of `Html` because `Html` does not implement the `sqlx::Encode` trait
    pub html: String,
}

impl Page {
    pub fn new(url: Url) -> Self {
        Page { url }
    }

    /// 'Crawl' a Page, which turns it into a CrawledPage
    pub(crate) fn into_crawled(self, html: String) -> CrawledPage {
        CrawledPage::new(self, html)
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
    pub fn new(page: Page, html: String) -> Self {
        CrawledPage {
            url: page.url,
            html,
        }
    }

    /// Add a `CrawledPage` to the database.
    /// # Returns
    /// - Returns `Err` if the `CrawledPage` is already in the database.
    pub async fn add_to_db(&self, pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let query =
            "INSERT INTO public.pages (url, html, is_indexed) VALUES ($1, $2, $3)";

        sqlx::query(query)
            .bind(self.url.to_string())
            .bind(self.html.as_str())
            .bind(false)
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
