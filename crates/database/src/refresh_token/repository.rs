use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tracing::error;
use utils::{AppError, AppResult};
use uuid::Uuid;

use crate::Database;

#[allow(clippy::module_name_repetitions)]
pub type DynRefreshTokenRepository = Arc<dyn RefreshTokenRepositoryTrait + Send + Sync>;

#[async_trait]
pub trait RefreshTokenRepositoryTrait {
    /// Persist a new refresh token JTI.
    async fn store(&self, jti: Uuid, user_id: Uuid, expires_at: DateTime<Utc>) -> AppResult<()>;
    /// Delete a JTI. Returns `true` if the row existed (i.e., the token was valid).
    async fn revoke(&self, jti: Uuid) -> AppResult<bool>;
}

#[async_trait]
impl RefreshTokenRepositoryTrait for Database {
    async fn store(&self, jti: Uuid, user_id: Uuid, expires_at: DateTime<Utc>) -> AppResult<()> {
        sqlx::query!(
            "INSERT INTO refresh_tokens (jti, user_id, expires_at) VALUES ($1, $2, $3)",
            jti,
            user_id,
            expires_at
        )
        .execute(&self.db)
        .await
        .map(|_| ())
        .map_err(|e| {
            error!("failed to store refresh token: {e}");
            AppError::InternalServerError
        })
    }

    async fn revoke(&self, jti: Uuid) -> AppResult<bool> {
        let result = sqlx::query!("DELETE FROM refresh_tokens WHERE jti = $1", jti)
            .execute(&self.db)
            .await
            .map_err(|e| {
                error!("failed to revoke refresh token: {e}");
                AppError::InternalServerError
            })?;
        Ok(result.rows_affected() > 0)
    }
}
