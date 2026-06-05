pub fn resolve_author(user_flag: Option<String>) -> String {
    let username = user_flag
        .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()));
    format!("user:{username}")
}
