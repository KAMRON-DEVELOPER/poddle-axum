use thiserror::Error;

#[derive(Error, Debug)]
pub enum GithubAppError {
    #[error("IOError")]
    IOError(#[from] std::io::Error),

    #[error("LsonWebTokenError")]
    LsonWebTokenError(#[from] jsonwebtoken::errors::Error),
}
