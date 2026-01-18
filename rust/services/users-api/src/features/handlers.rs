use crate::{
    config::Config,
    error::AppError,
    features::{
        models::{OAuthUser, Provider, User, UserRole, UserStatus},
        repository::UsersRepository,
        schemas::{
            AuthIn, AuthOut, GithubOAuthUser, GoogleOAuthUser, OAuthCallback, RedirectResponse,
            Tokens, UserIn, VerifyQuery,
        },
    },
    services::build_oauth::{GithubOAuthClient, GoogleOAuthClient},
};
use bcrypt::{DEFAULT_COST, hash, verify};
use factory::factories::{database::Database, mailtrap::Mailtrap};
use http_contracts::message::MessageResponse;
use serde_json::{Value, json};
use std::net::SocketAddr;
use users_core::jwt::{Claims, TokenType, create_token, verify_token};

use axum::{
    Json,
    extract::{ConnectInfo, Multipart, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use axum_extra::{
    TypedHeader,
    extract::{PrivateCookieJar, cookie::Cookie},
    headers::{Authorization, UserAgent, authorization::Bearer},
};
use chrono::Utc;
use cookie::{SameSite, time::Duration as CookieDuration};
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse,
};
use object_store::{ObjectStore, aws::AmazonS3, path::Path as ObjectStorePath};
use reqwest::Client;
use tracing::{debug, error, info};
use uuid::Uuid;

// -- =====================
// -- GOOGLE OAUTH
// -- =====================
pub async fn google_oauth_handler(
    jar: PrivateCookieJar,
    State(config): State<Config>,
    State(google_oauth_client): State<GoogleOAuthClient>,
) -> Result<impl IntoResponse, AppError> {
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, _csrf_token) = google_oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/user.phonenumbers.read".to_string(),
        ))
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    let pkce_verifier_cookie =
        Cookie::build(("pkce_verifier", pkce_code_verifier.secret().to_string()))
            .http_only(true)
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(CookieDuration::days(365))
            .secure(config.cookie_secure);
    let jar = jar.add(pkce_verifier_cookie);

    Ok((jar, Redirect::to(auth_url.as_ref())).into_response())
}

pub async fn google_oauth_callback_handler(
    jar: PrivateCookieJar,
    State(http_client): State<Client>,
    State(database): State<Database>,
    State(config): State<Config>,
    Query(query): Query<OAuthCallback>,
    State(google_oauth_client): State<GoogleOAuthClient>,
) -> Result<impl IntoResponse, AppError> {
    let pkce_verifier = jar
        .get("pkce_verifier")
        .map(|cookie| PkceCodeVerifier::new(cookie.value().to_string()))
        .ok_or(AppError::MissingPkceCodeVerifierError)?;

    let token_response = google_oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client)
        .await?;

    let access_token = token_response.access_token().secret();

    let get_google_oauth_user_response = http_client
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(access_token.clone())
        .send()
        .await?;
    debug!(
        "get_google_oauth_user_response: {:#?}",
        get_google_oauth_user_response
    );

    let google_oauth_user = get_google_oauth_user_response
        .json::<GoogleOAuthUser>()
        .await?;
    debug!("google_oauth_user: {:#?}", google_oauth_user);
    let oauth_user: OAuthUser = google_oauth_user.into();
    debug!("oauth_user: {:#?}", oauth_user);

    let google_oauth_user_sub = sqlx::query_scalar!(
        r#"
            INSERT INTO oauth_users (id, provider, username, email, picture)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
        "#,
        oauth_user.id,
        oauth_user.provider as Provider,
        oauth_user.username,
        oauth_user.email,
        oauth_user.picture
    )
    .fetch_one(&database.pool)
    .await?;

    let google_oauth_user_sub_cookie =
        Cookie::build(("google_oauth_user_sub", google_oauth_user_sub))
            .http_only(true)
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(CookieDuration::days(365))
            .secure(config.cookie_secure);
    let jar = jar.add(google_oauth_user_sub_cookie);

    let response = Json(RedirectResponse {
        redirect_to: "complete-profile".to_string(),
    });
    Ok((jar, response).into_response().into_response())
}

// -- =====================
// -- GITHUB OAUTH
// -- =====================
pub async fn github_oauth_handler(
    jar: PrivateCookieJar,
    State(config): State<Config>,
    State(github_oauth_client): State<GithubOAuthClient>,
) -> Result<impl IntoResponse, AppError> {
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, _csrf_token) = github_oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("user:email".to_string()))
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    let pkce_verifier_cookie =
        Cookie::build(("pkce_verifier", pkce_code_verifier.secret().to_string()))
            .http_only(true)
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(CookieDuration::days(365))
            .secure(config.cookie_secure);
    let jar = jar.add(pkce_verifier_cookie);

    Ok((jar, Redirect::to(auth_url.as_ref())).into_response())
}

pub async fn github_oauth_callback_handler(
    jar: PrivateCookieJar,
    State(http_client): State<Client>,
    State(database): State<Database>,
    State(config): State<Config>,
    Query(query): Query<OAuthCallback>,
    State(github_oauth_client): State<GithubOAuthClient>,
) -> Result<impl IntoResponse, AppError> {
    let pkce_verifier = jar
        .get("pkce_verifier")
        .map(|cookie| PkceCodeVerifier::new(cookie.value().to_string()))
        .ok_or(AppError::MissingPkceCodeVerifierError)?;

    let token_response = github_oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client)
        .await?;

    let access_token = token_response.access_token().secret();

    let get_github_oauth_user_response = http_client
        .get("https://api.github.com/user")
        .header("User-Agent", "PineSpotApp")
        .bearer_auth(access_token.clone())
        .send()
        .await?;
    debug!(
        "get_github_oauth_user_response: {:#?}",
        get_github_oauth_user_response
    );

    let github_oauth_user_text = get_github_oauth_user_response.text().await?;
    debug!("github_oauth_user_text: {:#?}", github_oauth_user_text);
    let github_oauth_user_json = serde_json::from_str::<Value>(&github_oauth_user_text)?;
    debug!("github_oauth_user_json: {:#?}", github_oauth_user_json);

    // let github_oauth_user = get_github_oauth_user_response.json::<GithubOAuthUser>().await?;
    let github_oauth_user = serde_json::from_str::<GithubOAuthUser>(&github_oauth_user_text)?;
    debug!("github_oauth_user: {:#?}", github_oauth_user);
    let oauth_user: OAuthUser = github_oauth_user.into();
    debug!("oauth_user: {:#?}", oauth_user);

    let github_oauth_user_id = sqlx::query_scalar!(
        r#"
            INSERT INTO oauth_users (id, provider, username, email, picture)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
        "#,
        oauth_user.id,
        oauth_user.provider as Provider,
        oauth_user.username,
        oauth_user.email,
        oauth_user.picture
    )
    .fetch_one(&database.pool)
    .await?;

    let github_oauth_user_sub_cookie =
        Cookie::build(("github_oauth_user_id", github_oauth_user_id))
            .http_only(true)
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(CookieDuration::days(365))
            .secure(config.cookie_secure);
    let jar = jar.add(github_oauth_user_sub_cookie);

    let response = Json(RedirectResponse {
        redirect_to: "complete-profile".to_string(),
    });
    Ok((jar, response).into_response())
}

// -- =====================
// -- CONTINUE WITH EMAIL
// -- =====================
#[tracing::instrument(
    name = "continue_with_email_handler",
    skip(jar, database, config, user_agent, addr, auth_in),
    fields(
        email = %auth_in.email,
        user_id = tracing::field::Empty // Placeholder to fill later
    ),
    err
)]
pub async fn continue_with_email_handler(
    jar: PrivateCookieJar,
    State(database): State<Database>,
    State(config): State<Config>,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(auth_in): Json<AuthIn>,
) -> Result<impl IntoResponse, AppError> {
    debug!("auth_in is {:#?}", auth_in);

    let maybe_user = UsersRepository::find_user_by_email(&auth_in.email, &database.pool).await?;

    debug!("maybe_user is {:#?}", maybe_user);

    if let Some(user) = maybe_user {
        tracing::Span::current().record("user_id", &user.id.to_string());

        let same = verify(&auth_in.password, &user.password)?;

        if !same {
            return Err(AppError::ValidationError(
                "Password is incorrect".to_string(),
            ));
        }

        let new_access = create_token(&config, user.id, TokenType::Access)?;
        let new_refresh = create_token(&config, user.id, TokenType::Refresh)?;

        let max_age_days = config.refresh_token_expire_in_days;
        let refresh_cookie = Cookie::build(("refresh_token", new_refresh.clone()))
            .http_only(true)
            .path("/")
            .same_site(SameSite::Lax)
            .max_age(CookieDuration::days(max_age_days))
            .secure(config.cookie_secure);
        let jar = jar.add(refresh_cookie);

        let tokens = Tokens {
            access_token: new_access,
            refresh_token: Some(new_refresh.clone()),
        };

        UsersRepository::create_session(
            &database.pool,
            &user.id,
            &user_agent.to_string(),
            &addr.ip().to_string(),
            &new_refresh,
        )
        .await?;

        let response = Json(AuthOut { user, tokens });
        return Ok((jar, response).into_response());
    }

    if auth_in.username.is_none() {
        info!(
            reason = "missing_username",
            "user not found, prompting registration"
        );
        return Ok(MessageResponse::new("new_user").into_response());
    }

    let mut tx = database.pool.begin().await?;

    let username = auth_in.username.clone().unwrap_or_default();
    let hash_password = hash(auth_in.password, DEFAULT_COST)?;

    let email_oauth_user_id = UsersRepository::create_oauth_user(
        &username,
        &auth_in.email,
        &hash_password,
        Provider::Email,
        &mut tx,
    )
    .await?;

    let user = UsersRepository::create_user(
        username,
        auth_in.email,
        hash_password,
        email_oauth_user_id,
        &mut tx,
    )
    .await?;

    let token = create_token(&config, user.id, TokenType::EmailVerification)?;
    let verification_link = format!("{}/auth/verify?token={}", config.frontend_endpoint, token);

    let mailtrap = Mailtrap::new();

    match mailtrap
        .send_email_verification_link(
            &user.email,
            &user.username,
            &verification_link,
            &config.email_service_api_key,
            &config.email_service_verification_template_uuid,
        )
        .await
    {
        Ok(_) => {
            tx.commit().await?;
            tracing::Span::current().record("user_id", &user.id.to_string());
            let new_access = create_token(&config, user.id, TokenType::Access)?;
            let new_refresh = create_token(&config, user.id, TokenType::Refresh)?;

            let max_age_days = config.refresh_token_expire_in_days;
            let refresh_cookie = Cookie::build(("refresh_token", new_refresh.clone()))
                .http_only(true)
                .path("/")
                .same_site(SameSite::Lax)
                .max_age(CookieDuration::days(max_age_days))
                .secure(config.cookie_secure);
            let jar = jar.add(refresh_cookie);

            let tokens = Tokens {
                access_token: new_access,
                refresh_token: Some(new_refresh.clone()),
            };

            UsersRepository::create_session(
                &database.pool,
                &user.id,
                &user_agent.to_string(),
                &addr.ip().to_string(),
                &new_refresh,
            )
            .await?;

            // 8. Return the new user and tokens
            let response = Json(AuthOut { user, tokens });
            Ok((jar, response).into_response())
        }
        Err(email_error) => {
            error!(name: "MailtrapError", "email_error: {}", email_error);
            tx.rollback().await?;
            Err(email_error.into())
        }
    }
}

// -- =====================
// -- VERIFY
// -- =====================
#[tracing::instrument(
    name = "verify_handler",
    skip(jar, config, database, verify_query),
    err
)]
pub async fn verify_handler(
    jar: PrivateCookieJar,
    State(config): State<Config>,
    State(database): State<Database>,
    Query(verify_query): Query<VerifyQuery>,
) -> Result<impl IntoResponse, AppError> {
    debug!("verify_query is '{}'", verify_query.token.clone());
    let verification_token_claims = verify_token(&config, &verify_query.token)?;

    if verification_token_claims.typ != TokenType::EmailVerification {
        return Err(AppError::InvalidTokenError);
    }

    let query_result = sqlx::query!(
        "UPDATE users SET email_verified = TRUE, status = 'active' WHERE id = $1",
        verification_token_claims.sub
    )
    .execute(&database.pool)
    .await?;

    match query_result.rows_affected() {
        0 => Err(AppError::InternalServerError(
            "User couldn't set to verified".to_string(),
        )),
        _ => {
            let redirect_to = if jar.get("refresh_token").is_none() {
                "auth".to_string()
            } else {
                "dashboard".to_string()
            };

            let response = Json(RedirectResponse { redirect_to });
            return Ok((jar, response).into_response());
        }
    }
}

// -- =====================
// -- GET USER
// -- =====================
#[tracing::instrument(name = "get_user_handler", skip(claims, database), err)]
pub async fn get_user_handler(
    claims: Claims,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
    let user = sqlx::query_as!(
        User,
        r#"
            SELECT
                id,
                username,
                email,
                password,
                picture,
                role AS "role: UserRole",
                status AS "status: UserStatus",
                email_verified,
                oauth_user_id,
                created_at,
                updated_at
            FROM users WHERE id = $1
        "#,
        claims.sub
    )
    .fetch_optional(&database.pool)
    .await?
    .ok_or_else(|| AppError::NotFoundError("User not found".to_string()))?;

    Ok(Json(user))
}

// -- =====================
// -- UPDATE USER
// -- =====================
pub async fn update_user_handler(
    State(s3): State<AmazonS3>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
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
#[tracing::instrument(name = "delete_user_handler", skip(claims, database), err)]
pub async fn delete_user_handler(
    claims: Claims,
    State(database): State<Database>,
) -> Result<impl IntoResponse, AppError> {
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
#[tracing::instrument(name = "refresh_handler", skip(config, jar, auth_header), err)]
pub async fn refresh_handler(
    State(config): State<Config>,
    jar: PrivateCookieJar,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<impl IntoResponse, AppError> {
    let (token, is_web) = if let Some(cookie) = jar.get("refresh_token") {
        (cookie.value().to_string(), true)
    } else if let Some(TypedHeader(Authorization(bearer))) = auth_header {
        (bearer.token().to_string(), false)
    } else {
        return Err(AppError::MissingRefreshToken);
    };

    let claims = verify_token(&config, &token)?;
    if claims.typ != TokenType::Refresh {
        return Err(AppError::Unauthorized("Refresh token required".into()));
    }

    let now = Utc::now().timestamp();
    let threshold_secs = config.refresh_token_renewal_threshold_days * 24 * 60 * 60;
    let new_refresh = if claims.exp.saturating_sub(now) < threshold_secs {
        Some(create_token(&config, claims.sub, TokenType::Refresh)?)
    } else {
        None
    };

    let jar = if is_web {
        if let Some(ref refresh) = new_refresh {
            let max_age_days = config.refresh_token_expire_in_days;
            let cookie = Cookie::build(("refresh_token", refresh.clone()))
                .http_only(true)
                .same_site(SameSite::Lax)
                .max_age(CookieDuration::days(max_age_days))
                .secure(config.cookie_secure);
            jar.add(cookie)
        } else {
            jar
        }
    } else {
        jar
    };

    let new_access = create_token(&config, claims.sub, TokenType::Access)?;

    let response = Json(Tokens {
        access_token: new_access,
        refresh_token: new_refresh,
    });

    Ok((jar, response))
}

// -- =====================
// -- LOGOUT
// -- =====================
#[tracing::instrument(name = "logout_handler", skip(jar))]
pub async fn logout_handler(jar: PrivateCookieJar) -> impl IntoResponse {
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
            .secure(secure)
            .max_age(CookieDuration::seconds(0));

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
