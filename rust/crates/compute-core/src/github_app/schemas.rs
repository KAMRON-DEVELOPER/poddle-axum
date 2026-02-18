use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct InstallationTokenResponse {
    pub token: String,
    pub expires_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstallationReposResponse {
    pub total_count: i64,
    pub repositories: Vec<Repository>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Repository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub default_branch: Option<String>,
    pub clone_url: String,
}
