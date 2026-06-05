pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn short_uuid(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}
