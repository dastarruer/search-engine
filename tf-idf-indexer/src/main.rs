use sqlx::postgres::PgPoolOptions;
use tf_idf_indexer::{Indexer, utils};

#[tokio::main]
async fn main() {
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

    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .expect("Migrations should not throw any errors.");

    let mut indexer = Indexer::new_with_pool(&pool).await;
    indexer.run(&pool).await;
}
