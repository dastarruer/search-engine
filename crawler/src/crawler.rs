use std::collections::VecDeque;

use reqwest::IntoUrl;
use scraper::{Html, Selector};

pub struct Crawler {
    queue: VecDeque<String>,
}

impl Crawler {
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for _ in self.queue.clone() {
            let url = self.queue.pop_back().unwrap();

            let html = Self::make_get_request(url).await?;

            let urls = Self::extract_urls_from_html(html);

            for url in urls {
                self.queue.push_front(url);
            }
        }

        Ok(())
    }

    /// Make a get request to a specific URL.
    /// This (should) return the HTML of the URL.
    async fn make_get_request(url: impl IntoUrl) -> Result<String, Box<dyn std::error::Error>> {
        Ok(reqwest::get(url).await?.text().await?)
    }

    fn extract_urls_from_html(html: String) -> Vec<String> {
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

    mod extract_urls_from_html {
        use std::{fs::File, io::Read, path::PathBuf};

        use super::super::Crawler;

        fn test_and_extract_urls_from_html_file(filename: &str, expected_urls: Vec<String>) {
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
            let expected_urls = vec!["https://www.wikipedia.org/".to_string()];

            test_and_extract_urls_from_html_file(filename, expected_urls);
        }

        #[test]
        fn test_multiple_hrefs() {
            let filename = "extract_multiple_hrefs.html";
            let expected_urls = vec![
                "https://www.wikipedia.org/".to_string(),
                "https://www.britannica.com/".to_string(),
                "https://www.youtube.com/".to_string(),
            ];

            test_and_extract_urls_from_html_file(filename, expected_urls);
        }
    }
}
