use reqwest::header::HeaderValue;
use thiserror::Error;

use crate::page::Page;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrawlerError {
    #[error("{url} is an empty page with no HTML content.", url = .0.url)]
    EmptyPage(Page),
    #[error("Retry-By header for {url} is invalid: {header:?}`.", url = page.url)]
    InvalidRetryByHeader{page: Page, header: HeaderValue},
}
