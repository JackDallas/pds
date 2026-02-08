use thiserror::Error;

#[derive(Debug, Error)]
pub enum PdsError {
    #[error("storage error: {0}")]
    Storage(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("auth error: {0}")]
    Auth(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("upstream error: {0}")]
    Upstream(String),

    #[error("account not found")]
    AccountNotFound,

    #[error("account takendown")]
    AccountTakendown,

    #[error("account deactivated")]
    AccountDeactivated,

    #[error("handle already taken")]
    HandleAlreadyTaken,

    #[error("invalid handle")]
    InvalidHandle,

    #[error("invalid password")]
    InvalidPassword,

    #[error("session expired")]
    SessionExpired,

    #[error("internal error: {0}")]
    InternalError(String),
}

pub type PdsResult<T> = Result<T, PdsError>;
