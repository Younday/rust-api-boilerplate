pub mod health;
pub mod users;

use health::healthy;
use axum::routing::{get, Router};

pub fn app() -> Router {
    Router::new()
    .route("/", get(healthy))
    .nest("/users", users::UserController::app())
}