use std::{collections::HashSet, time::Duration};

use once_cell::sync::Lazy;
use reqwest::{Client, ClientBuilder, StatusCode, Url, header::RETRY_AFTER};
use rustrict::{Censor, Type};
use scraper::{Html, Selector};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};

use crate::{
    error::CrawlerError,
    page::{CrawledPage, Page, PageQueue},
    utils::{construct_postgres_url, string_to_url},
};

#[derive(Clone)]
pub struct Crawler {
    queue: PageQueue,
    // Use [`Page`] instead of `CrawledPage` because comparing [`Page`] with `CrawledPage` does not work in hashsets for some reason
    // TODO: Convert to CrawledPage
    crawled: HashSet<Page>,
    pool: PgPool,
    client: Client,
}

// Should this be a global variable? No, but I need static access to this and this is the easiest solution ok-
// TODO: Find a way to move this into Crawler struct
static BLOCKED_KEYWORDS: Lazy<rustrict::Trie> = Lazy::new(|| {
    let mut trie = rustrict::Trie::default();

    // add a certain... domain that's been giving me trouble...
    trie.set("xvideos", Type::SEXUAL);

    trie
});

impl Crawler {
    pub async fn new(starting_pages: Vec<Page>) -> Self {
        let (pool, crawled) = Self::init_crawled_and_pool().await;

        let queue = Self::init_queue(starting_pages, &pool).await;

        let client = Self::init_client();

        Crawler {
            queue,
            crawled,
            pool,
            client,
        }
    }

    /// Run the main loop of the Crawler.
    ///
    /// # Returns
    /// - Returns `Ok` if no unrecoverable errors occur.
    /// - Returns `Err` if an untested fatal error happens.
    pub async fn run(&mut self) -> Result<(), CrawlerError> {
        while let Some(page) = self.next_page().await {
            match self.crawl_page(page.clone()).await {
                Ok(crawled_page) => {
                    if let Err(e) = crawled_page.add_to_db(&self.pool).await {
                        log::error!("Error inserting into DB: {}", e);
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
    pub async fn next_page(&mut self) -> Option<Page> {
        self.queue.pop(&self.pool).await
    }

    /// Crawl a single page.
    ///
    /// # Errors
    /// This function returns a [`CrawlerError`] if:
    /// - The [`Page`]'s HTML could not be fetched due to a fatal HTTP status code or a request timeout.
    /// - The [`Page`] is not in English.
    /// - [`Crawler::extract_html_from_page`] fails.
    pub(crate) async fn crawl_page(&mut self, page: Page) -> Result<CrawledPage, CrawlerError> {
        let html = self.extract_html_from_page(page.clone()).await?;

        let html = Html::parse_fragment(html.as_str());

        if !Self::is_english(&html) {
            return Err(CrawlerError::NonEnglishPage(page));
        }

        if Self::is_inappropriate_page(&page, &html) {
            return Err(CrawlerError::InappropriateSite(page));
        }

        let title = Self::extract_title_from_html(&html);
        let urls = self.extract_urls_from_html(&html);

        let base_url = page.url.clone();

        for url in urls {
            let url = string_to_url(&base_url, url);

            let page = if let Some(url) = url {
                Page::from(url)
            } else {
                continue;
            };

            if self.crawled.contains(&page) || self.is_page_queued(&page) {
                log::warn!("{} is a duplicate", page.url);
                continue;
            }

            // Add the page to the queue of pages to crawl
            if let Err(e) = self.queue.queue_page(page.clone(), &self.pool).await {
                log::warn!("Error with queueing page: {}", e);
                continue;
            };

            log::info!("{} is queued", page.url);

            // Add the page to self.crawled, so that it is never crawled again
            self.crawled.insert(page);
        }

        log::info!("Crawled {:?}...", base_url);

        Ok(page.into_crawled(title, html.html()))
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
    async fn extract_html_from_page(&self, page: Page) -> Result<String, CrawlerError> {
        let mut resp = self.make_get_request(page.clone()).await?;

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let html = Self::extract_html_from_resp(resp).await?;

                if html.is_none() {
                    return Err(CrawlerError::EmptyPage(page));
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
                        return Err(CrawlerError::InvalidRetryByHeader {
                            page,
                            header: Some(retry_after.to_owned()),
                        });
                    }

                    let delay_secs = delay_secs.unwrap();

                    let delay = Duration::from_secs(delay_secs);

                    if delay > MAX_DELAY {
                        return Err(CrawlerError::RequestTimeout(page));
                    }

                    tokio::time::sleep(delay).await;

                    while attempts <= MAX_ATTEMPTS && resp.status() != StatusCode::OK {
                        resp = self.make_get_request(page.clone()).await?;
                        attempts += 1;
                    }

                    if resp.status() != StatusCode::OK {
                        return Err(CrawlerError::RequestTimeout(page));
                    }

                    let html = Self::extract_html_from_resp(resp).await?;

                    Ok(html.unwrap())
                } else {
                    // just give up. it's not worth it.
                    Err(CrawlerError::InvalidRetryByHeader { page, header: None })
                }
            }
            // just give up. it's not worth it.
            _ => Err(CrawlerError::MalformedHttpStatus { page, status }),
        }
    }

    /// Checks URL domain against a list of blocked keywords relating to inappropriate content.
    fn is_inappropriate_page(page: &Page, html: &Html) -> bool {
        // add a certain uh... domain that's been giving me trouble
        let mut blocked_keywords = rustrict::Trie::default();
        blocked_keywords.set("xvideos", Type::SEXUAL);

        let mut domain = Censor::from_str(page.url.as_str());
        domain.with_trie(&BLOCKED_KEYWORDS);

        // First check that the domain is appropriate
        // Note that `Type::NONE` just means that the content is
        // appropriate
        if domain.analyze() != Type::NONE {
            return true;
        }

        let body_selector = Selector::parse("body").unwrap();

        let content = html
            .select(&body_selector)
            .flat_map(|e| e.text())
            .flat_map(|t| t.split_whitespace())
            .collect::<String>();
        let mut content = Censor::from_str(content.as_str());
        content.with_trie(&BLOCKED_KEYWORDS);

        // Then check if the content is appropriate
        content.analyze() != Type::NONE
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
    async fn make_get_request(&self, page: Page) -> Result<reqwest::Response, CrawlerError> {
        self.client
            .get(page.url.clone())
            .send()
            .await
            .map_err(|e| CrawlerError::FailedRequest {
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

    async fn init_queue(starting_pages: Vec<Page>, pool: &sqlx::PgPool) -> PageQueue {
        let mut queue = PageQueue::default();

        queue.refresh_queue(pool).await;

        // Queue each page in starting_pages
        for page in starting_pages {
            if let Err(e) = queue.queue_page(page, pool).await {
                log::warn!("Error initializing page queue: {}", e);
            };
        }

        queue
    }

    /// Initialize the hashset of visited [`Page`]'s and the Postgres pool.
    /// Will return an empty hashset if the database is empty.
    async fn init_crawled_and_pool() -> (sqlx::Pool<sqlx::Postgres>, HashSet<Page>) {
        let url = construct_postgres_url();
        let url = url.as_str();

        let pool = PgPoolOptions::new()
            .max_connections(10) // maximum connections in the pool
            .min_connections(2) // keep a few always alive
            .acquire_timeout(std::time::Duration::from_secs(5)) // connection timeout
            .max_lifetime(Some(std::time::Duration::from_secs(1800))) // recycle old connections
            .idle_timeout(Some(std::time::Duration::from_secs(600))) // close idle connections
            .connect(url) // async connect
            .await
            .expect("DATABASE_URL should correctly point to the PostGreSQL database.");

        let visited_query = "SELECT * FROM pages WHERE is_crawled = TRUE;";
        let mut visited = HashSet::new();

        let query = sqlx::query(visited_query);

        let rows = (query.fetch_all(&pool).await).ok();

        if rows.is_none() {
            return (pool, visited);
        }

        rows.unwrap().iter().for_each(|row| {
            let page = Page::from(Url::parse(row.get("url")).unwrap());

            visited.insert(page);
        });

        (pool, visited)
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
    async fn extract_html_from_resp(
        resp: reqwest::Response,
    ) -> Result<Option<String>, CrawlerError> {
        let url = resp.url().clone();

        let html = resp
            .text()
            .await
            .map_err(|e| CrawlerError::HtmlDecodingError {
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

// Methods for benchmarks
#[cfg(feature = "bench")]
impl Crawler {
    fn init_queue_test(starting_pages: Vec<Page>) -> PageQueue {
        let mut queue = PageQueue::default();

        for page in starting_pages {
            queue.queue_page_test(page);
        }

        queue
    }

    /// Returns the next [`Page`] in the queue.
    ///
    /// # Returns
    /// - Return `Some(Page)` if a [`Page`] exists in the queue.
    /// - Returns `None` if the queue is empty.
    fn next_page_test(&mut self) -> Option<Page> {
        self.queue.pop_test()
    }

    /// Perform a test run without writing to the database.
    ///
    /// # Returns
    /// - Returns `Ok` if no errors happen.
    /// - Returns `Err` if an untested fatal error happens.
    pub async fn test_run(&mut self) {
        while let Some(page) = self.next_page_test() {
            match self.crawl_page_test(page.clone()).await {
                Ok(_) => {
                    log::info!("Crawl successful.");
                }
                Err(e) => {
                    log::warn!("Crawl failed: {}", e);
                }
            }
        }

        log::info!("All done! no more pages left");
    }
}

// Methods for tests and benchmarks
#[cfg(any(test, feature = "bench"))]
impl Crawler {
    /// Create a test instance of a [`Crawler`], which uses an empty [`HashSet`] for `crawled`, making new instances much faster to create.
    /// Also uses [`PgPool::connect_lazy`] to create a connection, which is much faster and lightweight.
    pub async fn test_new(starting_url: Page) -> Self {
        let url = "postgres://search_db_user:123@localhost:5432/search_db";
        let pool = sqlx::postgres::PgPool::connect_lazy(url).unwrap();

        let queue = Self::init_queue_test(vec![starting_url]);

        let crawled = HashSet::new();

        let client = Self::init_client();

        Crawler {
            queue,
            crawled,
            pool,
            client,
        }
    }

    /// Crawl a single page without writing to the database.
    ///
    /// # Errors
    /// This function returns a [`CrawlerError`] if:
    /// - The [`Page`]'s HTML could not be fetched due to a fatal HTTP status code or a request timeout.
    /// - The [`Page`] is not in English.
    /// - [`Crawler::extract_html_from_page`] fails.
    pub async fn crawl_page_test(&mut self, page: Page) -> Result<CrawledPage, CrawlerError> {
        let html = self.extract_html_from_page(page.clone()).await?;

        let html = Html::parse_fragment(html.as_str());

        if !Self::is_english(&html) {
            return Err(CrawlerError::NonEnglishPage(page));
        }

        let title = Self::extract_title_from_html(&html);
        let urls = self.extract_urls_from_html(&html);

        let base_url = page.url.clone();

        for url in urls {
            let url = string_to_url(&base_url, url);

            let page = if let Some(url) = url {
                Page::from(url)
            } else {
                continue;
            };

            if self.crawled.contains(&page) || self.is_page_queued(&page) {
                log::warn!("{} is a duplicate", page.url);
                continue;
            }

            // Add the page to the queue of pages to crawl
            self.queue.queue_page_test(page.clone());

            log::info!("{} is queued", page.url);

            // Add the page to self.crawled, so that it is never crawled again
            self.crawled.insert(page);
        }

        log::info!("Crawled {:?}...", base_url);

        Ok(page.into_crawled(title, html.html()))
    }
}

#[cfg(test)]
mod test {
    use crate::{
        crawler::Crawler,
        page::Page,
        utils::{HttpServer, test_file_path_from_filepath},
    };

    async fn create_crawler(starting_filename: &str) -> (Crawler, Page) {
        let filepath = test_file_path_from_filepath(starting_filename);
        let server = HttpServer::new_with_filepath(filepath);

        let page = Page::from(server.base_url());

        (Crawler::test_new(page.clone()).await, page)
    }

    mod is_english {
        use super::*;
        use scraper::Html;

        #[tokio::test]
        async fn test_non_english_page() {
            let (crawler, page) = create_crawler("non_english_page.html").await;

            let html = crawler.extract_html_from_page(page).await.unwrap();
            let html = Html::parse_fragment(html.as_str());

            assert!(!Crawler::is_english(&html));
        }
    }

    mod is_inappropriate_page {
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
                <p>hippopotamus hippopotamus hippopotamus</p>
            </body>"#,
            );

            let page = Page::from(Url::parse("https://a-very-innocent-site.com").unwrap());

            assert!(Crawler::is_inappropriate_page(&page, &html));
        }

        #[tokio::test]
        async fn test_safe_page() {
            let page = Page::from(Url::parse("https://safe.com").unwrap());
            assert!(!Crawler::is_inappropriate_page(
                &page,
                &Html::new_document()
            ));
        }
    }

    mod extract_html_from_page {
        use super::*;
        use crate::error::CrawlerError;
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
            let crawler = Crawler::test_new(page.clone()).await;

            let error = crawler
                .extract_html_from_page(page.clone())
                .await
                .unwrap_err();

            assert_eq!(
                error,
                CrawlerError::MalformedHttpStatus {
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
            assert_eq!(error, CrawlerError::EmptyPage(page))
        }

        mod status_429 {
            use crate::{
                crawler::Crawler,
                error::CrawlerError,
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
                let crawler = Crawler::test_new(page.clone()).await;

                let start = Instant::now();
                let error = crawler
                    .extract_html_from_page(page.clone())
                    .await
                    .unwrap_err();
                let end = Instant::now();

                let elapsed = (end - start).as_secs();
                assert_eq!(elapsed, TRY_AFTER_SECS);
                assert_eq!(error, CrawlerError::RequestTimeout(page))
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
                let crawler = Crawler::test_new(page.clone()).await;

                let error = crawler
                    .extract_html_from_page(page.clone())
                    .await
                    .unwrap_err();
                assert_eq!(error, CrawlerError::RequestTimeout(page))
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
                let crawler = Crawler::test_new(page.clone()).await;

                let error = crawler
                    .extract_html_from_page(page.clone())
                    .await
                    .unwrap_err();

                assert_eq!(
                    error,
                    CrawlerError::InvalidRetryByHeader { page, header: None }
                )
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
                Html::parse_fragment(crawler.extract_html_from_page(page).await.unwrap().as_str());
            let title = Crawler::extract_title_from_html(&html).unwrap();

            assert!(title.contains("a page with a title"))
        }

        #[tokio::test]
        async fn test_page_without_title() {
            let (crawler, page) = create_crawler("non_english_page.html").await;

            let html =
                Html::parse_fragment(crawler.extract_html_from_page(page).await.unwrap().as_str());
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

            crawler.crawl_page_test(page.clone()).await.unwrap();

            let expected_page = Page::from(Url::parse("https://www.wikipedia.org/").unwrap());
            assert!(crawler.queue.contains_page(&expected_page));
        }

        #[tokio::test]
        async fn test_already_visited_url() {
            let (mut crawler, page) = create_crawler("extract_single_href.html").await;

            let mut expected_queue = VecDeque::new();
            expected_queue.push_back(page.clone());
            assert_eq!(crawler.queue, expected_queue);

            crawler.crawl_page_test(page.clone()).await.unwrap();
            let queue_before = crawler.queue.clone();

            crawler.crawl_page_test(page.clone()).await.unwrap();
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
            let crawler = Crawler::test_new(page).await;

            let html_file = test_file_path_from_filepath(filename);

            let error_msg = format!("'{}' should exist.", filename);
            let error_msg = error_msg.as_str();
            let mut html = File::open(html_file).expect(error_msg);

            let mut buf = String::new();
            html.read_to_string(&mut buf).unwrap();
            let buf = Html::parse_fragment(buf.as_str());

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
