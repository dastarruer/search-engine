use std::fs;

use sqlx::{
    Pool,
    postgres::{PgPoolOptions, types::PgHstore},
};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tf_idf_indexer::Term;
use utils::migrate;

/// Set up a Postgres Docker container for testing purposes.
///
/// # Params
/// - `script` - The name of the script to run in the database without the
///   `.sql` file extension (e.g. `"refresh_queue"`). This is used for inserting
///   test data.
///
/// # Returns
/// - A [`ContainerAsync<Postgres>`], which is returned to prevent the
///   container from being dropped.
/// - A [`Pool`], which is a connection to the database within the Docker
///   container.
pub async fn setup(script: &str) -> (ContainerAsync<Postgres>, Pool<sqlx::Postgres>) {
    // Start a database container
    let container = Postgres::default()
        .with_tag("latest")
        .start()
        .await
        .unwrap();

    let db_url = construct_db_url(&container).await;

    let pool = PgPoolOptions::new().connect(&db_url).await.unwrap();

    migrate(&pool).await;
    run_setup_script(script, &pool).await;

    // Return the container so that it does not get dropped once out of scope
    (container, pool)
}

async fn run_setup_script(script: &str, pool: &Pool<sqlx::Postgres>) {
    let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(format!("{}.sql", script).as_str());

    let err_msg = format!("{} should exist.", script_path.to_str().unwrap());
    let query = fs::read_to_string(script_path).expect(&err_msg);

    // If there are multiple commands in the sql script, then run them
    // separately
    for query in query.split(";") {
        sqlx::query(query.trim()).execute(pool).await.unwrap();
    }
}

async fn construct_db_url(container: &ContainerAsync<Postgres>) -> String {
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let host = "127.0.0.1";
    let user = "postgres";
    let password = "postgres";
    let database = "postgres";

    format!("postgres://{user}:{password}@{host}:{port}/{database}")
}

pub fn dummy_terms() -> Vec<Term> {
    // ladder
    let expected_ladder_tf = PgHstore::from_iter([
        ("1".to_string(), Some("2".to_string())),
        ("2".to_string(), Some("1".to_string())),
        ("3".to_string(), Some("1".to_string())),
    ]);
    let expected_ladder_tf_idf = PgHstore::from_iter([
        ("1".to_string(), Some("0".to_string())),
        ("2".to_string(), Some("0".to_string())),
        ("3".to_string(), Some("0".to_string())),
    ]);

    // hippopotamus
    let expected_hippo_tf = PgHstore::from_iter([
        ("2".to_string(), Some("2".to_string())),
        ("3".to_string(), Some("2".to_string())),
    ]);
    let expected_hippo_tf_idf = PgHstore::from_iter([
        ("2".to_string(), Some("0.3521825".to_string())),
        ("3".to_string(), Some("0.3521825".to_string())),
    ]);

    // pipe
    let expected_pipe_tf = PgHstore::from_iter([
        ("1".to_string(), Some("1".to_string())),
    ]);
    let expected_pipe_tf_idf = PgHstore::from_iter([
        ("1".to_string(), Some("0.47712123".to_string())),
    ]);

    vec![
        Term::new(
            "ladder".into(),
            ordered_float::OrderedFloat(0.0),
            3,
            expected_ladder_tf,
            expected_ladder_tf_idf,
        ),
        Term::new(
            "hippopotamus".into(),
            ordered_float::OrderedFloat(0.17609125),
            2,
            expected_hippo_tf,
            expected_hippo_tf_idf,
        ),
        Term::new(
            "pipe".into(),
            ordered_float::OrderedFloat(0.47712123),
            1,
            expected_pipe_tf,
            expected_pipe_tf_idf,
        ),
    ]
}
