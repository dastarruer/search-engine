use rustrict::Censor;
use scraper::{Html, Selector};
use utils::ExtractText;

use crate::page::Page;

static BLOCKED_KEYWORDS: once_cell::sync::Lazy<rustrict::Trie> = once_cell::sync::Lazy::new(|| {
    let mut trie = rustrict::Trie::default();

    // add a certain... domain that's been giving me trouble...
    trie.set("xvideos", rustrict::Type::SEXUAL);
    // trie.set("SpongeBob", Type::NONE);
    trie
});

#[derive(Clone)]
pub struct UrlHandler {
    blocked_keywords: &'static once_cell::sync::Lazy<rustrict::Trie>,
}

impl UrlHandler {
    pub fn new() -> UrlHandler {
        UrlHandler {
            blocked_keywords: &BLOCKED_KEYWORDS,
        }
    }

    pub fn is_english(html: &Html) -> bool {
        let selector = Selector::parse("html").unwrap();

        for element in html.select(&selector) {
            if let Some(lang) = element.value().attr("lang")
                && lang.starts_with("en")
            {
                return true;
            }
        }
        false
    }

    /// Checks URL domain against a list of blocked keywords relating to inappropriate content.
    pub fn is_inappropriate_page(&self, page: &Page, html: &Html) -> bool {
        let mut domain = Censor::from_str(page.url.as_str());
        domain.with_trie(self.blocked_keywords);

        // First check that the domain is appropriate
        if Self::is_severity_inappropriate(domain.analyze()) {
            return true;
        }

        let content = html.extract_text();

        let mut content = Censor::from_str(content.as_str());
        content.with_trie(self.blocked_keywords);

        // Then check if the content is appropriate
        Self::is_severity_inappropriate(content.analyze())
    }

    /// Checks that the severity of something is at a high enough threshold to
    /// be considered inappropriate while also minimizing false positives and
    /// negatives.
    fn is_severity_inappropriate(severity: rustrict::Type) -> bool {
        // `Type::SEVERE` is a high enough threshold to prevent a majority of
        // false positives
        severity.is(rustrict::Type::SEVERE)
    }
}

#[cfg(test)]
mod test {
    mod is_english {
        use crate::{url_handler::UrlHandler, utils::create_crawler};
        use scraper::Html;

        #[tokio::test]
        async fn test_non_english_page() {
            let (crawler, page) = create_crawler("non_english_page.html").await;

            let html = crawler.extract_html_from_page(page).await.unwrap();
            let html = Html::parse_document(html.as_str());

            assert!(!UrlHandler::is_english(&html));
        }
    }

    mod is_inappropriate_page {
        use std::fs;

        use reqwest::Url;
        use scraper::Html;

        use crate::{url_handler::UrlHandler, page::Page, utils::test_file_path_from_filepath};

        #[test]
        fn test_inappropriate_page_url() {
            // a common... site that keeps getting crawled
            let page = Page::from(Url::parse("https://xvideos.com").unwrap());

            let url_handler = UrlHandler::new();

            assert!(url_handler.is_inappropriate_page(&page, &Html::new_document()));
        }

        #[test]
        fn test_inappropriate_page_content() {
            let html = Html::parse_document(
                r#"
            <body>
                <p>porn hippopotamus hippopotamus</p>
            </body>"#,
            );

            let page = Page::from(Url::parse("https://a-very-innocent-site.com").unwrap());

            let url_handler = UrlHandler::new();

            assert!(url_handler.is_inappropriate_page(&page, &html));
        }

        #[test]
        fn test_appropriate_page_content() {
            let filepath = test_file_path_from_filepath("appropriate-site.html");
            let filepath = filepath.to_str().unwrap();
            let html = fs::read_to_string(filepath).unwrap();
            let html = Html::parse_document(&html);

            let page = Page::from(
                Url::parse("https://spongebob.fandom.com/wiki/Hog_Huntin%27#References").unwrap(),
            );

            let url_handler = UrlHandler::new();

            assert!(!url_handler.is_inappropriate_page(&page, &html));
        }

        #[tokio::test]
        async fn test_appropriate_page_url() {
            let page = Page::from(Url::parse("https://safe.com").unwrap());

            let url_handler = UrlHandler::new();

            assert!(!url_handler.is_inappropriate_page(
                &page,
                &Html::parse_document(
                    r#"
                <body></body>"#
                )
            ));
        }
    }
}
