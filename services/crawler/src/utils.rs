use httpmock::prelude::*;
use reqwest::StatusCode;
use std::path::PathBuf;

use reqwest::Url;

#[cfg(test)]
use crate::{crawler::Crawler, page::Page};

/// An implementation of a mock HTTP server.
pub struct HttpServer {
    server: MockServer,
}

impl HttpServer {
    pub fn new_with_filepath(filepath: PathBuf) -> Self {
        let server = MockServer::start();

        let _mock = server.mock(|when, then| {
            when.method(GET).header("user-agent", crate::USER_AGENT);
            then.status(StatusCode::OK.as_u16())
                .header("content-type", "text/html")
                .body_from_file(filepath.display().to_string());
        });

        HttpServer { server }
    }

    pub fn new_with_mock(mock: impl FnOnce(httpmock::When, httpmock::Then)) -> Self {
        let server = MockServer::start();

        let _mock = server.mock(mock);

        HttpServer { server }
    }

    pub fn base_url(&self) -> Url {
        let base_url = self.server.base_url();

        let err_msg = format!("Base URL should be a valid url: {}", base_url);
        Url::parse(base_url.as_str()).expect(&err_msg)
    }
}

/// Return the path of a file in src/fixtures given just its filename.
#[cfg(test)]
pub fn test_file_path_from_filepath(filename: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR gets the source dir of the project
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("fixtures")
        .join(filename)
}

/// Converts a `String` to a `Url`.
///
/// # Returns
/// - Returns `None` if an error is encountered while parsing the `String`.
pub(crate) fn string_to_url(base_url: &Url, url: String) -> Option<Url> {
    if url.starts_with("https://") || url.starts_with("http://") {
        match Url::parse(url.as_str()) {
            Ok(url) => Some(url),
            Err(e) => {
                eprintln!("Error: {}", e);
                None
            }
        }
    } else {
        match base_url.join(url.as_str()) {
            Ok(url) => Some(url),
            Err(e) => {
                eprintln!("Error: {}", e);
                None
            }
        }
    }
}

#[cfg(test)]
pub async fn create_crawler(starting_filename: &str) -> (Crawler, Page) {
    let filepath = test_file_path_from_filepath(starting_filename);
    let server = HttpServer::new_with_filepath(filepath);

    let page = Page::from(server.base_url());

    (Crawler::test_new(vec![page.clone()]).await, page)
}
