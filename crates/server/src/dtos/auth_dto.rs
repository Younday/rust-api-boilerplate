use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct LoginDto {
    #[validate(required, email(message = "email is invalid"))]
    pub email: Option<String>,
    #[validate(required, length(min = 6))]
    pub password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshDto {
    pub refresh_token: String,
}

/// JWT payload — used for both access and refresh tokens.
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    /// Subject: the user's UUID.
    pub sub: Uuid,
    /// Expiration (Unix timestamp, seconds).
    pub exp: usize,
    /// Issued at (Unix timestamp, seconds).
    pub iat: usize,
    /// Either `"access"` or `"refresh"`.
    pub token_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    /// Always `"Bearer"`.
    pub token_type: String,
}
