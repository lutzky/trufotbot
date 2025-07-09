use axum::http::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Database Error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("{0} not found")]
    NotFound(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Internal server error")]
    InternalError(#[from] anyhow::Error),
}

impl ServiceError {
    pub fn not_found(msg: &str) -> ServiceError {
        ServiceError::NotFound(msg.to_string())
    }
}

impl axum::response::IntoResponse for ServiceError {
    fn into_response(self) -> axum::response::Response {
        match self {
            ServiceError::DatabaseError(error) => {
                log::error!("Database error: {error}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database operation failed".to_string(),
                )
            }
            ServiceError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ServiceError::InternalError(error) => {
                log::error!("Internal error: {error}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            ServiceError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        }
        .into_response()
    }
}
