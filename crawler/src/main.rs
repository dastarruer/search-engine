mod crawler;

use tokio;

use crate::crawler::Crawler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let html = Crawler::make_get_request("https://www.rust-lang.org").await.unwrap();

    println!("{html:#?}");
    Ok(())
}



