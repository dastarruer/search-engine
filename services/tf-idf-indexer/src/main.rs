#[cfg(feature = "logging")]
use flexi_logger::FileSpec;

use flexi_logger::{Duplicate, Logger, WriteMode};
use tf_idf_indexer::{Indexer};

#[tokio::main]
async fn main() {
    set_up_logging().await.expect("Log setup should not fail.");

    log::info!("Connecting to the database...");
    let pool = ::utils::init_pool().await;
    log::info!("Succesfully connected to the database!");

    ::utils::migrate(&pool).await;

    let mut indexer = Indexer::new_with_pool(&pool).await;

    log::info!("Running indexer...");
    indexer.run(&pool).await;
    log::info!("Indexer is finished!");
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
