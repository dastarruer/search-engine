use std::collections::VecDeque;

use reqwest::{IntoUrl, Url};
use scraper::{Html, Selector};

pub struct Crawler {
    queue: VecDeque<Url>,
    visited: VecDeque<Url>,
}

impl Crawler {
    pub fn new(starting_url: Url) -> Self {
        let mut queue = VecDeque::new();
        let visited = VecDeque::new();

        queue.push_back(starting_url);

        Crawler { queue, visited }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(url) = self.queue.pop_back() {
            self.crawl_from_url(url).await.unwrap();
        }

        Ok(())
    }

    // TODO: Make this private somehow, since this needs to be public for benchmarks
    pub async fn crawl_from_url(&mut self, url: Url) -> Result<(), Box<dyn std::error::Error>> {
        let html = Self::make_get_request(url.clone()).await?;
        let urls = self.extract_urls_from_html(html);

        let base_url = url;

        for url in urls {
            let url = if url.starts_with("https://") || url.starts_with("http://") {
                Url::parse(url.as_str()).unwrap()
            } else {
                base_url.join(url.as_str()).unwrap()
            };

            if self.is_url_visited(&url) {
                continue;
            }

            self.mark_url_as_visited(url.clone());
            self.queue.push_front(url);
        }
        println!("Crawled {:?}...", base_url);
        Ok(())
    }

    fn mark_url_as_visited(&mut self, url: Url) {
        self.visited.push_front(url.clone());
    }

    fn is_url_visited(&self, url: &Url) -> bool {
        self.visited.contains(url)
    }

    /// Make a get request to a specific URL.
    /// This (should) return the HTML of the URL.
    async fn make_get_request(url: impl IntoUrl) -> Result<String, Box<dyn std::error::Error>> {
        Ok(reqwest::get(url).await?.text().await?)
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
        use super::super::Crawler;

        // Instead of using #[test], we use #[tokio::test] so we can test async functions
        #[tokio::test]
        async fn test_basic_site() {
            let html =
                Crawler::make_get_request("https://crawler-test.com/status_codes/status_200")
                    .await
                    .unwrap();

            assert!(html.contains("Status code 200 body"))
        }
    }

    mod crawl_next_url {
        use reqwest::Url;

        use super::super::Crawler;

        #[tokio::test]
        async fn test_books_toscrape() {
            let url = Url::parse("https://books.toscrape.com/").unwrap();
            let mut crawler = Crawler::new(url.clone());

            assert_eq!(crawler.queue, vec![url.clone()]);

            crawler.crawl_from_url(url.clone()).await.unwrap();

            let expected_url = url.join("catalogue/category/books_1/index.html").unwrap();

            assert!(crawler.queue.contains(&expected_url));
        }

        #[tokio::test]
        async fn test_already_visited_url() {
            let url = Url::parse("https://books.toscrape.com/").unwrap();
            let mut crawler = Crawler::new(url.clone());

            assert_eq!(crawler.queue, vec![url.clone()]);

            // Crawl the url. This will fill up crawler.visited
            crawler.crawl_from_url(url.clone()).await.unwrap();

            crawler.queue.clear();

            // After clearing the url queue, we crawl the url again.
            // Since crawler.visited is filled, the queue should be empty after crawling the same url again
            crawler.crawl_from_url(url.clone()).await.unwrap();

            assert!(crawler.queue.is_empty());
        }
    }

    mod extract_urls_from_html {
        use std::{fs::File, io::Read, path::PathBuf};

        use reqwest::Url;

        use super::super::Crawler;

        fn test_and_extract_urls_from_html_file(filename: &str, expected_urls: Vec<String>) {
            let url = Url::parse("https://does-not-exist.com").unwrap();
            let crawler = Crawler::new(url);

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
