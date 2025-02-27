use crate::dtos::user_dto::SignUpUserDto;
use async_trait::async_trait;
use database::user::{model::User, repository::DynUserRepository};
use std::sync::Arc;
use tracing::{error, info};
use utils::{AppError, AppResult};

#[allow(clippy::module_name_repetitions)]
pub type DynUserService = Arc<dyn UserServiceTrait + Send + Sync>;

#[async_trait]
#[allow(clippy::module_name_repetitions)]
pub trait UserServiceTrait {
    // async fn get_current_user(&self, user_id: &str) -> AppResult<Option<User>>;

    async fn get_all_users(&self) -> AppResult<Vec<User>>;

    async fn signup_user(&self, request: SignUpUserDto) -> AppResult<User>;
}

#[derive(Clone)]
pub struct UserService {
    repository: DynUserRepository,
}

impl UserService {
    pub fn new(repository: DynUserRepository) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl UserServiceTrait for UserService {
    async fn signup_user(&self, request: SignUpUserDto) -> AppResult<User> {
        let email = request.email.unwrap();
        let name = request.name.unwrap();
        let password = request.password.unwrap();

        let existing_user = self.repository.get_user_by_email(&email).await?;

        if Some(existing_user).is_some() {
            error!("User {:?} already exists", email);
            return Err(AppError::Conflict(format!("email {email} is taken")));
        }

        let new_user = self
            .repository
            .create_user(&name, &email, &password)
            .await?;

        info!("Created user {:?}", new_user);

        Ok(new_user)
    }

    async fn get_all_users(&self) -> AppResult<Vec<User>> {
        let users = self.repository.get_all_users().await?;

        Ok(users)
    }
}
