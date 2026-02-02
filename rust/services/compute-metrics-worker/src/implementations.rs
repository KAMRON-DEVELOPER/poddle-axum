// use crate::error::AppError;

// -------------------------------------------------------------------------------
// ---------------------------- Error implementations ----------------------------
// -------------------------------------------------------------------------------

// impl From<prometheus_http_query::Error> for AppError {
//     fn from(e: prometheus_http_query::Error) -> Self {
//         match e {
//             prometheus_http_query::Error::Client(e) => AppError::InternalServerError(e.to_string()),
//             prometheus_http_query::Error::Prometheus(e) => {
//                 AppError::InternalServerError(e.to_string())
//             }
//             prometheus_http_query::Error::EmptySeriesSelector => {
//                 AppError::InternalServerError(e.to_string())
//             }
//             prometheus_http_query::Error::ParseUrl(e) => {
//                 AppError::InternalServerError(e.to_string())
//             }
//             _ => AppError::InternalServerError("Unexpected prometheus error".to_string()),
//         }
//     }
// }
