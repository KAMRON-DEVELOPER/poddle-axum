use axum::http::{Request, header};
use tower_http::trace::MakeSpan;
use tracing::Span;

use crate::trace_layer::custom_make_span::CustomMakeSpan;

impl<B> MakeSpan<B> for CustomMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        let user_agent = request
            .headers()
            .get(header::USER_AGENT)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown");

        tracing::info_span!(
            "Request",
            method = %request.method(),
            uri = %request.uri(),
            version = ?request.version(),
            user_agent = %user_agent,
        )
    }
}
