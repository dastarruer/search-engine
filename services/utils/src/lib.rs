/// Run database migrations on the database.
///
/// # Panics
/// This method panics if:
/// - Running the migrations throws an error
pub async fn migrate(pool: &sqlx::PgPool) {
    sqlx::migrate!("../../migrations").run(pool).await.expect("Database migrations should not throw an error.");
}
