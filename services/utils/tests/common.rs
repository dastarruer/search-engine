use sqlx::{
    Pool,
    postgres::{PgPoolOptions},
};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use utils::migrate;

/// Set up a Postgres Docker container for testing purposes.
///
/// # Returns
/// - A [`ContainerAsync<Postgres>`], which is returned to prevent the
///   container from being dropped.
/// - A [`Pool`], which is a connection to the database within the Docker
///   container.
// TODO: Move this to utils so it can be accessed by other crates
pub async fn setup() -> (ContainerAsync<Postgres>, Pool<sqlx::Postgres>) {
    // Start a database container
    let container = Postgres::default()
        .with_tag("latest")
        .start()
        .await
        .unwrap();

    let db_url = construct_db_url(&container).await;

    let pool = PgPoolOptions::new().connect(&db_url).await.unwrap();

    migrate(&pool).await;

    // Return the container so that it does not get dropped once out of scope
    (container, pool)
}

async fn construct_db_url(container: &ContainerAsync<Postgres>) -> String {
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let host = "127.0.0.1";
    let user = "postgres";
    let password = "postgres";
    let database = "postgres";

    format!("postgres://{user}:{password}@{host}:{port}/{database}")
}
