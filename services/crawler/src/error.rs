use reqwest::Url;
use reqwest::{StatusCode, header::HeaderValue};
use thiserror::Error;

use crate::page::Page;

#[derive(Debug, Error, PartialEq)]
pub enum Error {
    #[error("Request to {url} failed: {error_str}", url = page.url)]
    FailedRequest { page: Page, error_str: String },

    #[error("{url} is an empty page with no HTML content.", url = .0.url)]
    EmptyPage(Page),

    #[error("Retry-By header for {url} is invalid: {header:?}`.", url = page.url)]
    InvalidRetryByHeader {
        page: Page,
        header: Option<HeaderValue>,
    },

    #[error("{url} returned {status} status code.", url = page.url)]
    MalformedHttpStatus { page: Page, status: StatusCode },

    #[error("Request to {url} timed out.", url = .0.url)]
    RequestTimeout(Page),

    #[error("HTML decoding error from {url}: {error_str}.")]
    HtmlDecoding { url: Url, error_str: String },

    #[error("{url} is a non-English site.", url = .0.url)]
    NonEnglishPage(Page),

    #[error("{url} is an adult site.", url = .0.url)]
    InappropriateSite(Page),

    #[error("{url} contains an invalid domain.", url = .0)]
    InvalidDomain(Url),
}

// Convert a Box<Error> to an Error so error propogation with `?` works
impl From<std::boxed::Box<Error>> for Error {
    fn from(value: std::boxed::Box<Error>) -> Self {
        *value
    }
}
