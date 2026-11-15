use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unsupported link scheme: {0}")]
    UnsupportedScheme(String),
    #[error("malformed link: {0}")]
    Malformed(String),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("subscription error: {0}")]
    Subscription(String),
    #[error("cannot export this server as a share link")]
    NotExportable,
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn malformed(msg: impl Into<String>) -> Error {
    Error::Malformed(msg.into())
}
