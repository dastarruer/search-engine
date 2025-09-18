use std::path::PathBuf;

use crawler::crawler::Crawler;
use crawler::page::Page;
use flexi_logger::{Duplicate, FileSpec, Logger, WriteMode};
use reqwest::Url;

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

    let _logger = Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory(log_dir)
                .suppress_basename()
                .suffix("log"),
        )
        .duplicate_to_stdout(Duplicate::Info)
        .write_mode(WriteMode::BufferAndFlush)
        .start()?;

    Ok(())
}
