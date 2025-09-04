mod crawler;

use tokio;

use crate::crawler::Crawler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut crawler = Crawler::new("https://books.toscrape.com/".to_string());

    crawler.run().await?;

    Ok(())
}
