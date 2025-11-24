// TODO: Add missing docstrings for methods

use std::{sync::Arc, time::Duration};

use crate::{
    db::{DbManager, RealDbManager},
    error::Error,
    page::{CrawledPage, Page, PageQueue},
    url_handler::UrlHandler,
    utils::string_to_url,
};
use dashmap::DashSet;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use reqwest::{Client, ClientBuilder, StatusCode, header::RETRY_AFTER};
use scraper::{Html, Selector};
use tokio::sync::Mutex;

type CrawledPageFutures = FuturesUnordered<
    std::pin::Pin<Box<dyn Future<Output = Result<(CrawledPage, Vec<Page>), Error>>>>,
>;

#[derive(Clone)]
pub struct CrawlerContext {
    pub client: Client,
    url_handler: Arc<UrlHandler>,
    // TODO: Convert to CrawledPage
    // Use [`Page`] instead of `CrawledPage` because comparing [`Page`] with `CrawledPage` does not work in DashSets for some reason
    crawled: Arc<DashSet<Page>>,
    queue: Arc<Mutex<PageQueue>>,
}

#[derive(Clone)]
pub struct Crawler {
    // Use arc since dyn means the compiler doesn't know the size of the object at compilation time
    db_manager: Arc<dyn DbManager>,
    pub context: Arc<CrawlerContext>,
}

impl Crawler {
    pub async fn new(starting_pages: Vec<Page>, pool: &sqlx::PgPool) -> Self {
        let db_manager = Arc::new(RealDbManager::new(pool.to_owned()));

        let crawled = db_manager.clone().init_crawled().await;

        let queue = db_manager.clone().init_queue(starting_pages).await;

        let client = Self::init_client();

        let url_handler = UrlHandler::new();

        let context = Arc::new(CrawlerContext {
            client,
            url_handler: Arc::new(url_handler),
            crawled: Arc::new(crawled),
            queue: Arc::new(Mutex::new(queue)),
        });

        Crawler {
            context,
            db_manager,
        }
    }

    /// Run the main loop of the Crawler.
    ///
    /// # Returns
    /// - Returns `Ok` if no unrecoverable errors occur.
    /// - Returns `Err` if an untested fatal error happens.
    pub async fn run(&mut self) -> Result<(), Error> {
        let mut futures: CrawledPageFutures = FuturesUnordered::new();

        let mut pages = Vec::new();
        while let Some(page) = self.next_page().await {
            pages.push(page);
        }

        let db_manager = self.db_manager.clone();

        for page in pages {
            futures.push(Box::pin(Crawler::crawl_page(
                page.clone(),
                self.context.clone(),
            )));
        }

        while let Some(result) = futures.next().await {
            match result {
                Ok((crawled_page, queue)) => {
                    db_manager.add_crawled_page_to_db(&crawled_page).await;

                    for page in queue {
                        self.add_page(page).await;
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
        self.context
            .queue
            .lock()
            .await
            .pop(self.db_manager.clone())
            .await
    }

    /// Add a [`Page`] to [`Crawler::queue`] and [`Crawler::crawled`]
    async fn add_page(&mut self, page: Page) {
        self.context
            .queue
            .lock()
            .await
            .queue_page(page.clone(), self.db_manager.clone())
            .await;
        self.context.crawled.insert(page);
    }

    /// Crawl a single page.
    ///
    /// TODO: Add return value section here
    /// # Errors
    /// This function returns a [`CrawlerError`] if:
    /// - The [`Page`]'s HTML could not be fetched due to a fatal HTTP status code or a request timeout.
    /// - The [`Page`] is not in English.
    /// - [`Crawler::extract_html_from_page`] fails.
    pub async fn crawl_page(
        page: Page,
        context: Arc<CrawlerContext>,
    ) -> Result<(CrawledPage, Vec<Page>), Error> {
        let html = Crawler::extract_html_from_page(page.clone(), context.client.clone()).await?;

        let html = Html::parse_document(html.as_str());

        if !UrlHandler::is_english(&html) {
            return Err(Error::NonEnglishPage(page));
        }

        if context.url_handler.is_inappropriate_page(&page, &html) {
            return Err(Error::InappropriateSite(page));
        }

        let title = Crawler::extract_title_from_html(&html);
        let urls = Crawler::extract_urls_from_html(&html);

        let base_url = page.url.clone();

        let mut queue = Vec::new();
        for url in urls {
            let url = string_to_url(&base_url, url);

            let page = if let Some(url) = url {
                Page::from(UrlHandler::normalize_url(url)?)
            } else {
                continue;
            };

            if context.crawled.contains(&page) || context.queue.lock().await.contains_page(&page) {
                log::warn!("{} is a duplicate", page.url);
                continue;
            }

            queue.push(page.clone());

            log::info!("{} is queued", page.url);

            // NOTE: Adding pages to the queue is now the `run` method's responsibility, so the crawler can run asynchronously
            // Add the page to the queue of pages to crawl
            // self.context.queue.lock().await
            //     .queue_page(page.clone(), self.db_manager.clone())
            //     .await;
            // Add the page to self.context, so that it is never crawled again
            // self.context.insert(page);
        }

        log::info!("Crawled {:?}...", base_url);

        Ok((page.into_crawled(title, html), queue))
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
    pub async fn extract_html_from_page(page: Page, client: Client) -> Result<String, Error> {
        let mut resp = Crawler::make_get_request(page.clone(), client.clone()).await?;

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
                        resp = Crawler::make_get_request(page.clone(), client.clone()).await?;
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

    /// Make a get request to a specific URL, and return the [`reqwest::Response`].
    ///
    /// # Errors
    /// This function returns a [`CrawlerError`] if:
    /// - There was an error while sending the request.
    /// - The redirect loop was detected.
    /// - The redirect limit was exhausted.
    async fn make_get_request(page: Page, client: Client) -> Result<reqwest::Response, Error> {
        client
            .get(page.url.clone())
            .send()
            .await
            .map_err(|e| Error::FailedRequest {
                page,
                error_str: e.to_string(),
            })
    }

    fn extract_urls_from_html(html: &Html) -> Vec<String> {
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

        let context = Arc::new(CrawlerContext {
            client,
            url_handler: Arc::new(url_handler),
            crawled: Arc::new(crawled),
            queue: Arc::new(Mutex::new(queue)),
        });

        Crawler {
            context,
            db_manager,
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

            let html = Crawler::extract_html_from_page(page, crawler.context.client.clone())
                .await
                .unwrap();
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

            let error =
                Crawler::extract_html_from_page(page.clone(), crawler.context.client.clone())
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

            let error =
                Crawler::extract_html_from_page(page.clone(), crawler.context.client.clone())
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

                let error =
                    Crawler::extract_html_from_page(page.clone(), crawler.context.client.clone())
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

                let error =
                    Crawler::extract_html_from_page(page.clone(), crawler.context.client.clone())
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

            let html = Html::parse_document(
                Crawler::extract_html_from_page(page, crawler.context.client.clone())
                    .await
                    .unwrap()
                    .as_str(),
            );
            let title = Crawler::extract_title_from_html(&html).unwrap();

            assert!(title.contains("a page with a title"))
        }

        #[tokio::test]
        async fn test_page_without_title() {
            let (crawler, page) = create_crawler("non_english_page.html").await;

            let html = Html::parse_document(
                Crawler::extract_html_from_page(page, crawler.context.client.clone())
                    .await
                    .unwrap()
                    .as_str(),
            );
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
            assert_eq!(
                crawler.context.queue.lock().await.to_owned(),
                expected_queue
            );

            let (_, queue) = Crawler::crawl_page(page.clone(), crawler.context.clone())
                .await
                .unwrap();

            crawler.add_page(queue[0].clone()).await;

            let expected_page = Page::from(Url::parse("https://www.wikipedia.org/").unwrap());
            assert!(
                crawler
                    .context
                    .queue
                    .lock()
                    .await
                    .contains_page(&expected_page)
            );
        }

        #[tokio::test]
        async fn test_already_visited_url() {
            let (crawler, page) = create_crawler("extract_single_href.html").await;

            let mut expected_queue = VecDeque::new();
            expected_queue.push_back(page.clone());
            assert_eq!(
                crawler.context.queue.lock().await.to_owned(),
                expected_queue
            );

            Crawler::crawl_page(page.clone(), crawler.context.clone())
                .await
                .unwrap();
            let queue_before = crawler.context.queue.lock().await.clone();

            Crawler::crawl_page(page.clone(), crawler.context.clone())
                .await
                .unwrap();
            assert_eq!(crawler.context.queue.lock().await.to_owned(), queue_before)
        }
    }

    mod extract_urls_from_html {
        use crate::{crawler::Crawler, utils::test_file_path_from_filepath};
        use scraper::Html;
        use std::{fs::File, io::Read};

        async fn test_and_extract_urls_from_html_file(filename: &str, expected_urls: Vec<String>) {
            let html_file = test_file_path_from_filepath(filename);

            let error_msg = format!("'{}' should exist.", filename);
            let error_msg = error_msg.as_str();
            let mut html = File::open(html_file).expect(error_msg);

            let mut buf = String::new();
            html.read_to_string(&mut buf).unwrap();
            let buf = Html::parse_document(buf.as_str());

            let urls = Crawler::extract_urls_from_html(&buf);
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
