use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
    Extension,
};
use utils::AppError;
use uuid::Uuid;

use crate::{dtos::auth_dto::TokenType, services::Services};

/// Injected by handlers that require an authenticated caller.
/// Extracts and validates the `Authorization: Bearer <token>` header.
pub struct AuthenticatedUser {
    pub user_id: Uuid,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Extension(services) = Extension::<Services>::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::InternalServerError)?;

        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized)?;

        let claims = services.auth.verify_token(token)?;

        if claims.token_type != TokenType::Access {
            return Err(AppError::InvalidToken(
                "expected an access token".to_string(),
            ));
        }

        Ok(Self {
            user_id: claims.sub,
        })
    }
}
