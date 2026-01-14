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

    #[error("IO error, {0}")]
    IoError(#[from] std::io::Error),

    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("VaultClientError, {0}")]
    VaultClientError(#[from] vaultrs::error::ClientError),

    #[error("VaultClientSettingsBuilderError, {0}")]
    VaultClientSettingsBuilderError(#[from] vaultrs::client::VaultClientSettingsBuilderError),

    #[error("Lapin error, {0}")]
    LapinError(#[from] lapin::Error),

    #[error("Kube error")]
    KubeError(#[from] kube::Error),

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
    #[error("{0}")]
    NotFoundError(String),
    #[error("Invalid image format error")]
    InvalidImageFormatError(String),
    #[error("HTTP request error: {0}")]
    Request(#[from] reqwest::Error),
}
