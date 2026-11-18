use thiserror::Error;

pub type Result<T, E = AtlasError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum AtlasError {
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("operation conflicts with current state: {0}")]
    Conflict(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
}
