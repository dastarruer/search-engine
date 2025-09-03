use reqwest::IntoUrl;
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

#[cfg(test)]
mod test {
    mod make_get_request {
        use super::super::make_get_request;

        // Instead of using #[test], we use #[tokio::test] so we can test async functions
        #[tokio::test]
        async fn test_basic_site() {
            let html = make_get_request("https://crawler-test.com/status_codes/status_200").await.unwrap();

            assert!(html.contains("Status code 200 body"))
        }
    }
}
