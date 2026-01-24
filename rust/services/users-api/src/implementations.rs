use factory::factories::{mailtrap::error::MailtrapError, zepto::error::ZeptoError};
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

impl From<ZeptoError> for AppError {
    fn from(err: ZeptoError) -> Self {
        match err {
            ZeptoError::Api { error } => AppError::ExternalServiceError {
                service: "ZeptoMail".to_string(),
                code: error.code,
                message: error.message,
            },
            ZeptoError::Request(_) => AppError::ServiceUnavailable("ZeptoMail".to_string()),
            ZeptoError::Deserialize(e) => AppError::InternalServerError(e.to_string()),
        }
    }
}

impl From<MailtrapError> for AppError {
    fn from(err: MailtrapError) -> Self {
        match err {
            MailtrapError::Api { error } => AppError::ExternalServiceError {
                service: "Mailtrap".to_string(),
                code: "".to_string(),
                message: error.errors.join(","),
            },
            MailtrapError::Request(_) => AppError::ServiceUnavailable("Mailtrap".to_string()),
            MailtrapError::Deserialization(e) => AppError::InternalServerError(e.to_string()),
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
