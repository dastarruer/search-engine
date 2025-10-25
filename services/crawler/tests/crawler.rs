use std::time::Instant;

use crawler::{USER_AGENT, crawler::Crawler, error::Error, page::Page, utils::HttpServer};
use httpmock::Method::GET;
use reqwest::StatusCode;

mod common;

#[tokio::test]
async fn test_429_status() {
    // special setup (retry-after), keep as is
    const TRY_AFTER_SECS: u64 = 1;
    let filepath = common::test_file_path_from_filepath("dummy.html");

    let server = HttpServer::new_with_mock(|when, then| {
        when.method(GET).header("user-agent", USER_AGENT);
        then.status(StatusCode::TOO_MANY_REQUESTS.as_u16())
            .header("content-type", "text/html")
            .header("retry-after", TRY_AFTER_SECS.to_string())
            .body_from_file(filepath.display().to_string());
    });

    let page = Page::from(server.base_url());
    let crawler = Crawler::new(vec![page.clone()], None).await;

    let start = Instant::now();
    let error = crawler
        .extract_html_from_page(page.clone())
        .await
        .unwrap_err();
    let end = Instant::now();

    let elapsed = (end - start).as_secs();
    assert_eq!(elapsed, TRY_AFTER_SECS);
    assert_eq!(error, Error::RequestTimeout(page))
}
