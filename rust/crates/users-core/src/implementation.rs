use aide::{
    generate::GenContext,
    openapi::{Operation, SecurityRequirement},
};
use axum::{
    RequestPartsExt,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};

use crate::{
    error::ClaimsError,
    jwt::{Claims, JwtCapability, TokenType, verify_token},
};
use axum_extra::{
    TypedHeader,
    extract::cookie::{Key, PrivateCookieJar},
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

// Option A: State itself implements JwtCapability and can provide a Key
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync + JwtCapability,
    Key: FromRef<S>,
{
    type Rejection = ClaimsError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Attempt to get the access token from the secure HttpOnly cookie (TanStack Start SSR)
        // We use `from_request_parts` so Axum handles the Key extraction automatically.
        let jar = PrivateCookieJar::<Key>::from_request_parts(parts, state)
            .await
            .map_err(|_| ClaimsError::KeyError)?;

        let mut token_str = jar
            .get("access_token")
            .map(|cookie| cookie.value().to_string());

        // 2. Fallback: If no cookie exists, check the Authorization header (SPA / External API)
        if token_str.is_none() {
            if let Ok(TypedHeader(Authorization(bearer))) =
                parts.extract::<TypedHeader<Authorization<Bearer>>>().await
            {
                token_str = Some(bearer.token().to_string());
            }
        }

        // Ensure we actually found a token
        let token = token_str.ok_or(ClaimsError::Invalid)?;

        // Verify the token using the state's JwtCapability
        let claims = verify_token(state, &token)?;

        // Ensure it is specifically an Access Token
        if claims.typ != TokenType::Access {
            return Err(ClaimsError::WrongType);
        }

        Ok(claims)

        /*
        let jar = PrivateCookieJar::from_headers(&parts.headers, state);

        let token = if let Some(cookie) = jar.get("access_token") {
            cookie.value().to_string()
        };

        let (token, is_web) = if let Some(cookie) = jar.get("access_token") {
            (cookie.value().to_string(), true)
        } else if let Some(TypedHeader(Authorization(bearer))) = auth_header {
            (bearer.token().to_string(), false)
        } else {
            return Err(AppError::MissingRefreshToken);
        };

        let claims = verify_token(&config, &token)?;
        if claims.typ != TokenType::Refresh {
            return Err(AppError::Unauthorized("Refresh token required".into()));
        }

        return Ok(claims);

        // Fallback
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| ClaimsError::Invalid)?;

        // Since S implements JwtCapability, we pass 'state' directly to verify_token
        let claims = verify_token(state, bearer.token())?;

        if claims.typ != TokenType::Access {
            return Err(ClaimsError::WrongType);
        }

        Ok(claims)

        */
    }
}

// Option B: State can produce a JwtCapability via FromRef
// Box<dyn JwtCapability>: FromRef<S> means “For this implementation to exist, Box<dyn JwtCapability> must be constructible from &S.”
// impl<S> FromRequestParts<S> for Claims
// where
//     S: Send + Sync,
//     Box<dyn JwtCapability>: FromRef<S>,
// {
//     type Rejection = ClaimsError;

//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let TypedHeader(Authorization(bearer)) = parts
//             .extract::<TypedHeader<Authorization<Bearer>>>()
//             .await
//             .map_err(|_| ClaimsError::Invalid)?;

//         let config = Box::<dyn JwtCapability>::from_ref(state);

//         let claims = verify_token(&*config, bearer.token())?;

//         if claims.typ != TokenType::Access {
//             return Err(ClaimsError::WrongType);
//         }

//         Ok(claims)
//     }
// }
