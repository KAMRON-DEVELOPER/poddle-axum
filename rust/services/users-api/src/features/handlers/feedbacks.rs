use aide::axum::IntoApiResponse;
use axum::{Json, extract::State};
use axum_extra::extract::Query;
use factory::factories::{database::Database, mailtrap::Mailtrap};
use http_contracts::{
    list::schema::ListResponse, message::MessageResponse, pagination::schema::Pagination,
};
use tracing::{error, instrument};

use crate::{
    config::Config,
    error::AppError,
    features::{repositories::feedbacks::FeedbacksRepository, schemas::CreateFeedbackRequest},
};

#[instrument(name = "get_feedbacks_handler", skip_all)]
pub async fn get_feedbacks_handler(
    Query(p): Query<Pagination>,
    State(db): State<Database>,
) -> Result<impl IntoApiResponse, AppError> {
    let (data, total) = FeedbacksRepository::get_many(p.offset, p.limit, &db.pool).await?;

    Ok(Json(ListResponse { data, total }))
}

#[instrument(name = "create_feedback_handler", skip_all)]
pub async fn create_feedback_handler(
    State(cfg): State<Config>,
    State(db): State<Database>,
    Json(req): Json<CreateFeedbackRequest>,
) -> Result<impl IntoApiResponse, AppError> {
    let query_result = FeedbacksRepository::create(&req, &db.pool).await?;

    if query_result.rows_affected() == 0 {
        return Err(AppError::InternalServerError(
            "Could not save feedback".into(),
        ));
    }

    let mailtrap = Mailtrap::new();
    if let Err(err) = mailtrap
        .send_feedback_confirmation(&req.name, &req.email, &req.message, &cfg.mailtrap)
        .await
    {
        error!(name: "MailtrapError", "Email failed but DB saved: {}", err);
    }

    let message = format!("Thank you for your feedback, {}!", req.name);
    Ok(Json(MessageResponse { message }))
}
