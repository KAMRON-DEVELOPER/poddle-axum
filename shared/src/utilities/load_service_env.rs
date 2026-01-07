use std::path::PathBuf;
use tracing::{info, warn};

pub fn load_service_env() -> () {
    // CARGO_MANIFEST_DIR is baked at COMPILE time.
    // In Dev: It points to "services/users-api.
    // In Prod: It points to a path that likely doesn't exist in the container.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".into());

    let filename = match env.as_str() {
        "production" => ".env.production",
        "staging" => ".env.staging",
        _ => ".env",
    };

    let candidate = manifest_dir.join(filename);

    match dotenvy::from_path(&candidate) {
        Ok(_) => {
            info!("✅ Loaded .env from {}", candidate.display());
        }
        Err(e) => {
            // In Production, this error is EXPECTED because the file won't exist.
            // We assume K8s has already set the variables.
            warn!(
                "⚠️ No .env file found at {}. Assuming K8s/System variables are set.",
                candidate.display()
            );
            warn!("Error detail: {}", e);
        }
    }
}
