use axum::Json;
use cookie::{SameSite, time::Duration};
use sqlx::PgPool;
use tracing::instrument;
use users_core::jwt::{TokenType, create_token};

use axum_extra::extract::{PrivateCookieJar, cookie::Cookie};

use crate::{
    config::Config,
    error::AppError,
    features::{
        models::User,
        repositories::sessions::SessionsRepository,
        schemas::{AuthResponse, Tokens},
    },
};

#[instrument(name = "finalize_session", skip_all, fields(user_id = %user.id, ip_addr = %ip_addr), err)]
pub async fn finalize_session(
    user: User,
    user_agent: &str,
    ip_addr: &str,
    config: &Config,
    jar: PrivateCookieJar,
    pool: &PgPool,
) -> Result<(PrivateCookieJar, Json<AuthResponse>), AppError> {
    let access_token = create_token(config, user.id, TokenType::Access)?;
    let refresh_token = create_token(config, user.id, TokenType::Refresh)?;

    let access_cookie = Cookie::build(("access_token", access_token.clone()))
        .http_only(true)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(Duration::minutes(config.jwt.access_token_expire_in_minute))
        .secure(config.cookie_secure);
    let refresh_cookie = Cookie::build(("refresh_token", refresh_token.clone()))
        .http_only(true)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(Duration::days(config.jwt.refresh_token_expire_in_days))
        .secure(config.cookie_secure);
    let jar = jar.add(refresh_cookie).add(access_cookie);

    SessionsRepository::create(
        &user.id,
        &user_agent.to_string(),
        &ip_addr.to_string(),
        &refresh_token,
        &pool,
    )
    .await?;

    let tokens = Tokens {
        access_token,
        refresh_token: Some(refresh_token),
    };

    let res = Json(AuthResponse { user, tokens });
    Ok((jar, res))
}
