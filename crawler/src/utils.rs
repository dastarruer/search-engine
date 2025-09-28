use std::path::PathBuf;

use httpmock::prelude::*;
use reqwest::StatusCode;
use url::Url;

/// An implementation of a mock HTTP server.
#[cfg(any(test, feature = "bench"))]
pub struct HttpServer {
    server: MockServer,
}

#[cfg(any(test, feature = "bench"))]
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
        Url::parse(self.server.base_url().as_str()).unwrap()
    }
}

/// Return the path of a file in src/test-files given just its filename.
#[cfg(test)]
pub fn test_file_path_from_filepath(filename: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR gets the source dir of the project
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("test-files")
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
