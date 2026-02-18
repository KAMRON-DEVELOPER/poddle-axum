use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use std::{
    fs::{self},
    io::Error,
    path::PathBuf,
};

use chrono::Utc;

use crate::github_app::{GithubApp, GithubAppClaims, error::GithubAppError};

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
}
