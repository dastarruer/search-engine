use std::collections::{HashMap, HashSet};

use scraper::Html;
use sqlx::postgres::PgPoolOptions;
use tf_idf_indexer::{Indexer, Page, utils};

#[tokio::main]
async fn main() {
    let mut starting_pages = HashSet::new();

    for i in 0..4 {
        let id = i;

        let page = Page::new(
            Html::parse_document(
                r#"
            <body>
                <p>hippopotamus hippopotamus hippopotamus</p>
            </body>"#,
            ),
            id,
        );

        starting_pages.insert(page);
    }

    let mut indexer = Indexer::new(HashMap::new(), starting_pages);

    let url = utils::construct_postgres_url();
    let url = url.as_str();

    let max_connections = 10;
        let min_connections = 2;

        let connection_timeout = std::time::Duration::from_secs(5);

        let max_lifetime = Some(std::time::Duration::from_secs(1800));
        let idle_timeout = Some(std::time::Duration::from_secs(600));

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .acquire_timeout(connection_timeout) // connection timeout
            .max_lifetime(max_lifetime) // recycle old connections
            .idle_timeout(idle_timeout) // close idle connections
            .connect(url) // async connect
            .await
            .expect("DATABASE_URL should correctly point to the PostGreSQL database.");

    indexer.run(&pool).await;
}
