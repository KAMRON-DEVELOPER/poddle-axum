use axum::{
    RequestPartsExt,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};

use crate::{
    error::JwtError,
    jwt::{Claims, JwtCapability, TokenType, verify_token},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};

// Option A: State itself implements JwtConfig
// impl<S> FromRequestParts<S> for Claims
// where
//     S: Send + Sync + JwtConfig,
// {
//     type Rejection = JwtError;
//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let TypedHeader(Authorization(bearer)) = parts
//             .extract::<TypedHeader<Authorization<Bearer>>>()
//             .await
//             .map_err(|_| JwtError::Invalid)?;

//         // Since S implements JwtConfig, we pass 'state' directly to verify_token
//         let claims = verify_token(state, bearer.token())?;

//         if claims.typ != TokenType::Access {
//             return Err(JwtError::WrongType);
//         }

//         Ok(claims)
//     }
// }

// Option B: State can produce a JwtConfig via FromRef
// Box<dyn JwtConfig>: FromRef<S> means “For this implementation to exist, Box<dyn JwtConfig> must be constructible from &S.”
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
    Box<dyn JwtCapability>: FromRef<S>,
{
    type Rejection = JwtError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| JwtError::Invalid)?;

        let config = Box::<dyn JwtCapability>::from_ref(state);

        let claims = verify_token(&*config, bearer.token())?;

        if claims.typ != TokenType::Access {
            return Err(JwtError::WrongType);
        }

        Ok(claims)
    }
}
