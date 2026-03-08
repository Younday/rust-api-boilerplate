use axum::{routing::get, Extension, Json, Router};
use database::user::model::UserResponse;
use tracing::debug;
use utils::AppResult;

use crate::{extractors::auth_extractor::AuthenticatedUser, services::Services};

#[derive(Debug)]
pub struct UserController;

impl UserController {
    pub fn app() -> Router {
        Router::new().route("/", get(Self::all))
    }

    pub async fn all(
        Extension(services): Extension<Services>,
        auth: AuthenticatedUser,
    ) -> AppResult<Json<Vec<UserResponse>>> {
        debug!("listing all users, requested by {}", auth.user_id);
        let users = services.user.get_all_users().await?;
        let response = users.into_iter().map(UserResponse::from).collect();
        Ok(Json(response))
    }
}
