use crawler::crawler::Crawler;
use reqwest::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut crawler = Crawler::new(Url::parse("https://books.toscrape.com/").unwrap());

    crawler.run().await?;

    Ok(())
}
