// TODO: Add missing docstrings for methods

use std::{collections::HashSet, sync::Arc, time::Duration};

use crate::{
    db::{DbManager, RealDbManager},
    error::Error,
    page::{CrawledPage, Page, PageQueue},
    url_handler::UrlHandler,
    utils::string_to_url,
};
use reqwest::{Client, ClientBuilder, StatusCode, header::RETRY_AFTER};
use scraper::{Html, Selector};

#[derive(Clone)]
pub struct Crawler {
    queue: PageQueue,
    // Use [`Page`] instead of `CrawledPage` because comparing [`Page`] with `CrawledPage` does not work in hashsets for some reason
    // TODO: Convert to CrawledPage
    crawled: HashSet<Page>,
    client: Client,
    // Use arc since dyn means the compiler doesn't know the size of the object at compilation time
    db_manager: Arc<dyn DbManager>,
    url_handler: UrlHandler,
}

impl Crawler {
    pub async fn new(starting_pages: Vec<Page>, pool: &sqlx::PgPool) -> Self {
        let db_manager = Arc::new(RealDbManager::new(pool.to_owned()));

        let crawled = db_manager.clone().init_crawled().await;

        let queue = db_manager.clone().init_queue(starting_pages).await;

        let client = Self::init_client();

        let url_handler = UrlHandler::new();

        Crawler {
            queue,
            crawled,
            client,
            db_manager,
            url_handler,
        }
    }

    /// Run the main loop of the Crawler.
    ///
    /// # Returns
    /// - Returns `Ok` if no unrecoverable errors occur.
    /// - Returns `Err` if an untested fatal error happens.
    pub async fn run(&mut self) -> Result<(), Error> {
        while let Some(page) = self.next_page().await {
            match self.crawl_page(page.clone()).await {
                Ok(crawled_page) => {
                    self.db_manager.add_crawled_page_to_db(&crawled_page).await;
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
        self.queue.pop(self.db_manager.clone()).await
    }

    /// Crawl a single page.
    ///
    /// # Errors
    /// This function returns a [`CrawlerError`] if:
    /// - The [`Page`]'s HTML could not be fetched due to a fatal HTTP status code or a request timeout.
    /// - The [`Page`] is not in English.
    /// - [`Crawler::extract_html_from_page`] fails.
    pub async fn crawl_page(&mut self, page: Page) -> Result<CrawledPage, Error> {
        let html = self.extract_html_from_page(page.clone()).await?;

        let html = Html::parse_document(html.as_str());

        if !UrlHandler::is_english(&html) {
            return Err(Error::NonEnglishPage(page));
        }

        if self.url_handler.is_inappropriate_page(&page, &html) {
            return Err(Error::InappropriateSite(page));
        }

        let title = Self::extract_title_from_html(&html);
        let urls = self.extract_urls_from_html(&html);

        let base_url = page.url.clone();

        for url in urls {
            let url = string_to_url(&base_url, url);

            let page = if let Some(url) = url {
                Page::from(UrlHandler::normalize_url(url)?)
            } else {
                continue;
            };

            if self.crawled.contains(&page) || self.is_page_queued(&page) {
                log::warn!("{} is a duplicate", page.url);
                continue;
            }

            // Add the page to the queue of pages to crawl
            self.queue
                .queue_page(page.clone(), self.db_manager.clone())
                .await;

            log::info!("{} is queued", page.url);

            // Add the page to self.crawled, so that it is never crawled again
            self.crawled.insert(page);
        }

        log::info!("Crawled {:?}...", base_url);

        Ok(page.into_crawled(title, html))
    }

    fn extract_title_from_html(html: &Html) -> Option<String> {
        let selector =
            Selector::parse("title").expect("Parsing 'title' selector should not throw an error.");

        let element = html.select(&selector).next();

        element.map(|element| element.text().collect::<String>())
    }

    /// Extracts the HTML from a [`Page`].
    ///
    /// # Errors
    /// This function returns a [`CrawlerError`] if:
    /// - The [`Page`] is empty (has no HTML).
    /// - The response contains a non-200 or non-429 HTTP status code, or the request times out.
    /// - Sending the request results in an error.
    /// - Decoding the HTML from the [`reqwest::Response`] throws an error, such as UTF-8 errors.
    pub async fn extract_html_from_page(&self, page: Page) -> Result<String, Error> {
        let mut resp = self.make_get_request(page.clone()).await?;

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let html = Self::extract_html_from_resp(resp).await?;

                if html.is_none() {
                    return Err(Error::EmptyPage(page));
                }

                Ok(html.expect("`html` var can only be `Some`."))
            }
            StatusCode::TOO_MANY_REQUESTS => {
                const MAX_ATTEMPTS: u8 = 10;
                const MAX_DELAY: Duration = Duration::from_secs(60);

                let mut attempts = 0;

                let retry_after = resp.headers().get(RETRY_AFTER);

                if let Some(retry_after) = retry_after {
                    let delay_secs: Result<u64, _> = retry_after
                        .to_str()
                        .expect(
                            "Converting Retry-After header to a string should not return an error.",
                        )
                        .parse();

                    // If delay_secs is not a valid value in seconds
                    if delay_secs.is_err() {
                        return Err(Error::InvalidRetryByHeader {
                            page,
                            header: Some(retry_after.to_owned()),
                        });
                    }

                    let delay_secs = delay_secs.expect(
                        "Parsing a Retry-After header into a u64 value should not return an error.",
                    );

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

                    if html.is_none() {
                        return Err(Error::EmptyPage(page));
                    }

                    Ok(html.expect("`html` var can only be `Some`."))
                } else {
                    // just give up. it's not worth it.
                    Err(Error::InvalidRetryByHeader { page, header: None })
                }
            }
            // just give up. it's not worth it.
            _ => Err(Error::MalformedHttpStatus { page, status }),
        }
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

        let selector =
            Selector::parse("a").expect("Parsing `a` selector should not throw an error.");

        for element in html.select(&selector) {
            if let Some(url) = element.value().attr("href") {
                urls.push(url.to_owned());
            };
        }

        urls
    }

    fn init_client() -> Client {
        ClientBuilder::new()
            .user_agent(crate::USER_AGENT)
            // Reduce bandwidth usage; compliant with wikimedia's robot policy: https://wikitech.wikimedia.org/wiki/Robot_policy#Generally_applicable_rules
            .gzip(true)
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Creating a `reqwest::Client` should not throw an error.")
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

#[cfg(any(test, feature = "bench-utils"))]
impl Crawler {
    pub async fn test_new(starting_pages: Vec<Page>) -> Self {
        use crate::db::MockDbManager;

        let db_manager: Arc<dyn DbManager> = Arc::new(MockDbManager::new());

        let crawled = db_manager.clone().init_crawled().await;

        let queue = db_manager.clone().init_queue(starting_pages).await;

        let client = Self::init_client();

        let url_handler = UrlHandler::new();

        Crawler {
            queue,
            crawled,
            client,
            db_manager,
            url_handler,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use crate::{
        crawler::Crawler,
        page::Page,
        utils::{HttpServer, create_crawler, test_file_path_from_filepath},
    };

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
            let crawler = Crawler::test_new(vec![page.clone()]).await;

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
                let crawler = Crawler::test_new(vec![page.clone()]).await;

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
                let crawler = Crawler::test_new(vec![page.clone()]).await;

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

            crawler.crawl_page(page.clone()).await.unwrap();

            let expected_page = Page::from(Url::parse("https://www.wikipedia.org/").unwrap());
            assert!(crawler.queue.contains_page(&expected_page));
        }

        #[tokio::test]
        async fn test_already_visited_url() {
            let (mut crawler, page) = create_crawler("extract_single_href.html").await;

            let mut expected_queue = VecDeque::new();
            expected_queue.push_back(page.clone());
            assert_eq!(crawler.queue, expected_queue);

            crawler.crawl_page(page.clone()).await.unwrap();
            let queue_before = crawler.queue.clone();

            crawler.crawl_page(page.clone()).await.unwrap();
            assert_eq!(crawler.queue, queue_before)
        }
    }

    mod extract_urls_from_html {
        use crate::{crawler::Crawler, utils::test_file_path_from_filepath};
        use reqwest::Url;
        use scraper::Html;
        use std::{fs::File, io::Read};

        async fn test_and_extract_urls_from_html_file(filename: &str, expected_urls: Vec<String>) {
            // We don't need to send http requests in this module, so just provide a nonexistent site
            let non_existent_site = Url::parse("https://does-not-exist.comm").unwrap();
            let page = crate::page::Page::from(non_existent_site);
            let crawler = Crawler::test_new(vec![page]).await;

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
