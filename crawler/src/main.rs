use std::path::PathBuf;

use crawler::crawler::Crawler;
use crawler::page::Page;
use ftail::Ftail;
use log::LevelFilter;
use reqwest::Url;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    set_up_logging().await?;

    let mut crawler = Crawler::new(Page::from(
        Url::parse("https://en.wikipedia.org/wiki/Poland").unwrap(),
    ))
    .await;

    crawler.run().await?;

    Ok(())
}

async fn set_up_logging() -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("logs");

    if !log_dir.exists() {
        fs::create_dir(log_dir.clone()).await?;
    }

    let error_log_path = log_dir.join("errors.log");

    Ftail::new()
        .formatted_console(LevelFilter::Info)
        .single_file(&error_log_path, true, LevelFilter::Error)
        .init()?;

    Ok(())
}
