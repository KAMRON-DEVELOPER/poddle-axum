use users_core::jwt::JwtCapability;

use crate::config::Config;

use users_core::error::JwtError;

use crate::error::AppError;

// -------------------------------------------------------------------------------
// ---------------------------- Error implementations ----------------------------
// -------------------------------------------------------------------------------

impl From<JwtError> for AppError {
    fn from(e: JwtError) -> Self {
        match e {
            JwtError::Creation => AppError::TokenCreationError,
            JwtError::Invalid => AppError::InvalidTokenError,
            JwtError::Expired => AppError::ExpiredTokenError,
            JwtError::WrongType => AppError::WrongTokenTypeError,
        }
    }
}

impl From<lapin::Error> for AppError {
    fn from(value: lapin::Error) -> Self {
        match value {
            _ => AppError::InternalServerError(value.to_string()),
        }
    }
}

// -------------------------------------------------------------------------------
// --------------------------- Factory implementations ---------------------------
// -------------------------------------------------------------------------------

impl JwtCapability for Config {
    fn jwt_secret(&self) -> &str {
        &self.jwt.secret_key
    }

    fn access_token_expire_in_minute(&self) -> i64 {
        self.jwt.access_token_expire_in_minute
    }

    fn refresh_token_expire_in_days(&self) -> i64 {
        self.jwt.refresh_token_expire_in_days
    }

    fn email_verification_token_expire_in_hours(&self) -> i64 {
        self.jwt.email_verification_token_expire_in_hours
    }
}
