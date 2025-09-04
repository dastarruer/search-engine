use std::collections::VecDeque;

use reqwest::{IntoUrl, Url};
use scraper::{Html, Selector};

pub struct Crawler {
    queue: VecDeque<Url>,
}

impl Crawler {
    pub fn new(starting_url: Url) -> Self {
        let mut queue = VecDeque::new();
        queue.push_back(starting_url);

        Crawler { queue }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for _ in self.queue.clone() {
            self.crawl_next_url().await.unwrap();
        }

        Ok(())
    }

    async fn crawl_next_url(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let url = self.queue.pop_back().unwrap();

        let html = Self::make_get_request(url).await?;

        let urls = Self::extract_urls_from_html(html);

        for url in urls {
            self.queue.push_front(Url::parse(&url.to_string()).unwrap());
        }

        Ok(())
    }

    /// Make a get request to a specific URL.
    /// This (should) return the HTML of the URL.
    async fn make_get_request(url: impl IntoUrl) -> Result<String, Box<dyn std::error::Error>> {
        Ok(reqwest::get(url).await?.text().await?)
    }

    fn extract_urls_from_html(html: String) -> Vec<Url> {
        let mut urls = vec![];

        let fragment = Html::parse_fragment(html.as_str());
        let selector = Selector::parse("a").unwrap();

        for element in fragment.select(&selector) {
            if let Some(url) = element.value().attr("href") {
                urls.push(Url::parse(url).unwrap());
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

            crawler.crawl_next_url().await.unwrap();

            let expected_url = url.join("catalogue/category/books_1/index.html").unwrap();

            assert!(crawler.queue.contains(&expected_url))
        }
    }

    mod extract_urls_from_html {
        use std::{fs::File, io::Read, path::PathBuf};

        use reqwest::Url;

        use super::super::Crawler;

        fn test_and_extract_urls_from_html_file(filename: &str, expected_urls: Vec<Url>) {
            // CARGO_MANIFEST_DIR gets the source dir of the project
            let html_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join("test-files")
                .join(filename);

            let mut html =
                File::open(html_file).expect(format!("'{}' should exist.", filename).as_str());

            let mut buf = String::new();
            html.read_to_string(&mut buf).unwrap();

            let urls = Crawler::extract_urls_from_html(buf);

            assert_eq!(urls, expected_urls)
        }

        #[test]
        fn test_single_href() {
            let filename = "extract_single_href.html";
            let expected_urls = vec![Url::parse("https://www.wikipedia.org/").unwrap()];

            test_and_extract_urls_from_html_file(filename, expected_urls);
        }

        #[test]
        fn test_multiple_hrefs() {
            let filename = "extract_multiple_hrefs.html";
            let expected_urls = vec![
                Url::parse("https://www.wikipedia.org/").unwrap(),
                Url::parse("https://www.britannica.com/").unwrap(),
                Url::parse("https://www.youtube.com/").unwrap(),
            ];

            test_and_extract_urls_from_html_file(filename, expected_urls);
        }
    }
}
