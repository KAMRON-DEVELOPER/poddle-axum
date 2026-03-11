use aide::{
    OperationOutput,
    generate::GenContext,
    openapi::{Operation, Response, StatusCode},
};
use axum::Json;
use compute_core::services::event_emission_service::error::EventEmissionServiceError;
use http_contracts::error::schema::ErrorResponse;
use users_core::jwt::JwtCapability;

use crate::config::Config;

use users_core::error::ClaimsError;

use crate::error::AppError;

// -------------------------------------------------------------------------------
// ---------------------------- Error implementations ----------------------------
// -------------------------------------------------------------------------------

/// Teaches `aide` (the OpenAPI doc generator) how to describe `AppError` in the generated spec.
///
/// # Why this exists
/// Handlers return `Result<T, AppError>`. Without this impl, aide has no idea what error
/// responses those handlers can produce, so the generated OpenAPI spec would be missing
/// all error status codes entirely.
///
/// # How aide uses this
/// When aide processes `Result<T, AppError>`, it calls:
///   1. `T::inferred_responses`      → documents the success path (e.g. 200)
///   2. `AppError::inferred_responses` → documents all error paths (400, 401, … 500)
///
/// # The two methods
/// - `operation_response`  — returns the *base response schema* (i.e. what the JSON body looks like). Used as a building block by `inferred_responses`.
/// - `inferred_responses`  — returns every (status code, response) pair this type can produce
impl OperationOutput for AppError {
    type Inner = ErrorResponse;

    fn operation_response(ctx: &mut GenContext, operation: &mut Operation) -> Option<Response> {
        let mut res = Json::<ErrorResponse>::operation_response(ctx, operation).unwrap_or_default();
        res.description = "API Error Response".into();
        Some(res)
    }

    fn inferred_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<StatusCode>, Response)> {
        let base = Json::<ErrorResponse>::operation_response(ctx, operation).unwrap_or_default();

        let mk = |code: u16, desc: &str| {
            (
                Some(StatusCode::Code(code)),
                Response {
                    description: desc.to_string(),
                    ..base.clone()
                },
            )
        };

        vec![
            mk(400, "Bad request"),
            mk(401, "Unauthorized"),
            mk(403, "Forbidden"),
            mk(404, "Not found"),
            mk(422, "Validation error"),
            mk(500, "Internal server error"),
        ]
    }
}

impl From<EventEmissionServiceError> for AppError {
    fn from(e: EventEmissionServiceError) -> Self {
        match e {
            EventEmissionServiceError::SqlxError(error) => AppError::SqlxError(error),
            EventEmissionServiceError::RedisError(error) => AppError::RedisError(error),
        }
    }
}

impl From<ClaimsError> for AppError {
    fn from(e: ClaimsError) -> Self {
        match e {
            ClaimsError::Creation => AppError::TokenCreationError,
            ClaimsError::Expired => AppError::ExpiredTokenError,
            ClaimsError::WrongType => AppError::WrongTokenTypeError,
            ClaimsError::Invalid => AppError::InvalidTokenError,
            ClaimsError::KeyError => AppError::KeyError,
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

impl From<http::StatusCode> for AppError {
    fn from(value: http::StatusCode) -> Self {
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
