use utils::migrate;

mod common;

#[tokio::test]
async fn test_migrations() {
    // Run migrations once
    let (_container, pool) = common::setup().await;

    // Run migrations again. Migrations should be written so that they don't
    // panic if run multiple times
    migrate(&pool).await;
}
