use crate::{user::model::User, Database};
use async_trait::async_trait;
use std::sync::Arc;
use utils::AppResult;
use uuid::Uuid;

#[allow(clippy::module_name_repetitions)]
pub type DynUserRepository = Arc<dyn UserRepositoryTrait + Send + Sync>;

#[async_trait]
pub trait UserRepositoryTrait {
    async fn create_user(&self, name: &str, email: &str, password: &str) -> AppResult<User>;

    async fn get_user_by_id(&self, id: Uuid) -> AppResult<User>;

    async fn get_user_by_email(&self, email: &str) -> AppResult<User>;

    async fn update_user(&self, id: Uuid, name: &str, email: &str) -> AppResult<User>;

    async fn delete_user(&self, id: Uuid) -> AppResult<()>;

    async fn get_all_users(&self) -> AppResult<Vec<User>>;
}

#[async_trait]
impl UserRepositoryTrait for Database {
    async fn create_user(&self, name: &str, email: &str, password: &str) -> AppResult<User> {
        let new_user = User {
            id: Uuid::new_v4(),
            name: name.to_string(),
            email: email.to_string(),
            password: password.to_string(),
            created_at: Some(chrono::Utc::now()),
        };

        let user = sqlx::query_as!(
            User,
            "INSERT INTO users (id, name, email, password) VALUES ($1, $2, $3, $4) RETURNING *",
            new_user.id,
            new_user.name,
            new_user.email,
            new_user.password,
        )
        .fetch_one(&self.db)
        .await?;
        Ok(user)
    }

    async fn get_user_by_email(&self, email: &str) -> AppResult<User> {
        let user = sqlx::query_as!(User, "SELECT * FROM users WHERE email = $1", email)
            .fetch_one(&self.db)
            .await?;
        Ok(user)
    }

    async fn get_user_by_id(&self, id: Uuid) -> AppResult<User> {
        let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
            .fetch_one(&self.db)
            .await?;
        Ok(user)
    }

    async fn update_user(&self, id: Uuid, name: &str, email: &str) -> AppResult<User> {
        let user = sqlx::query_as!(
            User,
            "UPDATE users SET name = $1, email = $2 WHERE id = $3 RETURNING *",
            name,
            email,
            id
        )
        .fetch_one(&self.db)
        .await?;
        Ok(user)
    }

    async fn delete_user(&self, id: Uuid) -> AppResult<()> {
        sqlx::query!("DELETE FROM users WHERE id = $1", id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    async fn get_all_users(&self) -> AppResult<Vec<User>> {
        let users = sqlx::query_as!(User, "SELECT * FROM users")
            .fetch_all(&self.db)
            .await?;
        Ok(users)
    }
}
