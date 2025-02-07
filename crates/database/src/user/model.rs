use serde::{Deserialize, Serialize};
use chrono::serde::ts_seconds_option;
use sqlx::FromRow;
use validator::Validate;
use uuid::Uuid;


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
    pub created_at: Option<chrono::DateTime<chrono::Utc>>
}