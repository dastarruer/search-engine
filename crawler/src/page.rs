use url::Url;

#[allow(unused)]
#[derive(PartialEq, Debug)]
pub struct Page {
    pub url: Url,
}

#[allow(unused)]
#[derive(PartialEq)]
pub struct CrawledPage {
    url: Url,
    html: String,
}

#[allow(unused)]
impl Page {
    pub fn new(url: Url) -> Self {
        Page { url }
    }

    /// 'Crawl' a Page, which turns it into a CrawledPage
    pub(crate) fn crawl(self) -> CrawledPage {
        CrawledPage::new(self)
    }
}

#[allow(unused)]
impl CrawledPage {
    pub fn new(page: Page) -> Self {
        CrawledPage { url: page.url, html: String::new() }
    }
}
