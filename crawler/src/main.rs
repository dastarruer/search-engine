use crawler::crawler::Crawler;
use crawler::page::Page;
use reqwest::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut crawler = Crawler::new(Page::from(
        Url::parse("https://books.toscrape.com/").unwrap(),
    ));

    crawler.run().await?;

    Ok(())
}
