use crate::error::AppError;

impl From<lapin::Error> for AppError {
    fn from(value: lapin::Error) -> Self {
        match value {
            _ => AppError::InternalServerError(value.to_string()),
        }
    }
}
