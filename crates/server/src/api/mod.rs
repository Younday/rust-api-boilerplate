pub mod health;
pub mod users;

use axum::routing::{get, Router};
use health::healthy;

pub fn app() -> Router {
    Router::new()
        .route("/", get(healthy))
        .nest("/users", users::UserController::app())
}
