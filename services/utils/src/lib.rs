use once_cell::sync::Lazy;
use scraper::{Html, Node, Selector};

/// The maximum number of pages that both the crawler and indexer can store in
/// memory at a time.
///
/// The larger this value, the faster each service will run.
///
/// Increase this value on systems with more available memory, and decrease it
/// on systems with limited RAM to reduce memory usage.
// TODO: Make this two separate constants, one for the indexer and one for the crawler
pub const QUEUE_LIMIT: u32 = 500000;

static TEXT_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("body p, pa, p abbr, p acronym, p b, p bdo, p big, p button, p cite, p code, p dfn, p em, p i, p kbd, p label, p output, p q, p samp, p small, p span, p strong, p sub, p sup, p time, p tt, p var, h1, h2, h3, h4, h5, h6, ul li, ol li").unwrap()
});
static IMAGE_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("img").unwrap());

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

pub trait ExtractText {
    fn extract_text(&self) -> String;
}

impl ExtractText for Html {
    /// Extract all visible and image alt text from a parsed [`Html`] document.
    /// Each word is separated by whitespace, and punctuation is preserved.
    ///
    /// 'Visible text' means any text that the user can read if they go onto a
    /// page. For instance, the text of a Wikipedia article is considered
    /// visible text, while any Javascript or CSS is not.
    ///
    /// Alt text gets appended to the end of the return `String` with a space.
    fn extract_text(&self) -> String {
        let mut content = self
            .select(&TEXT_SELECTOR) // Select all tags with relevant text
            .flat_map(|el| {
                // Loop through each child node in the element
                el.children().filter_map(|node| {
                    // If the node has text, return the text
                    if let Node::Text(t) = node.value() {
                        if !t.trim().is_empty() {
                            Some(t.trim().to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<_>>()
            .join(" ");

        // Get alt text
        let alt_text = self
            .select(&IMAGE_SELECTOR)
            .flat_map(|el| el.value().attr("alt"))
            .collect::<Vec<_>>()
            .join(" ");

        // Add a space at the end of the string to seperate the alt text from the regular content
        if !alt_text.is_empty() {
            content.push(' ');
            content.push_str(alt_text.as_str());
        }

        content.trim().to_string()
    }
}

#[cfg(test)]
mod test {
    use scraper::Html;

    use crate::ExtractText;

    mod extract_text {
        use super::*;

        #[test]
        fn test_extract_text() {
            let html = Html::parse_document(
                r#"
                <body>
                    <p>hippopotamus hippopotamus hippopotamus</p>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus"
            );
        }

        #[test]
        fn test_nested_tags() {
            let html = Html::parse_document(
                r#"
                <body>
                    <p>hippopotamus <h1>hippopotamus <p>hippopotamus</p></h1></p>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus"
            );
        }

        #[test]
        fn test_header_tags() {
            let html = Html::parse_document(
                r#"
                <body>
                    <h1>hippopotamus</h1>
                    <h2>hippopotamus</h2>
                    <h3>hippopotamus</h3>
                    <h4>hippopotamus</h4>
                    <h5>hippopotamus</h5>
                    <h6>hippopotamus</h6>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus hippopotamus hippopotamus hippopotamus"
            );
        }

        #[test]
        fn test_unordered_list_tags() {
            let html = Html::parse_document(
                r#"
                <body>
                    <ul>
                        <li>hippopotamus</li>
                        <li>hippopotamus</li>
                        <li>hippopotamus</li>
                    </ul>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus"
            );
        }

        #[test]
        fn test_ordered_list_tags() {
            let html = Html::parse_document(
                r#"
                <body>
                    <ol>
                        <li>hippopotamus</li>
                        <li>hippopotamus</li>
                        <li>hippopotamus</li>
                    </ol>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus"
            );
        }

        #[test]
        fn test_nested_list_tags() {
            let html = Html::parse_document(
                r#"
                <body>
                    <ul>
                        <li>hippopotamus
                            <ul>
                                <li>hippopotamus</li>
                                <li>hippopotamus</li>
                            </ul>
                        </li>
                    </ul>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus"
            );
        }

        #[test]
        fn test_with_style_and_script_tags() {
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
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus"
            );
        }

        #[test]
        fn test_img_alt_text() {
            let html = Html::parse_document(
                r#"
                <body>
                    <img src="man_on_building.jpg" alt="A man on a building">
                    <p>hippopotamus hippopotamus hippopotamus</p>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus A man on a building"
            );
        }

        #[test]
        fn test_with_punctuation() {
            let html = Html::parse_document(
                r#"
                <html></html>

                <body>
                    <p>hippopotamus hippopotamus, Hippopotamus</p>
                    <p>hippopotamus world tis the won</p>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus, Hippopotamus hippopotamus world tis the won"
            );
        }

        #[test]
        fn test_inline_elements() {
            let html = Html::parse_document(
                r#"
                <html></html>

                <body>
                    <p><b>hippopotamus</b> <span>hippopotamus</span> <i>hippopotamus</i></p>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus"
            );
        }

        #[test]
        fn test_nested_inline_elements() {
            let html = Html::parse_document(
                r#"
                <html></html>

                <body>
                    <p><span><strong>hippopotamus</strong></span> hippopotamus <i>hippopotamus</i></p>
                </body>"#,
            );

            assert_eq!(
                html.extract_text(),
                "hippopotamus hippopotamus hippopotamus"
            );
        }
    }
}
