use aide::{
    OperationOutput,
    generate::GenContext,
    openapi::{Operation, ReferenceOr, Response, StatusCode},
};
use axum::Json;
use factory::factories::zepto::error::ZeptoError;
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
