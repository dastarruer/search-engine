use testcontainers_modules::{
    postgres::{Postgres},
    testcontainers::{ImageExt, runners::AsyncRunner},
};

/// Set up a Postgres Docker container for testing purposes.
pub async fn setup() {
    Postgres::default()
        .with_tag("latest")
        .with_env_var(
            String::from("DATABASE_URL"),
            String::from("postgres://postgres:postgres@127.0.0.1:5432/postgres"),
        )
        .start().await.unwrap();
}
