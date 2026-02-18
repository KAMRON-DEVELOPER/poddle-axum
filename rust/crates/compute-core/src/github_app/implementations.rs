use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::Client;
use std::{
    fs::{self},
    io::Error,
    path::PathBuf,
};

use chrono::Utc;

use crate::github_app::{
    GithubApp, GithubAppClaims,
    error::GithubAppError,
    schemas::{InstallationReposResponse, InstallationTokenResponse, Repository},
};

impl GithubApp {
    pub fn generate_jwt(&self) -> Result<String, GithubAppError> {
        let iat = Utc::now().timestamp();
        let exp = (Utc::now() + chrono::Duration::minutes(10)).timestamp();
        let iss = self.cfg.client_id.clone();

        let claims = GithubAppClaims { iat, exp, iss };

        let header = Header::new(Algorithm::RS256);
        let key = self
            .read_private_key(&self.cfg.private_key_path)
            .unwrap_or_else(|e| {
                panic!(
                    "Couldn't read {} at {:?}: {}",
                    "Github App Private Key", self.cfg.private_key_path, e
                )
            });
        let encoding_key = EncodingKey::from_rsa_pem(key.as_bytes())?;
        Ok(encode(&header, &claims, &encoding_key)?)
    }

    /// Read the file content as a string
    fn read_private_key(&self, path: &PathBuf) -> Result<String, Error> {
        fs::read_to_string(path)
    }

    pub async fn create_installation_token(
        &self,
        installation_id: i64,
        http: &Client,
    ) -> Result<String, GithubAppError> {
        // POST /app/installations/{installation_id}/access_tokens
        let jwt = self.generate_jwt()?;

        let res = http
            .post(format!(
                "https://api.github.com/app/installations/{}/access_tokens",
                installation_id
            ))
            .header("Authorization", format!("Bearer {}", jwt))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "poddle-compute")
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(GithubAppError::BadRequest(format!(
                "GitHub access_tokens failed: {}",
                res.status()
            )));
        }

        let res = res.json::<InstallationTokenResponse>().await?;
        Ok(res.token)
    }

    pub async fn list_installation_repos(
        &self,
        access_token: &str,
        http: &Client,
    ) -> Result<(Vec<Repository>, i64), GithubAppError> {
        // GET /installation/repositories
        let res = http
            .get("https://api.github.com/installation/repositories")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "poddle-compute")
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(GithubAppError::BadRequest(format!(
                "GitHub list repos failed: {}",
                res.status()
            )));
        }

        let res = res.json::<InstallationReposResponse>().await?;
        Ok((res.repositories, res.total_count))
    }
}
