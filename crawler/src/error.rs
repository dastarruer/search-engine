use reqwest::{StatusCode, header::HeaderValue};
use thiserror::Error;

use crate::page::Page;

#[derive(Debug, Error, PartialEq)]
pub enum CrawlerError {
    #[error("Request failed: {0}")]
    FailedRequest(String),

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

    #[error("`{0}`")]
    HtmlDecodingError(String),
}
