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
        let filepath = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("test-files")
            .join(filename);

        let _mock = server.mock(|when, then| {
            when.method(GET);
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
