use crate::get_optional_config_value::get_optional_config_value;
use std::path::PathBuf;
use std::str::FromStr;

pub async fn get_config_value<T>(
    secret_name: &str,
    env_name: Option<&str>,
    fallback_path: Option<&PathBuf>,
    fallback: Option<T>,
) -> T
where
    T: FromStr + Clone,
{
    if let Some(value) = get_optional_config_value::<T>(secret_name, env_name, fallback_path).await
    {
        return value;
    }

    fallback.expect(format!("Environment variable {} not set", secret_name).as_str())
}
