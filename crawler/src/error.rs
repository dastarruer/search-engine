use thiserror::Error;

use crate::page::Page;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrawlerError {
    #[error("{url} is an empty page with no HTML content.", url = .0.url)]
    EmptyPage(Page),
}
