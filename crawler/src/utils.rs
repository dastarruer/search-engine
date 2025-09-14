// TODO: Figure out how to mark this as tests/benchmarks only, so httpmock becomes a dev-dependency
use std::path::PathBuf;

use httpmock::prelude::*;
use url::Url;

/// An implementation of a mock HTTP server.
/// Even though this is public, this method is meant to be used for benchmarks and tests only.
pub struct HttpServer {
    server: MockServer,
}

impl HttpServer {
    pub fn new(filename: &str) -> Self {
        let server = MockServer::start();
        let filepath = test_file_path_from_filename(filename);

        let _mock = server.mock(|when, then| {
            when.method(GET).header("user-agent", crate::USER_AGENT);
            then.status(200)
                .header("content-type", "text/html")
                .body_from_file(filepath.display().to_string());
        });

        HttpServer { server }
    }

    pub fn base_url(&self) -> Url {
        Url::parse(self.server.base_url().as_str()).unwrap()
    }
}

/// Return the path of a file in src/test-files given just its filename.
/// Even though this is public, this method is meant to be used for tests only.
pub(crate) fn test_file_path_from_filename(filename: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR gets the source dir of the project
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("test-files")
        .join(filename)
}

/// Converts a `String` to a `Url`.
/// Returns `None` if an error is encountered while parsing the `String`.
///
/// # Parameters
/// - `base_url` - The URL that the original URL was extracted from.
/// - `url` - The `String` to be converted into a `Url`.
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
