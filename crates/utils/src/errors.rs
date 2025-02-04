#![allow(dead_code)]
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;

use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tracing::debug;
use tracing::log::error;
use validator::{ValidationErrors, ValidationErrorsKind};

pub type AppResult<T> = Result<T, AppError>;

pub type ErrorMap = HashMap<Cow<'static, str>, Vec<Cow<'static, str>>>;

#[derive(Debug, Deserialize, Serialize)]
pub struct HttpError {
    pub error: String,
}

impl HttpError {
    #[must_use]
    pub fn new(error: String) -> Self {
        Self { error }
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication is required to access this resource")]
    Unauthorized,
    #[error("user does not have privilege to access this resource")]
    Forbidden,
    #[error("unexpected error has occurred")]
    InternalServerError,
    #[error("{0}")]
    InternalServerErrorWithContext(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    PreconditionFailed(String),
    #[error(transparent)]
    AxumJsonRejection(#[from] JsonRejection),
    #[error(transparent)]
    ValidationError(#[from] ValidationErrors),
    #[error("unprocessable request has occurred")]
    UnprocessableEntity { errors: ErrorMap },
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    AnyhowError(#[from] anyhow::Error),
    #[error(transparent)]
    PoolTimedOut(#[from] sqlx::Error),
}

impl AppError {
    #[must_use]
    /// Maps `validator`'s `ValidationrErrors` to a simple map of property name/error messages structure.
    pub fn unprocessable_entity(errors: ValidationErrors) -> Response {
        let mut validation_errors = ErrorMap::new();

        for (field_property, error_kind) in errors.into_errors() {
            if let ValidationErrorsKind::Field(field_meta) = error_kind.clone() {
                for error in field_meta {
                    validation_errors
                        .entry(Cow::from(field_property.clone()))
                        .or_default()
                        .push(error.message.unwrap_or_else(|| {
                            let params: Vec<Cow<'static, str>> = error
                                .params
                                .iter()
                                .filter(|(key, _value)| *key != "value")
                                .map(|(key, value)| Cow::from(format!("{key} value is {value}")))
                                .collect();

                            if params.is_empty() {
                                Cow::from(format!("{field_property} is required"))
                            } else {
                                Cow::from(params.join(", "))
                            }
                        }));
                }
            }
        }

        let body = Json(json!({
            "errors": validation_errors,
        }));

        (StatusCode::BAD_REQUEST, body).into_response()
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        debug!("{:#?}", self);
        if let Self::ValidationError(e) = self {
            return Self::unprocessable_entity(e);
        }

        let (status, error_message) = match self {
            Self::InternalServerErrorWithContext(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
            Self::NotFound(err) => (StatusCode::NOT_FOUND, err),
            Self::Conflict(err) => (StatusCode::CONFLICT, err),
            Self::PreconditionFailed(err) => (StatusCode::PRECONDITION_FAILED, err),
            Self::BadRequest(err) => (StatusCode::BAD_REQUEST, err),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, Self::Unauthorized.to_string()),
            Self::Forbidden => (StatusCode::FORBIDDEN, Self::Forbidden.to_string()),
            Self::AxumJsonRejection(err) => (StatusCode::BAD_REQUEST, err.body_text()),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Self::InternalServerError.to_string(),
            ),
        };

        let body = Json(HttpError::new(error_message));

        (status, body).into_response()
    }
}
