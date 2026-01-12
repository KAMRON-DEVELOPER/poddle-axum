#[derive(thiserror::Error, Debug)]
pub enum JwtError {
    #[error("token creation failed")]
    Creation,

    #[error("invalid token")]
    Invalid,

    #[error("expired token")]
    Expired,

    #[error("wrong token type")]
    WrongType,
}
