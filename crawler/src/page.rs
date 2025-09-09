use url::Url;

#[derive(PartialEq)]
#[allow(unused)]
pub struct Page {
    url: Url,
}

#[allow(unused)]
pub struct CrawledPage {
    url: Url,
}

#[allow(unused)]
impl Page {
    pub fn new(url: Url) -> Self {
        Page { url }
    }
}

#[allow(unused)]
impl CrawledPage {
    pub fn new(page: Page) -> Self {
        CrawledPage { url: page.url }
    }
}
