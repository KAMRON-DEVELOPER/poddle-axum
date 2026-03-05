use aide::axum::IntoApiResponse;
use axum::Json;
use axum_extra::extract::PrivateCookieJar;
use serde_json::json;

use crate::error::AppError;

#[allow(unused)]
pub async fn github_webhook(jar: PrivateCookieJar) -> Result<impl IntoApiResponse, AppError> {
    return Ok(Json(json!({"ok": true})));
}
