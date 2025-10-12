use url::form_urlencoded;

/// Construct a URL to connect to a PostGreSql instance from the following set of environment variables:
/// - DB_USER
/// - DB_PASSWORD
/// - DB_ENDPOINT
/// - DB_PORT
/// - DB_NAME
pub(crate) fn construct_postgres_url() -> String {
    let endpoint = retrieve_env_var("DB_ENDPOINT");
    let port = retrieve_env_var("DB_PORT");
    let dbname = retrieve_env_var("DB_NAME");
    let user = retrieve_env_var("DB_USER");
    let password = retrieve_env_var("DB_PASSWORD");

    // If the password has special characters like '@' or '#' this will convert
    // them into a URL friendly format
    let encoded_password: String = form_urlencoded::byte_serialize(password.as_bytes()).collect();
    let encoded_user: String = form_urlencoded::byte_serialize(user.as_bytes()).collect();

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
