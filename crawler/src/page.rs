use url::Url;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Page {
    pub url: Url,
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct CrawledPage {
    pub url: Url,
    pub html: String,
    pub http_status: i16,
}

impl Page {
    pub fn new(url: Url) -> Self {
        Page { url }
    }

    /// 'Crawl' a Page, which turns it into a CrawledPage
    pub(crate) fn into_crawled(self, http_status: i16) -> CrawledPage {
        CrawledPage::new(self, http_status)
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
    pub fn new(page: Page, http_status: i16) -> Self {
        CrawledPage {
            url: page.url,
            html: String::new(),
            http_status,
        }
    }

    pub async fn add_to_db(&self, pool: &sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
        let query =
            "INSERT INTO public.pages (url, html, is_indexed, http_status) VALUES ($1, $2, $3, $4)";

        sqlx::query(query)
            .bind(self.url.to_string())
            .bind(self.html.as_str())
            .bind(false)
            .bind(self.http_status)
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
