pub mod crawler;
mod error;
pub mod page;
pub mod utils;

// From https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.user_agent
pub const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/dastarruer/search-engine/)"
);

/// The maximum number of [`page::Page`] instances that can be queued at a time.
///
/// Increase this value on systems with more available memory, and decrease it
/// on systems with limited RAM to reduce memory usage.
pub const QUEUE_LIMIT: i8 = 100;
