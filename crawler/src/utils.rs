use std::path::PathBuf;

use httpmock::prelude::*;
use reqwest::{StatusCode, Url};

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
pub(crate) fn construct_postgres_url() -> Url {
    let endpoint = retrieve_env_var("DB_ENDPOINT");
    let port = retrieve_env_var("DB_PORT");
    let dbname = retrieve_env_var("DB_NAME");
    let user = retrieve_env_var("DB_USER");
    let password = retrieve_env_var("DB_PASSWORD");

    // Start with a base URL
    let base_url = format!("postgresql://{}:{}/{}", endpoint, port, dbname);
    let mut url = Url::parse(base_url.as_str()).expect("Failed to build base URL");

    // Insert encoded username and password
    url.set_username(&user).expect("Invalid username");
    url.set_password(Some(&password)).expect("Invalid password");

    url
}

fn retrieve_env_var(var: &str) -> String {
    let error_msg = format!("{} must be set.", var);
    let error_msg = error_msg.as_str();
    std::env::var(var).expect(error_msg)
}
