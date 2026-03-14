use crate::{
    config::Config,
    error::AppError,
    features::{
        helpers::finalize_session,
        repositories::{oauth_users::OAuthUsersRepository, users::UsersRepository},
        schemas::{EmailAuthRequest, TokenQuery, Tokens, UserIn, UserMutationPayload},
    },
};
use aide::axum::IntoApiResponse;
use bcrypt::{hash, verify};
use factory::factories::{database::Database, mailtrap::Mailtrap};
use http_contracts::message::MessageResponse;
use serde_json::json;
use std::net::SocketAddr;
use users_core::jwt::{Claims, TokenType, create_token, verify_token};

use axum::{
    Json,
    extract::{ConnectInfo, Multipart, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::{
    TypedHeader,
    extract::{PrivateCookieJar, cookie::Cookie},
    headers::{Authorization, UserAgent, authorization::Bearer},
};
use chrono::Utc;
use cookie::{SameSite, time::Duration as CookieDuration};
use object_store::{ObjectStore, aws::AmazonS3, path::Path as ObjectStorePath};
use tracing::{debug, error, info_span, instrument, warn};
use uuid::Uuid;

// -- =====================
// -- EMAILT AUTH
// -- =====================
#[instrument(name = "email_auth_handler", skip_all, fields(email = %req.email, user_id = tracing::field::Empty), err)]
pub async fn email_auth_handler(
    jar: PrivateCookieJar,
    State(database): State<Database>,
    State(config): State<Config>,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<EmailAuthRequest>,
) -> Result<impl IntoApiResponse, AppError> {
    let user = UsersRepository::find_by_email(&req.email, &database.pool).await?;

    if let Some(user) = user {
        tracing::Span::current().record("user_id", &user.id.to_string());

        if let Some(password_hash) = &user.password {
            let password_input = req.password.clone();
            let hash_clone = password_hash.clone();

            let same = tokio::task::spawn_blocking(move || {
                let _span = info_span!("password_verifying").entered();
                verify(&password_input, &hash_clone)
            })
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

            if !same {
                return Err(AppError::ValidationError("Incorrect password".to_string()));
            }

            let res = finalize_session(
                user,
                user_agent.as_str(),
                &addr.ip().to_string(),
                &config,
                jar,
                &database.pool,
            )
            .await?;

            return Ok(res.into_response());
        }

        let providers =
            OAuthUsersRepository::find_providers_by_user_id(&user.id, &database.pool).await?;

        if providers.len() == 0 {
            error!(user_id = %user.id, "user has no password and no linked oauth provider");
            return Err(AppError::InternalServerError(
                "This account is missing a login method. Please contact support.".into(),
            ));
        }

        let token = create_token(&config, user.id, TokenType::PasswordSetup)?;
        let setup_link = format!(
            "{}/auth/set-password?token={}",
            config.frontend_endpoint, token
        );

        let mailtrap = Mailtrap::new();
        if let Err(e) = mailtrap
            .send_password_setup_link(&user.username, &user.email, &setup_link, &config.mailtrap)
            .await
        {
            error!("Failed to send password setup email: {}", e);
            return Err(AppError::InternalServerError(
                "Failed to send setup email".into(),
            ));
        }

        let providers_text = providers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Ok((
                StatusCode::ACCEPTED,
                Json(MessageResponse::new(&format!(
                    "This account was created with {}. Use that provider to sign in, or use the link we sent to set a password.", 
                    providers_text
                ))),
            ).into_response());
    }

    if req.username.is_none() {
        return Ok(MessageResponse::new("new_user").into_response());
    }

    let mut tx = database.pool.begin().await?;

    let password = req.password.clone();
    let hash_password = tokio::task::spawn_blocking(move || {
        let _span = info_span!("password_hashing").entered();
        hash(password, 10)
    })
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))??;

    let mut payload: UserMutationPayload = req.into();
    payload.hash_password = Some(hash_password);
    let user = UsersRepository::create(payload, &mut tx).await?;

    let token = create_token(&config, user.id, TokenType::EmailVerification)?;
    let verification_link = format!("{}/auth/verify?token={}", config.frontend_endpoint, token);

    let mailtrap = Mailtrap::new();

    match mailtrap
        .send_email_verification_link(
            &user.username,
            &user.email,
            &verification_link,
            &config.mailtrap,
        )
        .await
    {
        Ok(_) => {
            tx.commit().await?;
            tracing::Span::current().record("user_id", &user.id.to_string());

            let res = finalize_session(
                user,
                user_agent.as_str(),
                &addr.ip().to_string(),
                &config,
                jar,
                &database.pool,
            )
            .await?;

            return Ok(res.into_response());
        }
        Err(e) => {
            tx.rollback().await?;
            Err(e.into())
        }
    }
}

// -- =====================
// -- VERIFY
// -- =====================
#[instrument(name = "verify_handler", skip_all, err)]
pub async fn verify_handler(
    jar: PrivateCookieJar,
    State(config): State<Config>,
    State(database): State<Database>,
    Query(token_query): Query<TokenQuery>,
) -> Result<impl IntoApiResponse, AppError> {
    let claims = verify_token(&config, &token_query.token)?;

    if claims.typ != TokenType::EmailVerification {
        return Err(AppError::InvalidTokenError);
    }

    let query_result = UsersRepository::set_email_verified(&claims.sub, &database.pool).await?;

    if query_result.rows_affected() == 0 {
        warn!(user_id = %claims.sub, "email verification update affected zero rows");
        return Err(AppError::InternalServerError(
            "User not found or already verified".to_string(),
        ));
    }

    let message = if jar.get("refresh_token").is_none() {
        "auth".to_string()
    } else {
        "console/dashboard".to_string()
    };

    let res = Json(MessageResponse { message });
    return Ok((jar, res).into_response());
}

// -- =====================
// -- GET USER
// -- =====================
#[instrument(name = "get_user_handler", skip(claims, database), err)]
pub async fn get_user_handler(
    claims: Claims,
    State(database): State<Database>,
) -> Result<impl IntoApiResponse, AppError> {
    let user = UsersRepository::get(&claims.sub, &database.pool).await?;
    Ok(Json(user))
}

// -- =====================
// -- UPDATE USER
// -- =====================
#[instrument(name = "update_user_handler", skip_all, err)]
pub async fn update_user_handler(
    _claims: Claims,
    State(s3): State<AmazonS3>,
    State(_database): State<Database>,
    mut multipart: Multipart,
) -> Result<impl IntoApiResponse, AppError> {
    let mut oauth_user_schema = UserIn {
        username: None,
        email: None,
        password: None,
        picture: None,
    };

    let new_user_id = Uuid::new_v4();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();

        match name.as_str() {
            "username" => {
                oauth_user_schema.username = Some(field.text().await.unwrap());
            }
            "email" => {
                oauth_user_schema.email = Some(field.text().await.unwrap());
            }
            "password" => {
                oauth_user_schema.password = Some(field.text().await.unwrap());
            }
            "picture" => {
                let data = field.bytes().await.unwrap();
                let pic_id = Uuid::new_v4();
                let ext = infer::get(&data)
                    .ok_or_else(|| {
                        AppError::InvalidImageFormatError("Invalid image format".to_string())
                    })?
                    .extension();
                let location = ObjectStorePath::from(format!("{}/{}.{}", new_user_id, pic_id, ext));
                s3.put(&location, data.into()).await?;
                oauth_user_schema.picture = Some(location.to_string());
            }
            _ => {}
        }
    }

    debug!("oauth_user_schema: {:#?}", oauth_user_schema);
    Ok(())
}

// -- =====================
// -- DELETE USER
// -- =====================
#[instrument(name = "delete_user_handler", skip(claims, database), err)]
pub async fn delete_user_handler(
    claims: Claims,
    State(database): State<Database>,
) -> Result<impl IntoApiResponse, AppError> {
    debug!("claims: {:#?}", claims);

    let query_result = sqlx::query!("DELETE FROM users WHERE id = $1", claims.sub)
        .execute(&database.pool)
        .await?;

    match query_result.rows_affected() {
        0 => Err(AppError::NotFoundError("User not found".to_string())),
        _ => Ok(StatusCode::NO_CONTENT),
    }
}

// -- =====================
// -- REFRESH TOKEN
// -- =====================
#[instrument(name = "refresh_handler", skip(config, jar, auth_header), err)]
pub async fn refresh_handler(
    State(config): State<Config>,
    jar: PrivateCookieJar,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoApiResponse, AppError> {
    // TODO we need to refine the sesion logic
    // Currently we are not effectively using sessions. access token is stateless and refresh is statefull
    // - Look up the refresh token in the sessions table
    // - Check if `is_active == false` -> If false, someone might be tampering with tokens. You should revoke all sessions for that user to be safe.
    // - If valid, generate a new Access Token and a new Refresh Token. Update the DB row with the new refresh token, and send the new cookies back.
    // - When a user logs out `UPDATE sessions SET is_active = FALSE WHERE refresh_token = $1`

    let token = if let Some(cookie) = jar.get("refresh_token") {
        cookie.value().to_string()
    } else if let Some(TypedHeader(Authorization(bearer))) = auth_header {
        bearer.token().to_string()
    } else {
        return Err(AppError::MissingRefreshToken);
    };

    let claims = verify_token(&config, &token)?;
    if claims.typ != TokenType::Refresh {
        return Err(AppError::Unauthorized("Refresh token required".into()));
    }

    let now = Utc::now().timestamp();
    let threshold_secs = config.jwt.refresh_token_renewal_threshold_days * 24 * 60 * 60;
    let refresh_token = if claims.exp.saturating_sub(now) < threshold_secs {
        Some(create_token(&config, claims.sub, TokenType::Refresh)?)
    } else {
        None
    };

    let jar = if let Some(ref refresh) = refresh_token {
        let refresh_cookie = Cookie::build(("refresh_token", refresh.clone()))
            .http_only(true)
            .same_site(SameSite::Lax)
            .max_age(CookieDuration::days(
                config.jwt.refresh_token_expire_in_days,
            ))
            .secure(config.cookie_secure);
        jar.add(refresh_cookie)
    } else {
        jar
    };

    let access_token = create_token(&config, claims.sub, TokenType::Access)?;
    let access_cookie = Cookie::build(("access_token", access_token.clone()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::minutes(
            config.jwt.access_token_expire_in_minute,
        ))
        .secure(config.cookie_secure);
    let jar = jar.add(access_cookie);

    let response = Json(Tokens {
        access_token,
        refresh_token,
    });

    Ok((jar, response))
}

// -- =====================
// -- LOGOUT
// -- =====================
#[instrument(name = "logout_handler", skip(jar))]
pub async fn logout_handler(jar: PrivateCookieJar) -> impl IntoApiResponse {
    let mut jar = jar;

    // collect cookies into owned values first
    let cookies: Vec<(String, String, bool, Option<String>, Option<SameSite>, bool)> = jar
        .iter()
        .map(|c| {
            (
                c.name().to_string(),
                c.value().to_string(),
                c.http_only().unwrap_or(true),
                c.path().map(|p| p.to_string()),
                c.same_site(),
                c.secure().unwrap_or(false),
            )
        })
        .collect();

    for (name, value, http_only, path, same_site, secure) in cookies {
        debug!(
            "Removing cookie: {}, value: {}  http_only: {}  path: {:?}  same_site: {:?}  secure: {}",
            name, value, http_only, path, same_site, secure
        );

        let mut removal = Cookie::build((name, value))
            .http_only(http_only)
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(CookieDuration::seconds(0))
            .secure(secure);

        if let Some(path) = path {
            removal = removal.path(path);
        }

        if let Some(same_site) = same_site {
            removal = removal.same_site(same_site);
        }

        jar = jar.remove(removal);
    }

    (
        jar,
        Json(json!({
            "message": "all cookies cleared"
        })),
    )
}
