use reqwest::IntoUrl;
use scraper::{Html, Selector};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let html = make_get_request("https://www.rust-lang.org").await.unwrap();

    println!("{html:#?}");
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

#[cfg(test)]
mod test {
    mod make_get_request {
        use super::super::make_get_request;

        // Instead of using #[test], we use #[tokio::test] so we can test async functions
        #[tokio::test]
        async fn test_basic_site() {
            let html = make_get_request("https://crawler-test.com/status_codes/status_200")
                .await
                .unwrap();

            assert!(html.contains("Status code 200 body"))
        }
    }

    mod extract_urls_from_html {
        use std::{fs::File, io::Read, path::PathBuf};

        use super::super::extract_urls_from_html;

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

            let urls = extract_urls_from_html(buf);

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
