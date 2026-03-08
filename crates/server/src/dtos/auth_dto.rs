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

#[derive(Debug, Serialize, Deserialize, Validate, Default)]
pub struct RefreshDto {
    #[validate(length(min = 1))]
    pub refresh_token: String,
}

/// Discriminates access tokens from refresh tokens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    Access,
    Refresh,
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
    /// Discriminates access tokens from refresh tokens.
    pub token_type: TokenType,
    /// JWT ID — present only on refresh tokens, used for rotation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    /// Always `"Bearer"`.
    pub token_type: String,
}
