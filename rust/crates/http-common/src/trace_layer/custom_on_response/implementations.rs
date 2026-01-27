use axum::http::Response;
use std::time::Duration;
use tower_http::trace::OnResponse;
use tracing::{Level, Span, event};

use crate::trace_layer::custom_on_response::CustomOnResponse;

impl<B> OnResponse<B> for CustomOnResponse {
    fn on_response(self, response: &Response<B>, latency: Duration, _span: &Span) {
        let status = response.status();
        let code = status.as_u16();
        let latency = latency.as_millis();

        match code {
            200..=299 => {
                event!(Level::INFO, status = %status, latency = %latency, "✅ HTTP request finished")
            }
            300..=399 => {
                event!(Level::INFO, status = %status, latency = %latency, "🔄 HTTP request finished")
            }
            400..=499 => {
                event!(Level::WARN, status = %status, latency = %latency, "⚠️ HTTP request finished")
            }
            500..=599 => {
                event!(Level::ERROR, status = %status, latency = %latency, "🌋 HTTP request finished")
            }
            _ => {
                event!(Level::INFO, status = %status, latency = %latency, "❓ HTTP request finished")
            }
        }
    }
}
