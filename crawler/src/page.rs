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

#[allow(unused)]
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
