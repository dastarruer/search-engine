use scraper::{Html, Selector};

/// Run database migrations on the database.
///
/// # Panics
/// This method panics if:
/// - Running the migrations throws an error
pub async fn migrate(pool: &sqlx::PgPool) {
    sqlx::migrate!("../../migrations")
        .run(pool)
        .await
        .expect("Database migrations should not throw an error.");
}

/// Create a connection to the database from the following set of environment
/// variables:
/// - `DB_USER`
/// - `DB_PASSWORD`
/// - `DB_ENDPOINT`
/// - `DB_PORT`
/// - `DB_NAME`
pub async fn init_pool() -> sqlx::PgPool {
    let url = construct_postgres_url();
    let url = url.as_str();

    let max_connections = 10;
    let min_connections = 2;

    // Set a large connection timeout, since as the size of the db increases, queries take longer and longer to execute
    let connection_timeout = std::time::Duration::from_secs(500);

    let max_lifetime = Some(std::time::Duration::from_secs(1800));
    let idle_timeout = Some(std::time::Duration::from_secs(600));

    sqlx::postgres::PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(connection_timeout) // connection timeout
        .max_lifetime(max_lifetime) // recycle old connections
        .idle_timeout(idle_timeout) // close idle connections
        .connect(url) // async connect
        .await
        .expect("DATABASE_URL should correctly point to the PostGreSQL database.")
}

/// Construct a URL to connect to a Postgres instance from the following set
/// of environment variables:
/// - `DB_USER`
/// - `DB_PASSWORD`
/// - `DB_ENDPOINT`
/// - `DB_PORT`
/// - `DB_NAME`
fn construct_postgres_url() -> String {
    let endpoint = retrieve_env_var("DB_ENDPOINT");
    let port = retrieve_env_var("DB_PORT");
    let dbname = retrieve_env_var("DB_NAME");
    let user = retrieve_env_var("DB_USER");
    let password = retrieve_env_var("DB_PASSWORD");

    // If the password has special characters like '@' or '#' this will convert
    // them into a URL friendly format
    let encoded_password: String =
        url::form_urlencoded::byte_serialize(password.as_bytes()).collect();
    let encoded_user: String = url::form_urlencoded::byte_serialize(user.as_bytes()).collect();

    format!(
        "postgresql://{}:{}@{}:{}/{}",
        encoded_user, encoded_password, endpoint, port, dbname
    )
}

fn retrieve_env_var(var: &str) -> String {
    let error_msg = format!("{} must be set.", var);
    let error_msg = error_msg.as_str();
    std::env::var(var).expect(error_msg)
}

pub trait AddToDb {
    /// Add an object to a database.
    #[allow(async_fn_in_trait)]
    async fn add_to_db(&self, pool: &sqlx::PgPool);
}

/// Extract all visible text from a parsed [`Html`] document.
///
/// 'Visible text' means any text that the user can read if they go onto a
/// page. For instance, the text of a Wikipedia article is considered
/// visible text, while any Javascript or CSS is not.
pub fn extract_text(html: &Html) -> String {
    html.select(&Selector::parse("body p").unwrap()) // or "body div#foo div.inner"
        .flat_map(|el| el.text())
        .collect()
}

#[cfg(test)]
mod test {
    use scraper::Html;

    use crate::extract_text;

    #[test]
    fn test_extract_text() {
        let html = Html::parse_document(
            r#"
            <body>
                <style>
                    .global-navigation{
                        position: fixed;
                    }
                </style>

                <script>
                    let code = "hello world";
                </script>
                <p>hippopotamus hippopotamus hippopotamus</p>
            </body>"#,
        );

        assert_eq!(
            extract_text(&html),
            "hippopotamus hippopotamus hippopotamus"
        )
    }
}
