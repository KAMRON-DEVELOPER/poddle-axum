use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::JwtError;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Access,
    Refresh,
    EmailVerification,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Claims {
    pub sub: Uuid,
    pub typ: TokenType,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct JwtConfig {
    pub secret_key: String,
    pub access_token_expire_in_minute: i64,
    pub refresh_token_expire_in_days: i64,
    pub email_verification_token_expire_in_hours: i64,
    pub refresh_token_renewal_threshold_days: i64,
}

pub trait JwtCapability {
    fn jwt_secret(&self) -> &str;
    fn access_token_expire_in_minute(&self) -> i64;
    fn refresh_token_expire_in_days(&self) -> i64;
    fn email_verification_token_expire_in_hours(&self) -> i64;
}

#[tracing::instrument(name = "create_token", skip(cfg, user_id, typ), err)]
pub fn create_token<C: JwtCapability + ?Sized>(
    cfg: &C,
    user_id: Uuid,
    typ: TokenType,
) -> Result<String, JwtError> {
    let now = Utc::now();

    let exp = now
        + match typ {
            TokenType::Access => Duration::minutes(cfg.access_token_expire_in_minute()),
            TokenType::Refresh => Duration::days(cfg.refresh_token_expire_in_days()),
            TokenType::EmailVerification => {
                Duration::hours(cfg.email_verification_token_expire_in_hours())
            }
        };

    let claims = Claims {
        sub: user_id,
        typ,
        iat: now.timestamp(),
        exp: exp.timestamp(),
    };

    let encoding_key = EncodingKey::from_secret(cfg.jwt_secret().as_bytes());
    encode(&Header::new(Algorithm::HS256), &claims, &encoding_key).map_err(|_| JwtError::Creation)
}

#[tracing::instrument(name = "verify_token", skip(cfg, token), err)]
pub fn verify_token<C: JwtCapability + ?Sized>(cfg: &C, token: &str) -> Result<Claims, JwtError> {
    let decoding_key = DecodingKey::from_secret(cfg.jwt_secret().as_bytes());
    decode::<Claims>(token, &decoding_key, &Validation::default())
        .map(|d| d.claims)
        .map_err(|_| JwtError::Invalid)
}
