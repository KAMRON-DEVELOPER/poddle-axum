use axum::RequestPartsExt;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use users_core::jwt::{Claims, TokenType, verify_token};

use crate::config::Config;
use crate::error::AppError;

impl FromRequestParts<Config> for Claims {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, cfg: &Config) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::MissingAccessToken)?;

        // let cfg = Config::from_ref(state);

        let claims = verify_token(cfg, bearer.token())?;

        if claims.typ != TokenType::Access {
            return Err(AppError::Unauthorized("Access token required".into()));
        }

        Ok(claims)
    }
}

// impl<S> FromRequestParts<S> for Claims
// where
//     Config: FromRef<S>,
//     S: Send + Sync,
// {
//     type Rejection = AppError;
//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let TypedHeader(Authorization(bearer)) = parts
//             .extract::<TypedHeader<Authorization<Bearer>>>()
//             .await
//             .map_err(|_| AppError::MissingAccessToken)?;

//         let config = Config::from_ref(state);

//         let claims = verify_token(&config, bearer.token())?;

//         if claims.typ != TokenType::Access {
//             return Err(AppError::Unauthorized("Access token required".into()));
//         }

//         Ok(claims)
//     }
// }
