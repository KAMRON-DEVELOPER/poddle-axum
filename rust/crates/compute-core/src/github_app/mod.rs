use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod error;
pub mod implementations;
pub mod schemas;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GithubAppConfig {
    pub client_id: String,
    pub client_secret: String,
    pub public_link: String,
    pub private_key_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GithubAppClaims {
    // Issued at time
    pub iat: i64,
    // JWT expiration time
    pub exp: i64,
    // GitHub App's client ID
    pub iss: String,
}

#[derive(Clone, Debug)]
pub struct GithubApp {
    pub cfg: GithubAppConfig,
}
