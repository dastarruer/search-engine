mod crawler;

use reqwest::Url;

use crate::crawler::Crawler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut crawler = Crawler::new(Url::parse("https://books.toscrape.com/").unwrap());

    crawler.run().await?;

    Ok(())
}
