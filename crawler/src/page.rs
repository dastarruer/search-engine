use url::Url;

#[derive(PartialEq, Debug)]
pub struct Page {
    pub url: Url,
}

#[derive(PartialEq)]
pub struct CrawledPage {
    pub url: Url,
    pub html: String,
}

impl Page {
    pub fn new(url: Url) -> Self {
        Page { url }
    }

    /// 'Crawl' a Page, which turns it into a CrawledPage
    pub(crate) fn into_crawled(self) -> CrawledPage {
        CrawledPage::new(self)
    }
}

impl Clone for Page {
    fn clone(&self) -> Self {
        Page {
            url: self.url.clone(),
        }
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
    pub fn new(page: Page) -> Self {
        CrawledPage {
            url: page.url,
            html: String::new(),
        }
    }
}

impl PartialEq<Page> for CrawledPage {
    fn eq(&self, other: &Page) -> bool {
        self.url == other.url
    }
}
