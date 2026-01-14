use axum::Json;
use axum::http::StatusCode;
use axum_core::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database query error: {0}")]
    SqlxError(#[from] sqlx::Error),
    // Error for invalid user input (400)
    #[error("Bad request: {0}")]
    BadRequest(String),
    // Error for unauthorized requests (401)
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    // Error for authorized but now allowed (403)
    #[error("Forbidden: {0}")]
    Forbidden(String),
    // Error for invalid user input (400)
    #[error("Validation error, {0}")]
    ValidationError(String),
    // Error for invalid user input (400)
    #[error("Unexpected server error")]
    Unexpected,

    #[error("Encountered an error trying to convert an infallible value: {0}")]
    FromRequestPartsError(#[from] std::convert::Infallible),

    #[error("Serde json error")]
    SerdejsonError(#[from] serde_json::Error),

    #[error("External service error")]
    ExternalServiceError {
        service: String,
        code: String,
        message: String,
    },
    #[error("Service unavailable error")]
    ServiceUnavailable(String),
    #[error("Internal server error")]
    InternalServerError(String),

    #[error("Token creation error")]
    TokenCreationError,
    #[error("Invalid token error")]
    InvalidTokenError,
    #[error("Expired token error")]
    ExpiredTokenError,
    #[error("Wrong token type error")]
    WrongTokenTypeError,
    #[error("Missing authorization token error")]
    MissingAuthorizationToken,
    #[error("Missing acces token error")]
    MissingAccessToken,
    #[error("Missing refresh token error")]
    MissingRefreshToken,
    #[error("Missing oauth id error")]
    MissingOAuthIdError,
    #[error("Missing google oauth sub error")]
    MissingGoogleOAuthSubError,
    #[error("Missing github oauth id error")]
    MissingGithubOAuthIdError,
    #[error("Invalid authorization token error")]
    InvalidAuthorizationTokenError,
    #[error("Missing session token token error")]
    MissingSessionTokenError,
    #[error("Invalid session token error")]
    InvalidSessionTokenError,
    #[error("Session not found error")]
    SessionNotFoundError,
    #[error("Expired session token error")]
    ExpiredSessionTokenError,
    #[error("OAuth user not found error")]
    OAuthUserNotFoundError,
    #[error("OAuth user id expired error")]
    OAuthUserIdExpiredError,
    #[error("Json validation error")]
    JsonValidationError,
    #[error("Invalid form data, {0}")]
    InvalidFormData(String),
    #[error("Missing pkce code verifier error")]
    MissingPkceCodeVerifierError,
    #[error("Nonce not found error")]
    NonceNotFoundError,
    #[error("Id token not found error")]
    IdTokenNotFoundError,
    #[error("Validation error, {0}")]
    ValidatorValidationError(#[from] validator::ValidationError),
    #[error("Validation errors, {0}")]
    ValidatorValidationErrors(#[from] validator::ValidationErrors),
    #[error("{0}")]
    NotFoundError(String),
    #[error("Invalid image format error")]
    InvalidImageFormatError(String),
    #[error("HTTP request error: {0}")]
    Request(#[from] reqwest::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::SqlxError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            Self::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            Self::ValidationError(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            Self::Unexpected => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error occurred".into(),
            ),

            Self::FromRequestPartsError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            Self::SerdejsonError(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
            Self::ExternalServiceError {
                service,
                code,
                message,
            } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "Service error: service-{}, code-{}, message-{}",
                    service, code, message
                ),
            ),
            Self::ServiceUnavailable(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            Self::InternalServerError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal server error: {}", msg),
            ),

            Self::InvalidTokenError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid token".to_string(),
            ),
            Self::ExpiredTokenError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Expired token".to_string(),
            ),
            Self::WrongTokenTypeError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Wrong token type error".to_string(),
            ),
            Self::MissingOAuthIdError => (
                StatusCode::UNAUTHORIZED,
                "Missing oauth id error".to_string(),
            ),
            AppError::TokenCreationError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Token creation error".to_string(),
            ),
            Self::MissingAuthorizationToken => (
                StatusCode::UNAUTHORIZED,
                "Missing authorization token".to_string(),
            ),
            Self::MissingAccessToken => {
                (StatusCode::UNAUTHORIZED, "Missing access token".to_string())
            }
            Self::MissingRefreshToken => (
                StatusCode::UNAUTHORIZED,
                "Missing refresh token".to_string(),
            ),
            Self::MissingGoogleOAuthSubError => (
                StatusCode::UNAUTHORIZED,
                "Missing google oauth sub error".to_string(),
            ),
            Self::MissingGithubOAuthIdError => (
                StatusCode::UNAUTHORIZED,
                "Missing github oauth id error".to_string(),
            ),
            Self::MissingSessionTokenError => (
                StatusCode::UNAUTHORIZED,
                "Missing session token".to_string(),
            ),
            Self::InvalidSessionTokenError => (
                StatusCode::UNAUTHORIZED,
                "Invalid session token".to_string(),
            ),
            Self::SessionNotFoundError => {
                (StatusCode::UNAUTHORIZED, "Session not found".to_string())
            }
            Self::ExpiredSessionTokenError => (
                StatusCode::UNAUTHORIZED,
                "Expired session token".to_string(),
            ),
            Self::OAuthUserNotFoundError => (
                StatusCode::UNAUTHORIZED,
                "OAuth user not found error".to_string(),
            ),
            Self::OAuthUserIdExpiredError => (
                StatusCode::UNAUTHORIZED,
                "OAuth user id expired error".to_string(),
            ),
            Self::InvalidAuthorizationTokenError => (
                StatusCode::UNAUTHORIZED,
                "Invalid authorization token".to_string(),
            ),
            Self::JsonValidationError => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Json validation error".to_string(),
            ),
            Self::InvalidFormData(e) => (StatusCode::UNPROCESSABLE_ENTITY, e),
            Self::MissingPkceCodeVerifierError => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Missing pkce code verifier error".to_string(),
            ),
            Self::NonceNotFoundError => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Nonce not found error".to_string(),
            ),
            Self::IdTokenNotFoundError => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Id token not found error".to_string(),
            ),
            Self::ValidatorValidationError(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
            Self::ValidatorValidationErrors(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
            Self::NotFoundError(e) => (StatusCode::NOT_FOUND, e),
            Self::InvalidImageFormatError(e) => (StatusCode::UNPROCESSABLE_ENTITY, e),

            Self::Request(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        let body = Json(json!({"error": msg}));

        (status, body).into_response()
    }
}
