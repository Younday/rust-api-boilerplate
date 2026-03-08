use chrono::serde::ts_seconds_option;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, Default, FromRow)]
pub struct User {
    pub id: Uuid,
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(length(min = 1), email(message = "email is invalid"))]
    pub email: String,
    #[validate(length(min = 6))]
    pub password: String,
    #[serde(with = "ts_seconds_option")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Public user representation — never includes the password field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    #[serde(with = "ts_seconds_option")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            name: user.name,
            email: user.email,
            created_at: user.created_at,
        }
    }
}
