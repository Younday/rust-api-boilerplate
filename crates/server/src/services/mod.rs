pub mod auth;
pub mod user;

use std::sync::Arc;

use database::Database;
use tracing::info;
use utils::AppConfig;

use crate::services::{
    auth::{AuthService, DynAuthService},
    user::{DynUserService, UserService},
};

#[derive(Clone)]
pub struct Services {
    pub user: DynUserService,
    pub auth: DynAuthService,
}

impl Services {
    pub fn new(db: Database, config: Arc<AppConfig>) -> Self {
        info!("initializing services...");
        let repository = Arc::new(db);

        let user = Arc::new(UserService::new(repository.clone())) as DynUserService;
        let auth = Arc::new(AuthService::new(
            repository.clone(),
            config.jwt_secret.clone(),
            config.jwt_access_expiration_secs,
            config.jwt_refresh_expiration_secs,
        )) as DynAuthService;

        Self { user, auth }
    }
}
