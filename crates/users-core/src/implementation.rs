use axum::{
    RequestPartsExt,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};

use crate::{
    error::JwtError,
    jwt::{Claims, JwtConfig, TokenType, verify_token},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};

// impl<S> FromRequestParts<S> for Claims
// where
//     // This allows any State that contains a type implementing JwtConfig
//     // to use this extractor automatically.
//     S: Send + Sync + JwtConfig,
// {
//     type Rejection = JwtError;
//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let TypedHeader(Authorization(bearer)) = parts
//             .extract::<TypedHeader<Authorization<Bearer>>>()
//             .await
//             .map_err(|_| JwtError::Invalid)?;

//         // Get the config from the generic state
//         // let config = Box::<dyn JwtConfig>::from_ref(state);

//         // let claims = verify_token(&*config, bearer.token())?;

//         // Since S implements JwtConfig, we pass 'state' directly to verify_token
//         let claims = verify_token(state, bearer.token())?;

//         if claims.typ != TokenType::Access {
//             return Err(JwtError::WrongType);
//         }

//         Ok(claims)
//     }
// }

impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
    Box<dyn JwtConfig>: FromRef<S>,
{
    type Rejection = JwtError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| JwtError::Invalid)?;

        let config = Box::<dyn JwtConfig>::from_ref(state);

        let claims = verify_token(&*config, bearer.token())?;

        if claims.typ != TokenType::Access {
            return Err(JwtError::WrongType);
        }

        Ok(claims)
    }
}
