use std::{fs, path::PathBuf};

use crawler::crawler::Crawler;
use crawler::page::Page;
use reqwest::Url;

#[cfg(feature = "logging")]
use flexi_logger::{Duplicate, FileSpec, Logger, WriteMode};

#[cfg(feature = "logging")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    set_up_logging().await?;

    let mut crawler = Crawler::new(get_start_urls()).await;

    crawler.run().await?;

    Ok(())
}

#[cfg(not(feature = "logging"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut crawler = Crawler::new(get_start_urls()).await;

    crawler.run().await?;

    Ok(())
}

#[cfg(feature = "logging")]
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

fn get_start_urls() -> Vec<Page> {
    let sites_txt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sites.txt");

    fs::read_to_string(sites_txt_path)
        .unwrap()
        .lines()
        .map(|url| Page::from(Url::parse(url).unwrap()))
        .collect()
}
