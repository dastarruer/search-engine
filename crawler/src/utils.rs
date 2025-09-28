use std::path::PathBuf;

use httpmock::prelude::*;
use reqwest::StatusCode;
use url::{Url, form_urlencoded};

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

/// Construct a URL to connect to a PostGreSql instance from the following set of environment variables:
/// - DB_USER
/// - DB_PASSWORD
/// - DB_ENDPOINT
/// - DB_PORT
/// - DB_NAME
pub(crate) fn construct_postgres_url() -> String {
    let endpoint = std::env::var("DB_ENDPOINT").expect("DB_ENDPOINT must be set.");
    let port = std::env::var("DB_PORT").expect("DB_PORT must be set.");
    let dbname = std::env::var("DB_NAME").expect("DB_NAME must be set.");
    let user = std::env::var("DB_USER").expect("DB_USER must be set.");
    let password = std::env::var("DB_PASSWORD").expect("DB_PASSWORD must be set.");

    // If the password has special characters like '@' or '#' this will convert
    // them into a URL friendly format
    let encoded_password: String = form_urlencoded::byte_serialize(password.as_bytes()).collect();
    let encoded_user: String = form_urlencoded::byte_serialize(user.as_bytes()).collect();

    format!(
        "postgresql://{}:{}@{}:{}/{}",
        encoded_user, encoded_password, endpoint, port, dbname
    )
}
