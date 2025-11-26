// use axum::{
//     extract::{FromRef, FromRequestParts},
//     http::request::Parts,
// };
// use axum_extra::extract::{PrivateCookieJar, cookie::CookieJar};
// use cookie::Key;
// use serde::{Deserialize, Serialize};
// use shared::utilities::errors::AppError;
// use uuid::Uuid;

// use crate::features::models::Provider;

// #[derive(Deserialize, Serialize, Debug)]
// pub struct OptionalGoogleOAuthUserSub(pub Option<String>);

// impl<S> FromRequestParts<S> for OptionalGoogleOAuthUserSub
// where
//     S: Send + Sync,
// {
//     type Rejection = AppError;
//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let jar = CookieJar::from_request_parts(parts, state).await?;
//         if let Some(cookie) = jar.get("google_oauth_user_sub") {
//             let google_oauth_user_sub = cookie.value();
//             return Ok(Self(Some(google_oauth_user_sub.to_owned())));
//         }

//         Ok(Self(None))
//     }
// }

// #[derive(Deserialize, Serialize, Debug)]
// pub struct OptionalGithubOAuthUserId(pub Option<i64>);

// impl<S> FromRequestParts<S> for OptionalGithubOAuthUserId
// where
//     S: Send + Sync,
// {
//     type Rejection = AppError;

//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let jar = CookieJar::from_request_parts(parts, state).await?;
//         if let Some(cookie) = jar.get("github_oauth_user_id") {
//             let github_oauth_user_id = cookie.value().parse::<i64>().map_err(|_| {
//                 AppError::ValidationError("Github oauth user id is not integer".to_string())
//             })?;
//             return Ok(Self(Some(github_oauth_user_id)));
//         }

//         Ok(Self(None))
//     }
// }

// #[derive(Deserialize, Serialize, Debug)]
// pub struct OptionalEmailOAuthUserId(pub Option<Uuid>);

// impl<S> FromRequestParts<S> for OptionalEmailOAuthUserId
// where
//     S: Send + Sync,
// {
//     type Rejection = AppError;

//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let jar = CookieJar::from_request_parts(parts, state).await?;
//         if let Some(cookie) = jar.get("email_oauth_user_id") {
//             let email_oauth_user_id =
//                 Uuid::try_parse(cookie.value()).map_err(|_| AppError::InvalidTokenError)?;

//             return Ok(Self(Some(email_oauth_user_id)));
//         }

//         Ok(Self(None))
//     }
// }

// #[derive(Debug)]
// pub struct OptionalOAuthUserIdCookie(pub Option<OAuthUserIdCookie>);

// #[derive(Debug)]
// pub struct OAuthUserIdCookie {
//     pub id: String,
//     pub provider: Provider,
// }

// impl<S> FromRequestParts<S> for OAuthUserIdCookie
// where
//     Key: FromRef<S>,
//     S: Send + Sync,
// {
//     type Rejection = AppError;

//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         let jar = PrivateCookieJar::<Key>::from_request_parts(parts, state).await?;

//         if let Some(cookie) = jar.get("google_oauth_user_sub") {
//             return Ok(Self {
//                 provider: Provider::Google,
//                 id: cookie.value().to_string(),
//             });
//         }

//         if let Some(cookie) = jar.get("github_oauth_user_id") {
//             let id = cookie.value().parse::<i64>().map_err(|_| {
//                 AppError::ValidationError("Github oauth user id is not integer".to_string())
//             })?;
//             return Ok(Self {
//                 provider: Provider::Github,
//                 id: id.to_string(),
//             });
//         }

//         if let Some(cookie) = jar.get("email_oauth_user_id") {
//             let id = Uuid::try_parse(cookie.value()).map_err(|_| AppError::InvalidTokenError)?;
//             return Ok(Self {
//                 provider: Provider::Email,
//                 id: id.to_string(),
//             });
//         }

//         Err(AppError::MissingOAuthIdError)
//     }
// }

// impl<S> FromRequestParts<S> for OptionalOAuthUserIdCookie
// where
//     Key: FromRef<S>,
//     S: Send + Sync,
// {
//     type Rejection = AppError;

//     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         // let jar = CookieJar::from_request_parts(parts, state).await?;
//         let jar = PrivateCookieJar::<Key>::from_request_parts(parts, state).await?;

//         if let Some(cookie) = jar.get("google_oauth_user_sub") {
//             return Ok(Self(Some(OAuthUserIdCookie {
//                 provider: Provider::Google,
//                 id: cookie.value().to_string(),
//             })));
//         }

//         if let Some(cookie) = jar.get("github_oauth_user_id") {
//             let id = cookie.value().parse::<i64>().map_err(|_| {
//                 AppError::ValidationError("Github oauth user id is not integer".to_string())
//             })?;
//             return Ok(Self(Some(OAuthUserIdCookie {
//                 provider: Provider::Github,
//                 id: id.to_string(),
//             })));
//         }

//         if let Some(cookie) = jar.get("email_oauth_user_id") {
//             let id = Uuid::try_parse(cookie.value()).map_err(|_| AppError::InvalidTokenError)?;
//             return Ok(Self(Some(OAuthUserIdCookie {
//                 provider: Provider::Email,
//                 id: id.to_string(),
//             })));
//         }

//         Ok(Self(None))
//     }
// }

// // #[derive(Deserialize, Serialize, Debug)]
// // pub struct GoogleOAuthUserSub(pub String);

// // impl<S> FromRequestParts<S> for GoogleOAuthUserSub
// // where
// //     S: Send + Sync,
// // {
// //     type Rejection = AppError;
// //     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
// //         let jar = CookieJar::from_request_parts(parts, state).await?;
// //         if let Some(cookie) = jar.get("google_oauth_user_sub") {
// //             let google_oauth_user_sub = cookie.value();
// //             return Ok(Self(google_oauth_user_sub.to_owned()));
// //         }

// //         Err(AppError::MissingGoogleOAuthSubError)
// //     }
// // }

// // #[derive(Deserialize, Serialize, Debug)]
// // pub struct GithubOAuthUserId(pub i64);

// // impl<S> FromRequestParts<S> for GithubOAuthUserId
// // where
// //     S: Send + Sync,
// // {
// //     type Rejection = AppError;

// //     async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
// //         let jar = CookieJar::from_request_parts(parts, state).await?;
// //         if let Some(cookie) = jar.get("github_oauth_user_id") {
// //             let github_oauth_user_id = cookie.value().parse::<i64>().map_err(|_| {
// //                 AppError::ValidationError("Github oauth user id is not integer".to_string())
// //             })?;
// //             return Ok(Self(github_oauth_user_id));
// //         }

// //         Err(AppError::MissingGithubOAuthIdError)
// //     }
// // }
