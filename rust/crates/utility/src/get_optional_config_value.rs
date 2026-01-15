use std::path::{Path, PathBuf};
use std::str::FromStr;
use tokio::fs;

pub async fn get_optional_config_value<T>(
    secret_name: &str,
    env_name: Option<&str>,
    fallback_path: Option<&PathBuf>,
) -> Option<T>
where
    T: FromStr,
{
    // Docker secret
    let docker_secret = Path::new("/run/secrets").join(secret_name);
    if docker_secret.exists() {
        if let Ok(content) = fs::read_to_string(&docker_secret).await {
            if let Ok(parsed) = T::from_str(content.trim()) {
                return Some(parsed);
            }
        }
    }

    // Env var
    if let Some(env_key) = env_name
        && let Ok(val) = dotenvy::var(env_key)
    {
        if let Ok(parsed) = T::from_str(val.trim()) {
            return Some(parsed);
        }
    }

    // Fallback path
    if let Some(path) = fallback_path
        && path.exists()
    {
        if let Ok(content) = fs::read_to_string(path).await {
            if let Ok(parsed) = T::from_str(content.trim()) {
                return Some(parsed);
            }
        }
    }

    None
}
