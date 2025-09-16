use std::{
    clone,
    collections::{HashSet, VecDeque},
    time::Duration,
};

use reqwest::{Client, ClientBuilder, StatusCode, Url, header::RETRY_AFTER};
use scraper::{Html, Selector};

use sqlx::{PgPool, Row};

use crate::{
    page::{CrawledPage, Page},
    utils::string_to_url,
};

#[derive(Clone)]
pub struct Crawler {
    queue: VecDeque<Page>,
    // Use `Page` instead of `CrawledPage` because comparing `Page` with `CrawledPage` does not work in hashsets for some reason
    // TODO: Convert to CrawledPage
    visited: HashSet<Page>,
    pool: PgPool,
    client: Client,
}

impl Crawler {
    pub async fn new(starting_url: Page) -> Self {
        let queue = Self::init_queue(starting_url);

        let (pool, visited) = Self::init_visited_and_pool().await;

        let client = Self::init_client();

        Crawler {
            queue,
            visited,
            pool,
            client,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(page) = self.queue.pop_back() {
            let crawled_page = self.crawl_page(page.clone()).await;

            if let Ok(crawled_page) = crawled_page
                && let Some(crawled_page) = crawled_page
            {
                if let Err(e) = crawled_page.add_to_db(&self.pool).await {
                    eprintln!("Error: {}", e);
                }
            } else {
                println!("{} is unreachable", page.url);
            };
        }

        println!("All done! no more pages left");
        Ok(())
    }

    /// Perform a test run without writing to the database.
    /// Even though this is public, this method is meant to be used for benchmarks and tests only.
    pub async fn test_run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(page) = self.queue.pop_back() {
            let _crawled_page = self.crawl_page(page).await?;
        }

        println!("All done! no more pages left");
        Ok(())
    }

    /// Reset `self.visited` and `self.queue`.
    /// Even though this is public, this method is meant to be used for benchmarks and tests only.
    pub fn reset(&mut self) {
        self.queue = VecDeque::new();
        self.visited = HashSet::new();
    }

    /// Crawl a single page.
    /// # Returns
    /// - Returns `Ok(None)` if the `Page`'s HTML could not be fetched due to a fatal HTTP status code or a request timeout.
    /// - Returns `Err` if there is an edge case that has not been tested yet.
    ///
    /// # Note
    /// Even though this is public, this method is meant to be used for benchmarks and tests only.
    pub async fn crawl_page(
        &mut self,
        page: Page,
    ) -> Result<Option<CrawledPage>, Box<dyn std::error::Error>> {
        let html = self.extract_html_from_page(page.clone()).await?;

        if html.is_none() {
            return Ok(None);
        }

        let html = html.unwrap();

        let urls = self.extract_urls_from_html(html.as_str());

        let base_url = page.url.clone();

        for url in urls {
            let url = string_to_url(&base_url, url);

            let page = if let Some(url) = url {
                Page::from(url)
            } else {
                continue;
            };

            if self.visited.contains(&page) || self.is_page_queued(&page) {
                println!("{} is a duplicate", page.url);
                continue;
            }

            // Add the page to the queue of pages to crawl
            self.queue.push_front(page.clone());
            println!("{} is queued", page.url);

            // Add the page to self.visited, so that it is never crawled again
            self.visited.insert(page);
        }

        println!("Crawled {:?}...", base_url);

        Ok(Some(page.into_crawled(html)))
    }

    /// Extracts the html from a `Page`.
    /// # Returns
    /// - Returns `None` if the response contains a fatal HTTP status code, or the request times out.
    /// - Returns `Err` if sending the request results in an error.
    async fn extract_html_from_page(
        &self,
        page: Page,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let mut resp = self.make_get_request(page.clone()).await?;

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let html = resp.text().await;
                if let Err(e) = html {
                    Err(Box::new(e))
                } else {
                    Ok(Some(html.unwrap()))
                }
            }
            StatusCode::TOO_MANY_REQUESTS => {
                const MAX_ATTEMPTS: u8 = 10;
                const MAX_DELAY: Duration = Duration::from_secs(60);

                let mut attempts = 0;

                let retry_after = resp.headers().get(RETRY_AFTER);

                if let Some(delay_secs) = retry_after {
                    let delay_secs: Result<u64, _> = delay_secs.to_str().unwrap().parse();

                    // If
                    if delay_secs.is_err() {
                        return Ok(None);
                    }

                    let delay_secs = delay_secs.unwrap();

                    let delay = Duration::from_secs(delay_secs);

                    if delay > MAX_DELAY {
                        return Ok(None);
                    }

                    tokio::time::sleep(delay).await;

                    while attempts <= MAX_ATTEMPTS && resp.status() != StatusCode::OK {
                        resp = self.make_get_request(page.clone()).await?;
                        attempts += 1;
                    }

                    let html = resp.text().await;

                    if let Err(e) = html {
                        Err(Box::new(e))
                    } else {
                        Ok(Some(html.unwrap()))
                    }
                } else {
                    // just give up. it's not worth it.
                    Ok(None)
                }
            }
            _ => todo!(),
        }
    }

    fn is_page_queued(&self, page: &Page) -> bool {
        self.queue.iter().any(|crawled_page| page == crawled_page)
    }

    /// Make a get request to a specific URL.
    /// # Returns
    /// - A `Response`, which contains the HTML and HTTP status code of the request
    async fn make_get_request(
        &self,
        page: Page,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        Ok(self.client.get(page.url).send().await?)
    }

    fn extract_urls_from_html(&self, html: &str) -> Vec<String> {
        let mut urls = vec![];

        let fragment = Html::parse_fragment(html);
        let selector = Selector::parse("a").unwrap();

        for element in fragment.select(&selector) {
            if let Some(url) = element.value().attr("href") {
                urls.push(url.to_owned());
            };
        }

        urls
    }

    fn init_queue(starting_url: Page) -> VecDeque<Page> {
        let mut queue = VecDeque::new();
        queue.push_back(starting_url);
        queue
    }

    /// Initialize the hashset of visited `Page`'s and the Postgres pool.
    /// Will return an empty hashset if the database is empty.
    async fn init_visited_and_pool() -> (sqlx::Pool<sqlx::Postgres>, HashSet<Page>) {
        let url = "postgres://search_db_user:123@localhost:5432/search_db";

        let pool = sqlx::postgres::PgPool::connect(url).await.unwrap();

        let visited_query = "SELECT * FROM public.pages WHERE http_status = 200";
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
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    mod extract_html_from_page {

        use crate::{page::Page, utils::HttpServer};

        use super::super::Crawler;

        // Instead of using #[test], we use #[tokio::test] so we can test async functions
        #[tokio::test]
        async fn test_200_status() {
            let server = HttpServer::new_with_filename("extract_single_href.html");

            let page = Page::from(server.base_url());
            let crawler = Crawler::new(page.clone()).await;

            let html = crawler.extract_html_from_page(page).await.unwrap();

            assert_eq!(
                html.unwrap().strip_suffix("\n").unwrap(),
                String::from(r#"<a href="https://www.wikipedia.org/">This is a link.</a>"#)
            );
        }

        mod status_429 {
            use httpmock::Method::GET;
            use reqwest::StatusCode;
            use tokio::time::Instant;

            use crate::{
                crawler::Crawler,
                page::Page,
                utils::{HttpServer, test_file_path_from_filename},
            };

            #[tokio::test]
            async fn test_429_status() {
                const TRY_AFTER_SECS: u64 = 1;
                let filepath = test_file_path_from_filename("extract_single_href.html");

                let server = HttpServer::new_with_mock(|when, then| {
                    when.method(GET).header("user-agent", crate::USER_AGENT);
                    then.status(StatusCode::TOO_MANY_REQUESTS.as_u16())
                        .header("content-type", "text/html")
                        .header("retry-after", TRY_AFTER_SECS.to_string())
                        .body_from_file(filepath.display().to_string());
                });

                let page = Page::from(server.base_url());
                let crawler = Crawler::new(page.clone()).await;

                let start = Instant::now();
                let html = crawler.extract_html_from_page(page).await.unwrap();
                let end = Instant::now();

                let elapsed = (end - start).as_secs();

                // Fail the test if the retry-after header is not respected
                assert_eq!(elapsed, TRY_AFTER_SECS);

                assert_eq!(
                    html.unwrap().strip_suffix("\n").unwrap(),
                    String::from(r#"<a href="https://www.wikipedia.org/">This is a link.</a>"#)
                );
            }

            #[tokio::test]
            async fn test_429_status_with_large_retry_after() {
                // After 60 seconds, just don't bother
                const TRY_AFTER_SECS: u64 = 61;
                let filepath = test_file_path_from_filename("extract_single_href.html");

                let server = HttpServer::new_with_mock(|when, then| {
                    when.method(GET).header("user-agent", crate::USER_AGENT);
                    then.status(StatusCode::TOO_MANY_REQUESTS.as_u16())
                        .header("content-type", "text/html")
                        .header("retry-after", TRY_AFTER_SECS.to_string())
                        .body_from_file(filepath.display().to_string());
                });

                let page = Page::from(server.base_url());
                let crawler = Crawler::new(page.clone()).await;

                let html = crawler.extract_html_from_page(page).await.unwrap();

                assert!(html.is_none());
            }

            #[tokio::test]
            async fn test_429_status_with_no_header() {
                let filepath = test_file_path_from_filename("extract_single_href.html");

                let server = HttpServer::new_with_mock(|when, then| {
                    when.method(GET).header("user-agent", crate::USER_AGENT);
                    then.status(429)
                        .header("content-type", "text/html")
                        .body_from_file(filepath.display().to_string());
                });

                let page = Page::from(server.base_url());
                let crawler = Crawler::new(page.clone()).await;

                let html = crawler.extract_html_from_page(page).await.unwrap();

                assert!(html.is_none());
            }
        }
    }

    mod crawl_next_url {
        use std::collections::{HashSet, VecDeque};

        use reqwest::Url;

        use crate::{page::Page, utils::HttpServer};

        use super::super::Crawler;

        #[tokio::test]
        async fn test_basic_site() {
            let server = HttpServer::new_with_filename("extract_single_href.html");

            let page = Page::from(server.base_url());

            let mut crawler = Crawler::new(page.clone()).await;

            // Reset crawler.visited, which gets loaded from the db (we don't want that)
            crawler.visited = HashSet::new();

            let mut expected_queue = VecDeque::new();
            expected_queue.push_back(page.clone());

            assert_eq!(crawler.queue, expected_queue);

            crawler.crawl_page(page.clone()).await.unwrap();

            let expected_url = Page::from(Url::parse("https://www.wikipedia.org/").unwrap());

            assert!(crawler.queue.contains(&expected_url));
        }

        #[tokio::test]
        async fn test_already_visited_url() {
            let server = HttpServer::new_with_filename("extract_single_href.html");

            let page = Page::from(server.base_url());
            let mut crawler = Crawler::new(page.clone()).await;

            let mut expected_queue = VecDeque::new();
            expected_queue.push_back(page.clone());

            assert_eq!(crawler.queue, expected_queue);

            // Crawl the page for the first time
            crawler.crawl_page(page.clone()).await.unwrap();

            let queue_before = crawler.queue.clone();

            // Crawl the page a second time. After this, the queue should stay exactly the same.
            crawler.crawl_page(page.clone()).await.unwrap();

            assert_eq!(crawler.queue, queue_before)
        }
    }

    mod extract_urls_from_html {
        use std::{fs::File, io::Read};

        use reqwest::Url;

        use crate::{page::Page, utils::test_file_path_from_filename};

        use super::super::Crawler;

        async fn test_and_extract_urls_from_html_file(filename: &str, expected_urls: Vec<String>) {
            // We don't need to send http requests in this module, so just provide a nonexistent site
            let non_existent_site = Url::parse("https://does-not-exist.comm").unwrap();
            let page = Page::from(non_existent_site);

            let crawler = Crawler::new(page).await;

            let html_file = test_file_path_from_filename(filename);

            let error_msg = format!("'{}' should exist.", filename);
            let error_msg = error_msg.as_str();
            let mut html = File::open(html_file).expect(error_msg);

            let mut buf = String::new();
            html.read_to_string(&mut buf).unwrap();

            let urls = crawler.extract_urls_from_html(buf.as_str());

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
