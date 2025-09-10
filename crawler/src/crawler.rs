use std::collections::VecDeque;

use reqwest::Url;
use scraper::{Html, Selector};

use crate::page::Page;

pub struct Crawler {
    queue: VecDeque<Page>,
}

impl Crawler {
    pub fn new(starting_url: Page) -> Self {
        let mut queue = VecDeque::new();

        queue.push_back(starting_url);

        Crawler { queue }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(page) = self.queue.pop_back() {
            self.crawl_page(page).await.unwrap();
        }

        Ok(())
    }

    // TODO: Make this private somehow, since this needs to be public for benchmarks
    pub async fn crawl_page(&mut self, page: Page) -> Result<(), Box<dyn std::error::Error>> {
        let html = Self::make_get_request(page.clone()).await?;
        let urls = self.extract_urls_from_html(html);

        let base_url = page.url;

        for url in urls {
            let page = Page::from(
                if url.starts_with("https://") || url.starts_with("http://") {
                    Url::parse(url.as_str()).unwrap()
                } else {
                    base_url.join(url.as_str()).unwrap()
                },
            );

            if self.is_page_queued(&page) {
                continue;
            }

            // Add the page to the queue of pages to crawl
            self.queue.push_front(page.clone());
        }

        println!("Crawled {:?}...", base_url);
        Ok(())
    }

    fn is_page_queued(&self, page: &Page) -> bool {
        self.queue.iter().any(|crawled_page| page == crawled_page)
    }

    /// Make a get request to a specific URL.
    /// This (should) return the HTML of the URL.
    async fn make_get_request(page: Page) -> Result<String, Box<dyn std::error::Error>> {
        Ok(reqwest::get(page.url).await?.text().await?)
    }

    fn extract_urls_from_html(&self, html: String) -> Vec<String> {
        let mut urls = vec![];

        let fragment = Html::parse_fragment(html.as_str());
        let selector = Selector::parse("a").unwrap();

        for element in fragment.select(&selector) {
            if let Some(url) = element.value().attr("href") {
                urls.push(url.to_string());
            }
        }

        urls
    }
}

#[cfg(test)]
mod test {
    mod make_get_request {
        use url::Url;

        use crate::page::Page;

        use super::super::Crawler;

        // Instead of using #[test], we use #[tokio::test] so we can test async functions
        #[tokio::test]
        async fn test_basic_site() {
            let page =
                Page::from(Url::parse("https://crawler-test.com/status_codes/status_200").unwrap());
            let html = Crawler::make_get_request(page).await.unwrap();

            assert!(html.contains("Status code 200 body"))
        }
    }

    mod crawl_next_url {
        use reqwest::Url;

        use crate::page::Page;

        use super::super::Crawler;

        #[tokio::test]
        async fn test_books_toscrape() {
            let page = Page::from(Url::parse("https://books.toscrape.com/").unwrap());
            let mut crawler = Crawler::new(page.clone());

            assert_eq!(crawler.queue, vec![page.clone()]);

            crawler.crawl_page(page.clone()).await.unwrap();

            let expected_url = Page::from(
                page.url
                    .join("catalogue/category/books_1/index.html")
                    .unwrap(),
            );

            assert!(crawler.queue.contains(&expected_url));
        }

        #[tokio::test]
        async fn test_already_visited_url() {
            let page = Page::from(Url::parse("https://books.toscrape.com/").unwrap());
            let mut crawler = Crawler::new(page.clone());

            assert_eq!(crawler.queue, vec![page.clone()]);

            // Crawl the page for the first time
            crawler.crawl_page(page.clone()).await.unwrap();

            let queue_before = crawler.queue.clone();

            // Crawl the page a second time. After this, the queue should stay exactly the same.
            crawler.crawl_page(page.clone()).await.unwrap();

            assert_eq!(crawler.queue, queue_before)
        }
    }

    mod extract_urls_from_html {
        use std::{fs::File, io::Read, path::PathBuf};

        use reqwest::Url;

        use crate::page::Page;

        use super::super::Crawler;

        fn test_and_extract_urls_from_html_file(filename: &str, expected_urls: Vec<String>) {
            let page = Page::from(Url::parse("https://does-not-exist.com").unwrap());
            let crawler = Crawler::new(page);

            // CARGO_MANIFEST_DIR gets the source dir of the project
            let html_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join("test-files")
                .join(filename);

            let error_msg = format!("'{}' should exist.", filename);
            let error_msg = error_msg.as_str();
            let mut html = File::open(html_file).expect(error_msg);

            let mut buf = String::new();
            html.read_to_string(&mut buf).unwrap();

            let urls = crawler.extract_urls_from_html(buf);

            assert_eq!(urls, expected_urls)
        }

        #[test]
        fn test_single_href() {
            let filename = "extract_single_href.html";
            let expected_urls = vec![String::from("https://www.wikipedia.org/")];

            test_and_extract_urls_from_html_file(filename, expected_urls);
        }

        #[test]
        fn test_multiple_hrefs() {
            let filename = "extract_multiple_hrefs.html";
            let expected_urls = vec![
                String::from("https://www.wikipedia.org/"),
                String::from("https://www.britannica.com/"),
                String::from("https://www.youtube.com/"),
            ];

            test_and_extract_urls_from_html_file(filename, expected_urls);
        }
    }
}
