use crate::{
    config::Config,
    error::AppError,
    features::{
        helpers::finalize_session,
        models::Provider,
        repositories::{oauth_users::OAuthUsersRepository, users::UsersRepository},
        schemas::{
            GithubOAuthUser, GoogleOAuthUser, OAuthCallback, PasswordSetupRequest, TokenQuery,
            UserMutationPayload,
        },
    },
    services::{github_oauth::GithubOAuthClient, google_oauth::GoogleOAuthClient},
};
use aide::axum::IntoApiResponse;
use bcrypt::hash;
use factory::factories::database::Database;
use http_contracts::message::MessageResponse;
use std::net::SocketAddr;
use users_core::jwt::{TokenType, verify_token};

use axum::{
    Json,
    extract::{ConnectInfo, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use axum_extra::{
    TypedHeader,
    extract::{PrivateCookieJar, cookie::Cookie},
    headers::UserAgent,
};
use cookie::{SameSite, time::Duration as CookieDuration};
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse,
};
use reqwest::Client;
use tracing::{Instrument, error, info_span, instrument};

// -- =====================
// -- GOOGLE OAUTH
// -- =====================
#[instrument(name = "google_oauth_handler", skip_all, err)]
pub async fn google_oauth_handler(
    jar: PrivateCookieJar,
    State(config): State<Config>,
    State(google_oauth_client): State<GoogleOAuthClient>,
) -> Result<impl IntoApiResponse, AppError> {
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

#[instrument(name = "google_oauth_callback_handler", skip_all, fields(oauth_user_id = tracing::field::Empty, user_id = tracing::field::Empty), err)]
pub async fn google_oauth_callback_handler(
    jar: PrivateCookieJar,
    State(http_client): State<Client>,
    State(database): State<Database>,
    State(config): State<Config>,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(query): Query<OAuthCallback>,
    State(google_oauth_client): State<GoogleOAuthClient>,
) -> Result<impl IntoApiResponse, AppError> {
    let pkce_verifier = jar
        .get("pkce_verifier")
        .map(|cookie| PkceCodeVerifier::new(cookie.value().to_string()))
        .ok_or(AppError::MissingPkceCodeVerifierError)?;

    let token_response = google_oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client)
        .instrument(info_span!("exchange_code_request"))
        .await?;

    let access_token = token_response.access_token().secret();

    let get_google_oauth_user_response = http_client
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(access_token.clone())
        .send()
        .instrument(info_span!("get_google_oauth_user_request"))
        .await?;

    if !get_google_oauth_user_response.status().is_success() {
        let status = get_google_oauth_user_response.status();
        let error_body = get_google_oauth_user_response
            .text()
            .await
            .unwrap_or_default();
        tracing::error!(%status, %error_body, "Failed to fetch profile from Google");
        return Err(AppError::InternalServerError("OAuth provider error".into()));
    }

    let google_oauth_user = get_google_oauth_user_response
        .json::<GoogleOAuthUser>()
        .await?;

    let mut tx = database.pool.begin().await?;

    let oauth_user = OAuthUsersRepository::find(
        &google_oauth_user.sub.to_string(),
        &Provider::Google,
        &mut *tx,
    )
    .await?;

    let red = Redirect::to(&format!("{}/console/dashboard", config.frontend_endpoint));

    // --- oauth user found ---
    if let Some(oauth_user) = oauth_user {
        tracing::Span::current().record("oauth_user_id", &oauth_user.id);

        let user = UsersRepository::get(&oauth_user.user_id, &mut *tx)
            .await
            .map_err(|_| {
                error!(
                    provider = %oauth_user.provider,
                    oauth_user_id = %oauth_user.id,
                    user_id = %oauth_user.user_id,
                    "oauth account is linked to a missing user"
                );
                AppError::InternalServerError(
                    "This social account is linked incorrectly. Please contact support.".into(),
                )
            })?;

        tracing::Span::current().record("user_id", &user.id.to_string());

        let (jar, _) = finalize_session(
            user,
            user_agent.as_str(),
            &addr.ip().to_string(),
            &config,
            jar,
            &database.pool,
        )
        .await?;

        return Ok((jar, red).into_response());
    }

    let email = google_oauth_user.email.clone().ok_or_else(|| {
        AppError::ValidationError(
            "Google did not return an email address. Please use another sign-in method.".into(),
        )
    })?;

    // --- user found but oauth user not found ---
    if let Some(user) = UsersRepository::find_by_email(&email, &mut *tx).await? {
        tracing::Span::current().record("user_id", &user.id.to_string());

        let oauth_payload = (user.id, google_oauth_user).into();
        let oauth_user = OAuthUsersRepository::create(oauth_payload, &mut *tx)
        .await
        .map_err(|e| {
            error!(e = %e, user_id = %user.id, "failed to link google account to existing user");
            AppError::InternalServerError("Failed to link Google account.".into())
        })?;

        tracing::Span::current().record("oauth_user_id", &oauth_user.id);

        let (jar, _) = finalize_session(
            user,
            user_agent.as_str(),
            &addr.ip().to_string(),
            &config,
            jar,
            &database.pool,
        )
        .await?;

        return Ok((
            jar,
            Redirect::to(&format!("{}/console/dashboard", config.frontend_endpoint)),
        )
            .into_response());
    }

    // --- user and oauth user not found ---
    let payload: UserMutationPayload = (&google_oauth_user).into();
    let user = UsersRepository::create(payload, &mut tx).await?;
    tracing::Span::current().record("user_id", &user.id.to_string());

    let payload = (user.id, google_oauth_user).into();
    let oauth_user = OAuthUsersRepository::create(payload, &mut *tx).await?;
    tracing::Span::current().record("oauth_user_id", &oauth_user.id);

    let (jar, _) = finalize_session(
        user,
        user_agent.as_str(),
        &addr.ip().to_string(),
        &config,
        jar,
        &database.pool,
    )
    .await?;

    let redirect = Redirect::to(&format!("{}/console/dashboard", config.frontend_endpoint));
    Ok((jar, redirect).into_response())
}

// -- =====================
// -- GITHUB OAUTH
// -- =====================
#[instrument(name = "github_oauth_handler", skip_all, err)]
pub async fn github_oauth_handler(
    jar: PrivateCookieJar,
    State(config): State<Config>,
    State(github_oauth_client): State<GithubOAuthClient>,
) -> Result<impl IntoApiResponse, AppError> {
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

#[instrument(name = "github_oauth_callback_handler", skip_all, fields(oauth_user_id = tracing::field::Empty, user_id = tracing::field::Empty), err)]
pub async fn github_oauth_callback_handler(
    jar: PrivateCookieJar,
    State(http_client): State<Client>,
    State(database): State<Database>,
    State(config): State<Config>,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(query): Query<OAuthCallback>,
    State(github_oauth_client): State<GithubOAuthClient>,
) -> Result<impl IntoApiResponse, AppError> {
    let pkce_verifier = jar
        .get("pkce_verifier")
        .map(|cookie| PkceCodeVerifier::new(cookie.value().to_string()))
        .ok_or(AppError::MissingPkceCodeVerifierError)?;

    let token_response = github_oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client)
        .instrument(info_span!("exchange_code_request"))
        .await?;

    let access_token = token_response.access_token().secret();

    let get_github_oauth_user_response = http_client
        .get("https://api.github.com/user")
        .header("User-Agent", "Poddle Dev")
        .bearer_auth(access_token.clone())
        .send()
        .instrument(info_span!("get_github_oauth_user_request"))
        .await?;

    if !get_github_oauth_user_response.status().is_success() {
        let status = get_github_oauth_user_response.status();
        let error_body = get_github_oauth_user_response
            .text()
            .await
            .unwrap_or_default();
        tracing::error!(%status, %error_body, "Failed to fetch profile from Google");
        return Err(AppError::InternalServerError("OAuth provider error".into()));
    }

    let github_oauth_user = get_github_oauth_user_response
        .json::<GithubOAuthUser>()
        .await?;

    let mut tx = database.pool.begin().await?;

    let oauth_user = OAuthUsersRepository::find(
        &github_oauth_user.id.to_string(),
        &Provider::Github,
        &mut *tx,
    )
    .await?;

    let red = Redirect::to(&format!("{}/console/dashboard", config.frontend_endpoint));

    // --- oauth user found ---
    if let Some(oauth_user) = oauth_user {
        let user = UsersRepository::get(&oauth_user.user_id, &mut *tx)
            .await
            .map_err(|_| {
                error!(
                    provider = %oauth_user.provider,
                    oauth_user_id = %oauth_user.id,
                    user_id = %oauth_user.user_id,
                    "oauth account is linked to a missing user"
                );
                AppError::InternalServerError(
                    "This social account is linked incorrectly. Please contact support.".into(),
                )
            })?;

        let (jar, _) = finalize_session(
            user,
            user_agent.as_str(),
            &addr.ip().to_string(),
            &config,
            jar,
            &database.pool,
        )
        .await?;

        return Ok((jar, red).into_response());
    }

    let email = github_oauth_user.email.clone().ok_or_else(|| {
        AppError::ValidationError(
            "GitHub did not return an email address. Please use another sign-in method.".into(),
        )
    })?;

    // --- user found but oauth user not found ---
    if let Some(user) = UsersRepository::find_by_email(&email, &mut *tx).await? {
        tracing::Span::current().record("user_id", &user.id.to_string());

        let oauth_payload = (user.id, github_oauth_user).into();
        let oauth_user = OAuthUsersRepository::create(oauth_payload, &mut *tx)
        .await
        .map_err(|e| {
            error!(e = %e, user_id = %user.id, "failed to link google account to existing user");
            AppError::InternalServerError("Failed to link Google account.".into())
        })?;

        tracing::Span::current().record("oauth_user_id", &oauth_user.id);

        let (jar, _) = finalize_session(
            user,
            user_agent.as_str(),
            &addr.ip().to_string(),
            &config,
            jar,
            &database.pool,
        )
        .await?;

        return Ok((
            jar,
            Redirect::to(&format!("{}/console/dashboard", config.frontend_endpoint)),
        )
            .into_response());
    }

    // --- user and oauth user not found ---
    let payload: UserMutationPayload = (&github_oauth_user).into();
    let user = UsersRepository::create(payload, &mut tx).await?;
    tracing::Span::current().record("user_id", &user.id.to_string());

    let payload = (user.id, github_oauth_user).into();
    let oauth_user = OAuthUsersRepository::create(payload, &mut *tx).await?;
    tracing::Span::current().record("oauth_user_id", &oauth_user.id);

    tx.commit().await?;

    let (jar, _) = finalize_session(
        user,
        user_agent.as_str(),
        &addr.ip().to_string(),
        &config,
        jar,
        &database.pool,
    )
    .await?;

    Ok((jar, red).into_response())
}

// -- =====================
// -- PASSWORD SETUP
// -- =====================
#[instrument(name = "verify_handler", skip_all, err)]
pub async fn password_setup_handler(
    State(config): State<Config>,
    State(database): State<Database>,
    Query(token_query): Query<TokenQuery>,
    Json(req): Json<PasswordSetupRequest>,
) -> Result<impl IntoApiResponse, AppError> {
    let claims = verify_token(&config, &token_query.token)?;

    if claims.typ != TokenType::PasswordSetup {
        return Err(AppError::InvalidTokenError);
    }

    let hash_password = tokio::task::spawn_blocking(move || {
        let _span = info_span!("password_hashing").entered();
        hash(req.password, 10)
    })
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))??;

    let query_result =
        UsersRepository::update_password(&claims.sub, &hash_password, &database.pool).await?;

    if query_result.rows_affected() == 0 {
        return Err(AppError::NotFoundError("User not found".to_string()));
    }

    Ok((
        StatusCode::OK,
        Json(MessageResponse::new(
            "Password set successfully. You can now log in with your email.",
        )),
    )
        .into_response())
}
