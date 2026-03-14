use axum::Json;
use cookie::{SameSite, time::Duration};
use sqlx::{PgPool, Postgres, Transaction};
use users_core::jwt::{TokenType, create_token};

use axum_extra::extract::{PrivateCookieJar, cookie::Cookie};

use crate::{
    config::Config,
    error::AppError,
    features::{
        models::User,
        repository::UsersRepository,
        schemas::{AuthResponse, Tokens},
    },
};

pub async fn finalize_auth_session(
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
        .max_age(Duration::days(config.jwt.access_token_expire_in_minute))
        .secure(config.cookie_secure);
    let refresh_cookie = Cookie::build(("refresh_token", refresh_token.clone()))
        .http_only(true)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(Duration::days(config.jwt.refresh_token_expire_in_days))
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

    let res = Json(AuthResponse { user, tokens });
    Ok((jar, res))
}

async fn resolve_or_create_oauth_user(
    oauth_user: OAuthUser,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<User, AppError> {
    let oauth_user_id = UsersRepository::create_oauth_user(
        &oauth_user.id,
        oauth_user.username.as_deref(),
        oauth_user.email.as_deref(),
        oauth_user.picture.as_deref(),
        None,
        oauth_user.provider,
        &mut **tx,
    )
    .await?;

    if let Some(user) =
        UsersRepository::find_user_by_oauth_user_id(&oauth_user_id, &mut **tx).await?
    {
        return Ok(user);
    }

    if let Some(email) = oauth_user.email.as_deref() {
        if let Some(existing_user) = UsersRepository::find_user_by_email(email, &mut **tx).await? {
            if let Some(existing_oauth_user_id) = &existing_user.oauth_user_id {
                if existing_oauth_user_id != &oauth_user_id {
                    tracing::error!(
                        user_id = %existing_user.id,
                        existing_oauth_user_id = %existing_oauth_user_id,
                        new_oauth_user_id = %oauth_user_id,
                        "user email matched but oauth_user_id conflicts"
                    );

                    return Err(AppError::ConflictError(
                        "Account linkage conflict. Please contact support.".to_string(),
                    ));
                }

                return Ok(existing_user);
            }

            let result = UsersRepository::link_user_to_oauth_user_if_unlinked(
                &existing_user.id,
                &oauth_user_id,
                &mut **tx,
            )
            .await?;

            if result.rows_affected() == 0 {
                tracing::error!(
                    user_id = %existing_user.id,
                    oauth_user_id = %oauth_user_id,
                    "failed to link existing user to oauth user"
                );

                return Err(AppError::ConflictError(
                    "Could not link OAuth account safely.".to_string(),
                ));
            }

            let linked_user =
                UsersRepository::get_user_by_id(&existing_user.id, tx.as_mut()).await?;

            return Ok(linked_user);
        }
    }

    UsersRepository::create_user(
        oauth_user
            .username
            .clone()
            .unwrap_or_else(|| "user".to_string()),
        oauth_user.email.clone().unwrap_or_default(),
        oauth_user.picture.clone(),
        None,
        oauth_user_id,
        tx,
    )
    .await
}
