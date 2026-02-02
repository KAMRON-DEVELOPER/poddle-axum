use uuid::Uuid;

pub fn format_namespace(user_id: &Uuid) -> String {
    format!(
        "user-{}",
        user_id
            .as_simple()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>()
    )
}

/// generate resource name like `app-{id[:8]}`
pub fn format_resource_name(id: &Uuid) -> String {
    format!(
        "app-{}",
        id.as_simple()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>()
    )
}
