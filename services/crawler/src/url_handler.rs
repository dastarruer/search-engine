use rustrict::Censor;
use scraper::{Html, Selector};
use url::Url;
use utils::ExtractText;

use crate::{error::Error, page::Page};

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
        let selector =
            Selector::parse("html").expect("Parsing `html` selector should not throw an error.");

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

    /// Normalize a url by stripping any passive parameters that do not change
    /// the page content.
    ///
    /// Also strips fragment identifiers (e.g. `https://example.com/data.csv#row=4`
    /// is normalized as `https://example.com/data.csv`), since these usually
    /// do not change page content.
    // Use Box<Error> since the variant returned in this method is too large
    pub fn normalize_url(url: Url) -> Result<Url, Box<Error>> {
        // If the url does not have any parameters or fragment, it is
        // already normalized
        if let None = url.query()
            && let None = url.fragment()
        {
            return Ok(url);
        }

        let domain = url.domain();

        let domain = if let Some(domain) = domain {
            domain
        } else {
            return Err(Box::new(Error::InvalidDomain(url)));
        };

        let path = url.path();
        let params: Vec<_> = url
            .query_pairs()
            .filter(|(query, _)| !Self::query_is_passive(query))
            .collect();

        let mut url = if !params.is_empty() {
            Url::parse_with_params(format!("https://{}{}", domain, path).as_str(), params)
                .expect("Normalized URL must be a valid url.")
        } else {
            Url::parse(format!("https://{}{}", domain, path).as_str())
                .expect("Normalized URL must be a valid url.")
        };

        // Strip the fragment identifier
        url.set_fragment(None);

        Ok(url)
    }

    fn query_is_passive(query: &str) -> bool {
        query.contains("utm") || query == "id" || query == "t"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    mod normalize_url {
        use url::Url;

        use crate::url_handler::UrlHandler;

        #[test]
        fn test_url_with_no_params() {
            let url = Url::parse("https://safe.com").unwrap();

            assert_eq!(
                UrlHandler::normalize_url(url.clone()).unwrap().as_str(),
                url.as_str()
            );
        }

        #[test]
        fn test_url_with_active_params() {
            let url = Url::parse("https://safe.com?filter=automatic&rating=5").unwrap();

            assert_eq!(
                UrlHandler::normalize_url(url.clone()).unwrap().as_str(),
                url.as_str()
            );
        }

        #[test]
        fn test_url_with_passive_params() {
            let url =
                Url::parse("https://safe.com?utm_source=newsletter&id=seranking&t=60s").unwrap();

            assert_eq!(
                UrlHandler::normalize_url(url.clone()).unwrap().as_str(),
                Url::parse("https://safe.com").unwrap().as_str()
            );
        }

        #[test]
        fn test_url_with_fragment() {
            let url = Url::parse("https://safe.com#Header").unwrap();

            assert_eq!(
                UrlHandler::normalize_url(url.clone()).unwrap().as_str(),
                Url::parse("https://safe.com").unwrap().as_str()
            );
        }

        #[test]
        fn test_url_with_fragment_and_params() {
            let url = Url::parse("https://safe.com?utm_source=newsletter&rating=5#Header").unwrap();

            assert_eq!(
                UrlHandler::normalize_url(url.clone()).unwrap().as_str(),
                Url::parse("https://safe.com?rating=5").unwrap().as_str()
            );
        }
    }

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

        use crate::{page::Page, url_handler::UrlHandler, utils::test_file_path_from_filepath};

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
