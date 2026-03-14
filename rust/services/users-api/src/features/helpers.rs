use axum::Json;
use cookie::{SameSite, time::Duration};
use sqlx::PgPool;
use users_core::jwt::{TokenType, create_token};

use axum_extra::extract::{PrivateCookieJar, cookie::Cookie};

use crate::{
    config::Config,
    error::AppError,
    features::{
        models::User,
        repository::UsersRepository,
        schemas::{AuthOut, Tokens},
    },
};

pub async fn finalize_auth_session(
    user: User,
    user_agent: &str,
    ip_addr: &str,
    config: &Config,
    jar: PrivateCookieJar,
    pool: &PgPool,
) -> Result<(PrivateCookieJar, Json<AuthOut>), AppError> {
    let access_token = create_token(config, user.id, TokenType::Access)?;
    let refresh_token = create_token(config, user.id, TokenType::Refresh)?;

    let max_age_days = config.jwt.refresh_token_expire_in_days;
    let access_cookie = Cookie::build(("access_token", access_token.clone()))
        .http_only(true)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(Duration::days(max_age_days))
        .secure(config.cookie_secure);
    let refresh_cookie = Cookie::build(("refresh_token", refresh_token.clone()))
        .http_only(true)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(Duration::days(max_age_days))
        .secure(config.cookie_secure);
    let jar = jar.add(refresh_cookie).add(access_cookie);

    UsersRepository::create_session(
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

    let res = Json(AuthOut { user, tokens });
    Ok((jar, res))
}
