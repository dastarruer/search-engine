pub mod crawler;
pub mod error;
pub mod page;
pub mod utils;
mod db;

// From https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.user_agent
pub const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/dastarruer/search-engine/)"
);
