// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use axum::http::StatusCode;
use color_eyre::eyre;
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
    InternalError(#[from] eyre::Error),
}

impl ServiceError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
}

impl axum::response::IntoResponse for ServiceError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::DatabaseError(error) => {
                log::error!("Database error: {error:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database operation failed".to_string(),
                )
            }
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::InternalError(error) => {
                log::error!("Internal error: {error:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        }
        .into_response()
    }
}
