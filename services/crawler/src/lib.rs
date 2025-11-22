pub mod crawler;
pub mod error;
pub mod page;
pub mod utils;
pub(crate) mod url_handler;
mod db;

// From https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.user_agent
pub const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/dastarruer/search-engine/)"
);

/// The maximum number of pages that the crawler will store in
/// memory at a time.
///
/// The larger this value, the faster the crawler will run.
///
/// Increase this value on systems with more available memory, and decrease it
/// on systems with limited RAM to reduce memory usage.
pub const QUEUE_LIMIT: u32 = 10000;
