#[cfg(feature = "logging")]
use flexi_logger::FileSpec;

use flexi_logger::{Duplicate, Logger, WriteMode};
use sqlx::postgres::PgPoolOptions;
use tf_idf_indexer::{Indexer, utils};

#[tokio::main]
async fn main() {
    set_up_logging().await.expect("Log setup should not fail.");

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

    let mut indexer = Indexer::new_with_pool(&pool).await;
    indexer.run(&pool).await;
}

#[cfg(feature = "logging")]
async fn set_up_logging() -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("logs");

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

// The only differnce with this is that it does not write log output to a file
#[cfg(not(feature = "logging"))]
async fn set_up_logging() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = Logger::try_with_str("info")?
        .duplicate_to_stdout(Duplicate::Info)
        .write_mode(WriteMode::BufferAndFlush)
        .start()?;

    Ok(())
}
