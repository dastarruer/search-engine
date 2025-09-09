use url::Url;

#[derive(PartialEq)]
pub struct Page {
    pub url: Url,
}

pub struct CrawledPage {
    url: Url,
}

impl Page {
    pub fn new(url: Url) -> Self {
        Page { url }
    }
}
