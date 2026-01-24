use serde::de::DeserializeOwned;

use crate::get_optional_config_value::get_optional_config_value;
use std::path::PathBuf;

pub async fn get_config_value<T>(
    secret_name: &str,
    env_name: Option<&str>,
    fallback_path: Option<&PathBuf>,
    fallback: Option<T>,
) -> T
where
    T: DeserializeOwned + Clone,
{
    if let Some(value) = get_optional_config_value::<T>(secret_name, env_name, fallback_path).await
    {
        return value;
    }

    fallback.unwrap_or_else(|| {
        panic!(
            "Configuration value '{}' not set (env, secret, or fallback path)",
            secret_name
        )
    })
}
