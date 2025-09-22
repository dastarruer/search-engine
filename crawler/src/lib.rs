pub mod crawler;
pub mod page;
pub mod utils;
mod error;

// From https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.user_agent
pub const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/dastarruer/search-engine/)"
);
