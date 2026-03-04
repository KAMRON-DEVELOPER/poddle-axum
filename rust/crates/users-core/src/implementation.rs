use aide::{
    generate::GenContext,
    openapi::{Operation, SecurityRequirement},
};
use axum::{RequestPartsExt, extract::FromRequestParts, http::request::Parts};

use crate::{
    error::JwtError,
    jwt::{Claims, JwtCapability, TokenType, verify_token},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};

impl aide::OperationInput for Claims {
    fn operation_input(_ctx: &mut GenContext, operation: &mut Operation) {
        operation.security.push(SecurityRequirement::from_iter([(
            "bearerAuth".to_string(),
            Vec::new(),
        )]));
    }
}

// Option A: State itself implements JwtCapability
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync + JwtCapability,
{
    type Rejection = JwtError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| JwtError::Invalid)?;

        // Since S implements JwtCapability, we pass 'state' directly to verify_token
        let claims = verify_token(state, bearer.token())?;

        if claims.typ != TokenType::Access {
            return Err(JwtError::WrongType);
        }

        Ok(claims)
    }
}

// Option B: State can produce a JwtCapability via FromRef
// Box<dyn JwtCapability>: FromRef<S> means “For this implementation to exist, Box<dyn JwtCapability> must be constructible from &S.”
// impl<S> FromRequestParts<S> for Claims
// where
//     S: Send + Sync,
//     Box<dyn JwtCapability>: FromRef<S>,
// {
//     type Rejection = JwtError;

//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let TypedHeader(Authorization(bearer)) = parts
//             .extract::<TypedHeader<Authorization<Bearer>>>()
//             .await
//             .map_err(|_| JwtError::Invalid)?;

//         let config = Box::<dyn JwtCapability>::from_ref(state);

//         let claims = verify_token(&*config, bearer.token())?;

//         if claims.typ != TokenType::Access {
//             return Err(JwtError::WrongType);
//         }

//         Ok(claims)
//     }
// }
