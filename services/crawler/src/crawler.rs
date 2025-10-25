use std::{collections::HashSet, time::Duration};

use crate::{
    error::Error,
    page::{CrawledPage, Page, PageQueue},
    utils::string_to_url,
};
use once_cell::sync::Lazy;
use reqwest::{Client, ClientBuilder, StatusCode, Url, header::RETRY_AFTER};
use rustrict::{Censor, Type};
use scraper::{Html, Selector};
use sqlx::Row;

use utils::{AddToDb, ExtractText};

#[derive(Clone)]
pub struct Crawler {
    queue: PageQueue,
    // Use [`Page`] instead of `CrawledPage` because comparing [`Page`] with `CrawledPage` does not work in hashsets for some reason
    // TODO: Convert to CrawledPage
    crawled: HashSet<Page>,
    client: Client,
}

// Should this be a global variable? No, but I need static access to this and this is the easiest solution ok-
// TODO: Find a way to move this into Crawler struct
static BLOCKED_KEYWORDS: Lazy<rustrict::Trie> = Lazy::new(|| {
    let mut trie = rustrict::Trie::default();

    // add a certain... domain that's been giving me trouble...
    trie.set("xvideos", Type::SEXUAL);
    // trie.set("SpongeBob", Type::NONE);
    trie
});

impl Crawler {
    pub async fn new(starting_pages: Vec<Page>, pool: Option<&sqlx::PgPool>) -> Self {
        let crawled = if let Some(pool) = pool {
            Self::init_crawled(pool).await
        } else {
            HashSet::new()
        };

        let queue = Self::init_queue(starting_pages, pool).await;

        let client = Self::init_client();

        Crawler {
            queue,
            crawled,
            client,
        }
    }

    /// Run the main loop of the Crawler.
    ///
    /// # Returns
    /// - Returns `Ok` if no unrecoverable errors occur.
    /// - Returns `Err` if an untested fatal error happens.
    pub async fn run(&mut self, pool: Option<&sqlx::PgPool>) -> Result<(), Error> {
        while let Some(page) = self.next_page(pool).await {
            match self.crawl_page(page.clone(), pool).await {
                Ok(crawled_page) => {
                    if let Some(pool) = pool {
                        crawled_page.add_to_db(pool).await;
                    } else {
                        log::info!("Crawl successful.");
                    }
                }
                Err(e) => {
                    log::warn!("Crawl failed: {}", e);
                }
            }
        }

        log::info!("All done! no more pages left");
        Ok(())
    }

    /// Returns the next [`Page`] in the queue.
    ///
    /// # Returns
    /// - Return `Some(Page)` if a [`Page`] exists in the queue.
    /// - Returns `None` if the queue is empty.
    pub async fn next_page(&mut self, pool: Option<&sqlx::PgPool>) -> Option<Page> {
        self.queue.pop(pool).await
    }

    /// Crawl a single page.
    ///
    /// # Errors
    /// This function returns a [`CrawlerError`] if:
    /// - The [`Page`]'s HTML could not be fetched due to a fatal HTTP status code or a request timeout.
    /// - The [`Page`] is not in English.
    /// - [`Crawler::extract_html_from_page`] fails.
    pub async fn crawl_page(
        &mut self,
        page: Page,
        pool: Option<&sqlx::PgPool>,
    ) -> Result<CrawledPage, Error> {
        let html = self.extract_html_from_page(page.clone()).await?;

        let html = Html::parse_document(html.as_str());

        if !Self::is_english(&html) {
            return Err(Error::NonEnglishPage(page));
        }

        if Self::is_inappropriate_page(&page, &html) {
            return Err(Error::InappropriateSite(page));
        }

        let title = Self::extract_title_from_html(&html);
        let urls = self.extract_urls_from_html(&html);

        let base_url = page.url.clone();

        for url in urls {
            let url = string_to_url(&base_url, url);

            let page = if let Some(url) = url {
                Page::from(Self::normalize_url(url))
            } else {
                continue;
            };

            if self.crawled.contains(&page) || self.is_page_queued(&page) {
                log::warn!("{} is a duplicate", page.url);
                continue;
            }

            // Add the page to the queue of pages to crawl
            self.queue.queue_page(page.clone(), pool).await;

            log::info!("{} is queued", page.url);

            // Add the page to self.crawled, so that it is never crawled again
            self.crawled.insert(page);
        }

        log::info!("Crawled {:?}...", base_url);

        Ok(page.into_crawled(title, html.html()))
    }

    /// Normalize a url by stripping any passive parameters that do not change
    /// the page content.
    ///
    /// Also strips fragment identifiers (e.g. `https://example.com/data.csv#row=4`
    /// is normalized as `https://example.com/data.csv`), since these usually
    /// do not change page content.
    fn normalize_url(url: Url) -> Url {
        // If the url does not have any parameters or fragment, it is
        // already normalized
        if let None = url.query()
            && let None = url.fragment()
        {
            return url;
        }

        let domain = url.domain().expect("Url must have a valid domain.");
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

        url
    }

    fn query_is_passive(query: &str) -> bool {
        query.contains("utm") || query == "id" || query == "t"
    }

    fn extract_title_from_html(html: &Html) -> Option<String> {
        let selector = Selector::parse("title").unwrap();

        let element = html.select(&selector).next();

        element.map(|element| element.text().collect::<String>())
    }

    fn is_english(html: &Html) -> bool {
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

    /// Extracts the HTML from a [`Page`].
    ///
    /// # Errors
    /// This function returns a [`CrawlerError`] if:
    /// - The [`Page`] is empty (has no HTML).
    /// - The response contains a non-200 or non-429 HTTP status code, or the request times out.
    /// - Sending the request results in an error.
    /// - Decoding the HTML from the [`reqwest::Response`] throws an error, such as UTF-8 errors.
    async fn extract_html_from_page(&self, page: Page) -> Result<String, Error> {
        let mut resp = self.make_get_request(page.clone()).await?;

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let html = Self::extract_html_from_resp(resp).await?;

                if html.is_none() {
                    return Err(Error::EmptyPage(page));
                }

                Ok(html.unwrap())
            }
            StatusCode::TOO_MANY_REQUESTS => {
                const MAX_ATTEMPTS: u8 = 10;
                const MAX_DELAY: Duration = Duration::from_secs(60);

                let mut attempts = 0;

                let retry_after = resp.headers().get(RETRY_AFTER);

                if let Some(retry_after) = retry_after {
                    let delay_secs: Result<u64, _> = retry_after.to_str().unwrap().parse();

                    // If delay_secs is not a valid value in seconds
                    if delay_secs.is_err() {
                        return Err(Error::InvalidRetryByHeader {
                            page,
                            header: Some(retry_after.to_owned()),
                        });
                    }

                    let delay_secs = delay_secs.unwrap();

                    let delay = Duration::from_secs(delay_secs);

                    if delay > MAX_DELAY {
                        return Err(Error::RequestTimeout(page));
                    }

                    tokio::time::sleep(delay).await;

                    while attempts <= MAX_ATTEMPTS && resp.status() != StatusCode::OK {
                        resp = self.make_get_request(page.clone()).await?;
                        attempts += 1;
                    }

                    if resp.status() != StatusCode::OK {
                        return Err(Error::RequestTimeout(page));
                    }

                    let html = Self::extract_html_from_resp(resp).await?;

                    Ok(html.unwrap())
                } else {
                    // just give up. it's not worth it.
                    Err(Error::InvalidRetryByHeader { page, header: None })
                }
            }
            // just give up. it's not worth it.
            _ => Err(Error::MalformedHttpStatus { page, status }),
        }
    }

    /// Checks URL domain against a list of blocked keywords relating to inappropriate content.
    fn is_inappropriate_page(page: &Page, html: &Html) -> bool {
        let mut domain = Censor::from_str(page.url.as_str());
        domain.with_trie(&BLOCKED_KEYWORDS);

        // First check that the domain is appropriate
        if Self::is_severity_inappropriate(domain.analyze()) {
            return true;
        }

        let content = html.extract_text();

        let mut content = Censor::from_str(content.as_str());
        content.with_trie(&BLOCKED_KEYWORDS);

        // Then check if the content is appropriate
        Self::is_severity_inappropriate(content.analyze())
    }

    /// Checks that the severity of something is at a high enough threshold to
    /// be considered inappropriate while also minimizing false positives and
    /// negatives.
    fn is_severity_inappropriate(severity: rustrict::Type) -> bool {
        // `Type::SEVERE` is a high enough threshold to prevent a majority of
        // false positives
        severity.is(Type::SEVERE)
    }

    fn is_page_queued(&self, page: &Page) -> bool {
        self.queue.contains_page(page)
    }

    /// Make a get request to a specific URL, and return the [`reqwest::Response`].
    ///
    /// # Errors
    /// This function returns a [`CrawlerError`] if:
    /// - There was an error while sending the request.
    /// - The redirect loop was detected.
    /// - The redirect limit was exhausted.
    async fn make_get_request(&self, page: Page) -> Result<reqwest::Response, Error> {
        self.client
            .get(page.url.clone())
            .send()
            .await
            .map_err(|e| Error::FailedRequest {
                page,
                error_str: e.to_string(),
            })
    }

    fn extract_urls_from_html(&self, html: &Html) -> Vec<String> {
        let mut urls = vec![];

        let selector = Selector::parse("a").unwrap();

        for element in html.select(&selector) {
            if let Some(url) = element.value().attr("href") {
                urls.push(url.to_owned());
            };
        }

        urls
    }

    async fn init_queue(starting_pages: Vec<Page>, pool: Option<&sqlx::PgPool>) -> PageQueue {
        let mut queue = PageQueue::default();

        if let Some(pool) = pool {
            queue.refresh_queue(pool).await;
        }

        // Queue each page in starting_pages
        for page in starting_pages {
            queue.queue_page(page, pool).await;
        }

        queue
    }

    /// Initialize the hashset of visited [`Page`]'s and the Postgres pool.
    /// Will return an empty hashset if the database is empty.
    async fn init_crawled(pool: &sqlx::PgPool) -> HashSet<Page> {
        let visited_query = format!(
            "SELECT * FROM pages WHERE is_crawled = TRUE LIMIT {}",
            utils::QUEUE_LIMIT
        );
        let mut visited = HashSet::new();

        let query = sqlx::query(visited_query.as_str());

        let rows = (query.fetch_all(pool).await).ok();

        if rows.is_none() {
            return visited;
        }

        rows.unwrap().iter().for_each(|row| {
            let page = Page::from(Url::parse(row.get("url")).unwrap());

            visited.insert(page);
        });

        visited
    }

    fn init_client() -> Client {
        ClientBuilder::new()
            .user_agent(crate::USER_AGENT)
            // Reduce bandwidth usage; compliant with wikimedia's robot policy: https://wikitech.wikimedia.org/wiki/Robot_policy#Generally_applicable_rules
            .gzip(true)
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap()
    }

    /// Extracts the HTML from a [`reqwest::Response`].
    ///
    /// # Returns
    /// - Returns `None` if the [`reqwest::Response`] has no HTML content.
    /// - Returns `Err` if decoding the HTML from the [`reqwest::Response`] throws an error, such as UTF 8 errors.
    async fn extract_html_from_resp(resp: reqwest::Response) -> Result<Option<String>, Error> {
        let url = resp.url().clone();

        let html = resp.text().await.map_err(|e| Error::HtmlDecoding {
            url,
            error_str: e.to_string(),
        })?;

        if html.is_empty() {
            Ok(None)
        } else {
            Ok(Some(html))
        }
    }
}

#[cfg(test)]
mod test {
    use scraper::Html;
    use std::collections::HashSet;

    use crate::{
        crawler::Crawler,
        page::Page,
        utils::{HttpServer, test_file_path_from_filepath},
    };

    async fn create_crawler(starting_filename: &str) -> (Crawler, Page) {
        let filepath = test_file_path_from_filepath(starting_filename);
        let server = HttpServer::new_with_filepath(filepath);

        let page = Page::from(server.base_url());

        (Crawler::new(vec![page.clone()], None).await, page)
    }

    mod normalize_url {
        use super::*;
        use url::Url;

        #[test]
        fn test_url_with_no_params() {
            let url = Url::parse("https://safe.com").unwrap();

            assert_eq!(Crawler::normalize_url(url.clone()).as_str(), url.as_str());
        }

        #[test]
        fn test_url_with_active_params() {
            let url = Url::parse("https://safe.com?filter=automatic&rating=5").unwrap();

            assert_eq!(Crawler::normalize_url(url.clone()).as_str(), url.as_str());
        }

        #[test]
        fn test_url_with_passive_params() {
            let url =
                Url::parse("https://safe.com?utm_source=newsletter&id=seranking&t=60s").unwrap();

            assert_eq!(
                Crawler::normalize_url(url.clone()).as_str(),
                Url::parse("https://safe.com").unwrap().as_str()
            );
        }

        #[test]
        fn test_url_with_fragment() {
            let url = Url::parse("https://safe.com#Header").unwrap();

            assert_eq!(
                Crawler::normalize_url(url.clone()).as_str(),
                Url::parse("https://safe.com").unwrap().as_str()
            );
        }

        #[test]
        fn test_url_with_fragment_and_params() {
            let url = Url::parse("https://safe.com?utm_source=newsletter&rating=5#Header").unwrap();

            assert_eq!(
                Crawler::normalize_url(url.clone()).as_str(),
                Url::parse("https://safe.com?rating=5").unwrap().as_str()
            );
        }
    }

    mod is_english {
        use super::*;
        use scraper::Html;

        #[tokio::test]
        async fn test_non_english_page() {
            let (crawler, page) = create_crawler("non_english_page.html").await;

            let html = crawler.extract_html_from_page(page).await.unwrap();
            let html = Html::parse_document(html.as_str());

            assert!(!Crawler::is_english(&html));
        }
    }

    mod is_inappropriate_page {
        use std::fs;

        use reqwest::Url;
        use scraper::Html;

        use super::*;

        #[test]
        fn test_inappropriate_page_url() {
            // a common... site that keeps getting crawled
            let page = Page::from(Url::parse("https://xvideos.com").unwrap());
            assert!(Crawler::is_inappropriate_page(&page, &Html::new_document()));
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

            assert!(Crawler::is_inappropriate_page(&page, &html));
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

            assert!(!Crawler::is_inappropriate_page(&page, &html));
        }

        #[tokio::test]
        async fn test_appropriate_page_url() {
            let page = Page::from(Url::parse("https://safe.com").unwrap());
            assert!(!Crawler::is_inappropriate_page(
                &page,
                &Html::parse_document(
                    r#"
                <body></body>"#
                )
            ));
        }
    }

    mod extract_html_from_page {
        use super::*;
        use crate::error::Error;
        use httpmock::Method::GET;
        use reqwest::StatusCode;

        // Instead of using #[test], we use #[tokio::test] so we can test async functions
        #[tokio::test]
        async fn test_200_status() {
            let (crawler, page) = create_crawler("extract_single_href.html").await;

            let html = crawler.extract_html_from_page(page).await.unwrap();
            assert!(html.contains(r#"<a href="https://www.wikipedia.org/">This is a link.</a>"#));
        }

        #[tokio::test]
        async fn test_malformed_status() {
            // special setup (mock server), keep as is
            const EXPECTED_STATUS: StatusCode = StatusCode::NOT_FOUND;
            let filepath = test_file_path_from_filepath("extract_single_href.html");

            let server = HttpServer::new_with_mock(|when, then| {
                when.method(GET).header("user-agent", crate::USER_AGENT);
                then.status(EXPECTED_STATUS.as_u16())
                    .header("content-type", "text/html")
                    .body_from_file(filepath.display().to_string());
            });

            let page = Page::from(server.base_url());
            let crawler = Crawler::new(vec![page.clone()], None).await;

            let error = crawler
                .extract_html_from_page(page.clone())
                .await
                .unwrap_err();

            assert_eq!(
                error,
                Error::MalformedHttpStatus {
                    page,
                    status: EXPECTED_STATUS
                }
            )
        }

        #[tokio::test]
        async fn test_empty_page() {
            let (crawler, page) = create_crawler("empty.html").await;

            let error = crawler
                .extract_html_from_page(page.clone())
                .await
                .unwrap_err();
            assert_eq!(error, Error::EmptyPage(page))
        }

        mod status_429 {
            use crate::{
                crawler::Crawler,
                error::Error,
                page::Page,
                utils::{HttpServer, test_file_path_from_filepath},
            };
            use httpmock::Method::GET;
            use reqwest::StatusCode;
            use tokio::time::Instant;

            #[tokio::test]
            async fn test_429_status() {
                // special setup (retry-after), keep as is
                const TRY_AFTER_SECS: u64 = 1;
                let filepath = test_file_path_from_filepath("extract_single_href.html");

                let server = HttpServer::new_with_mock(|when, then| {
                    when.method(GET).header("user-agent", crate::USER_AGENT);
                    then.status(StatusCode::TOO_MANY_REQUESTS.as_u16())
                        .header("content-type", "text/html")
                        .header("retry-after", TRY_AFTER_SECS.to_string())
                        .body_from_file(filepath.display().to_string());
                });

                let page = Page::from(server.base_url());
                let crawler = Crawler::new(vec![page.clone()], None).await;

                let start = Instant::now();
                let error = crawler
                    .extract_html_from_page(page.clone())
                    .await
                    .unwrap_err();
                let end = Instant::now();

                let elapsed = (end - start).as_secs();
                assert_eq!(elapsed, TRY_AFTER_SECS);
                assert_eq!(error, Error::RequestTimeout(page))
            }

            #[tokio::test]
            async fn test_429_status_with_large_retry_after() {
                // special setup (retry-after too long), keep as is
                const TRY_AFTER_SECS: u64 = 61;
                let filepath = test_file_path_from_filepath("extract_single_href.html");

                let server = HttpServer::new_with_mock(|when, then| {
                    when.method(GET).header("user-agent", crate::USER_AGENT);
                    then.status(StatusCode::TOO_MANY_REQUESTS.as_u16())
                        .header("content-type", "text/html")
                        .header("retry-after", TRY_AFTER_SECS.to_string())
                        .body_from_file(filepath.display().to_string());
                });

                let page = Page::from(server.base_url());
                let crawler = Crawler::new(vec![page.clone()], None).await;

                let error = crawler
                    .extract_html_from_page(page.clone())
                    .await
                    .unwrap_err();
                assert_eq!(error, Error::RequestTimeout(page))
            }

            #[tokio::test]
            async fn test_429_status_with_no_header() {
                // special setup (missing retry-after header), keep as is
                let filepath = test_file_path_from_filepath("extract_single_href.html");

                let server = HttpServer::new_with_mock(|when, then| {
                    when.method(GET).header("user-agent", crate::USER_AGENT);
                    then.status(429)
                        .header("content-type", "text/html")
                        .body_from_file(filepath.display().to_string());
                });

                let page = Page::from(server.base_url());
                let crawler = Crawler::new(vec![page.clone()], None).await;

                let error = crawler
                    .extract_html_from_page(page.clone())
                    .await
                    .unwrap_err();

                assert_eq!(error, Error::InvalidRetryByHeader { page, header: None })
            }
        }
    }

    mod extract_title_from_html {
        use super::*;
        use scraper::Html;

        #[tokio::test]
        async fn test_page_with_title() {
            let (crawler, page) = create_crawler("page_with_title.html").await;

            let html =
                Html::parse_document(crawler.extract_html_from_page(page).await.unwrap().as_str());
            let title = Crawler::extract_title_from_html(&html).unwrap();

            assert!(title.contains("a page with a title"))
        }

        #[tokio::test]
        async fn test_page_without_title() {
            let (crawler, page) = create_crawler("non_english_page.html").await;

            let html =
                Html::parse_document(crawler.extract_html_from_page(page).await.unwrap().as_str());
            let title = Crawler::extract_title_from_html(&html);

            assert!(title.is_none())
        }
    }

    mod crawl_next_url {
        use super::*;
        use reqwest::Url;
        use std::collections::VecDeque;

        #[tokio::test]
        async fn test_basic_site() {
            let (mut crawler, page) = create_crawler("extract_single_href.html").await;

            let mut expected_queue = VecDeque::new();
            expected_queue.push_back(page.clone());
            assert_eq!(crawler.queue, expected_queue);

            crawler.crawl_page(page.clone(), None).await.unwrap();

            let expected_page = Page::from(Url::parse("https://www.wikipedia.org/").unwrap());
            assert!(crawler.queue.contains_page(&expected_page));
        }

        #[tokio::test]
        async fn test_already_visited_url() {
            let (mut crawler, page) = create_crawler("extract_single_href.html").await;

            let mut expected_queue = VecDeque::new();
            expected_queue.push_back(page.clone());
            assert_eq!(crawler.queue, expected_queue);

            crawler.crawl_page(page.clone(), None).await.unwrap();
            let queue_before = crawler.queue.clone();

            crawler.crawl_page(page.clone(), None).await.unwrap();
            assert_eq!(crawler.queue, queue_before)
        }
    }

    mod extract_urls_from_html {
        use super::super::Crawler;
        use crate::utils::test_file_path_from_filepath;
        use reqwest::Url;
        use scraper::Html;
        use std::{fs::File, io::Read};

        async fn test_and_extract_urls_from_html_file(filename: &str, expected_urls: Vec<String>) {
            // We don't need to send http requests in this module, so just provide a nonexistent site
            let non_existent_site = Url::parse("https://does-not-exist.comm").unwrap();
            let page = crate::page::Page::from(non_existent_site);
            let crawler = Crawler::new(vec![page], None).await;

            let html_file = test_file_path_from_filepath(filename);

            let error_msg = format!("'{}' should exist.", filename);
            let error_msg = error_msg.as_str();
            let mut html = File::open(html_file).expect(error_msg);

            let mut buf = String::new();
            html.read_to_string(&mut buf).unwrap();
            let buf = Html::parse_document(buf.as_str());

            let urls = crawler.extract_urls_from_html(&buf);
            assert_eq!(urls, expected_urls)
        }

        #[tokio::test]
        async fn test_single_href() {
            let filename = "extract_single_href.html";
            let expected_urls = vec![String::from("https://www.wikipedia.org/")];
            test_and_extract_urls_from_html_file(filename, expected_urls).await;
        }

        #[tokio::test]
        async fn test_multiple_hrefs() {
            let filename = "extract_multiple_hrefs.html";
            let expected_urls = vec![
                String::from("https://www.wikipedia.org/"),
                String::from("https://www.britannica.com/"),
                String::from("https://www.youtube.com/"),
            ];
            test_and_extract_urls_from_html_file(filename, expected_urls).await;
        }
    }
}
