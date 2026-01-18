use axum::{Json, response::IntoResponse};
use axum_extra::extract::PrivateCookieJar;
use serde_json::json;

use crate::error::AppError;

#[allow(unused)]
pub async fn github_webhook(jar: PrivateCookieJar) -> Result<impl IntoResponse, AppError> {
    return Ok(Json(json!({"ok": true})));
}
