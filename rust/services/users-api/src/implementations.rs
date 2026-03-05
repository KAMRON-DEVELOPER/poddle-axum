use aide::{
    OperationOutput,
    generate::GenContext,
    openapi::{Operation, ReferenceOr, Response, StatusCode},
};
use axum::Json;
use factory::factories::{mailtrap::error::MailtrapError, zepto::error::ZeptoError};
use http_contracts::error::schema::ErrorResponse;
use users_core::jwt::JwtCapability;

use crate::config::Config;

use users_core::error::JwtError;

use crate::error::AppError;

// -------------------------------------------------------------------------------
// ---------------------------- Error implementations ----------------------------
// -------------------------------------------------------------------------------

impl OperationOutput for AppError {
    type Inner = ErrorResponse;

    fn operation_response(ctx: &mut GenContext, operation: &mut Operation) -> Option<Response> {
        let base = Json::<ErrorResponse>::operation_response(ctx, operation).unwrap_or_default();

        // Add descriptions to common status codes
        let responses = operation.responses.get_or_insert_with(Default::default);

        responses.responses.insert(
            StatusCode::Code(400),
            ReferenceOr::Item(Response {
                description: "Bad request / validation error".into(),
                ..base.clone()
            }),
        );
        responses.responses.insert(
            StatusCode::Code(401),
            ReferenceOr::Item(Response {
                description: "Unauthorized — missing or invalid token".into(),
                ..base.clone()
            }),
        );
        responses.responses.insert(
            StatusCode::Code(403),
            ReferenceOr::Item(Response {
                description: "Forbidden".into(),
                ..base.clone()
            }),
        );
        responses.responses.insert(
            StatusCode::Code(404),
            ReferenceOr::Item(Response {
                description: "Not found".into(),
                ..base.clone()
            }),
        );
        responses.responses.insert(
            StatusCode::Code(500),
            ReferenceOr::Item(Response {
                description: "Internal server error".into(),
                ..base.clone()
            }),
        );

        Some(Response {
            description: "Error".into(),
            ..base
        })
    }
}

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
