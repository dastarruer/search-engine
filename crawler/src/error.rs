use reqwest::{StatusCode, header::HeaderValue};
use thiserror::Error;

use crate::page::Page;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrawlerError {
    #[error("{url} is an empty page with no HTML content.", url = .0.url)]
    EmptyPage(Page),
    #[error("Retry-By header for {url} is invalid: {header:?}`.", url = page.url)]
    InvalidRetryByHeader { page: Page, header: HeaderValue },
    #[error("{url} returned {status} status code.", url = page.url)]
    MalformedHttpStatus { page: Page, status: StatusCode },
    #[error("Request to {url} timed out.", url = .0.url)]
    RequestTimeout(Page)
}
