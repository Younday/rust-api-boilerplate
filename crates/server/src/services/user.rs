use std::sync::Arc;

use async_trait::async_trait;
use database::user::{model::User, repository::DynUserRepository};
use utils::AppResult;

#[allow(clippy::module_name_repetitions)]
pub type DynUserService = Arc<dyn UserServiceTrait + Send + Sync>;

#[async_trait]
#[allow(clippy::module_name_repetitions)]
pub trait UserServiceTrait {
    async fn get_all_users(&self) -> AppResult<Vec<User>>;
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
    async fn get_all_users(&self) -> AppResult<Vec<User>> {
        self.repository.get_all_users().await
    }
}
