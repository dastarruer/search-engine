use std::collections::HashMap;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resp = reqwest::get("https://www.rust-lang.org")
        .await?
        .text()
        .await?;

    println!("{resp:#?}");
    Ok(())
}
