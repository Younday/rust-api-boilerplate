use axum::{http::StatusCode, routing::post, Extension, Json, Router};
use utils::AppResult;

use crate::{
    dtos::{
        auth_dto::{AuthResponse, LoginDto, RefreshDto},
        user_dto::SignUpUserDto,
    },
    extractors::validation_extractor::ValidationExtractor,
    services::Services,
};

#[derive(Debug)]
pub struct AuthController;

impl AuthController {
    pub fn app() -> Router {
        Router::new()
            .route("/signup", post(Self::signup))
            .route("/login", post(Self::login))
            .route("/refresh", post(Self::refresh))
    }

    pub async fn signup(
        Extension(services): Extension<Services>,
        ValidationExtractor(dto): ValidationExtractor<SignUpUserDto>,
    ) -> AppResult<(StatusCode, Json<AuthResponse>)> {
        let response = services.auth.register(dto).await?;
        Ok((StatusCode::CREATED, Json(response)))
    }

    pub async fn login(
        Extension(services): Extension<Services>,
        ValidationExtractor(dto): ValidationExtractor<LoginDto>,
    ) -> AppResult<Json<AuthResponse>> {
        let response = services.auth.login(dto).await?;
        Ok(Json(response))
    }

    pub async fn refresh(
        Extension(services): Extension<Services>,
        ValidationExtractor(dto): ValidationExtractor<RefreshDto>,
    ) -> AppResult<Json<AuthResponse>> {
        let response = services.auth.refresh_token(dto).await?;
        Ok(Json(response))
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use database::Database;
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use std::sync::Arc;
    use tower::ServiceExt;
    use utils::{AppConfig, CargoEnv};

    use crate::services::Services;

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn test_config() -> Arc<AppConfig> {
        Arc::new(AppConfig {
            cargo_env: CargoEnv::Development,
            app_host: "127.0.0.1".to_string(),
            app_port: 8080,
            postgres_uri: String::new(),
            jwt_secret: "integration-test-secret-key".to_string(),
            jwt_access_expiration_secs: 3600,
            jwt_refresh_expiration_secs: 604800,
        })
    }

    /// Minimal test router: routes nested under `/api/v1` + Extension(services).
    /// Skips rate limiting and buffering — those Tower layers require a Tokio time
    /// driver that #[sqlx::test] doesn't initialise.
    /// Since `PgPool` is cheaply cloneable, call `setup_router(pool.clone())` for each
    /// `oneshot` call within the same test.
    fn setup_router(pool: PgPool) -> axum::Router {
        let services = Services::new(Database { db: pool }, test_config());
        axum::Router::new()
            .nest("/api/v1", crate::api::app())
            .layer(axum::Extension(services))
    }

    async fn post_json(router: axum::Router, uri: &str, body: Value) -> axum::response::Response {
        let req = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        router.oneshot(req).await.unwrap()
    }

    async fn get_authed(router: axum::Router, uri: &str, token: &str) -> axum::response::Response {
        let req = Request::builder()
            .method("GET")
            .uri(uri)
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        router.oneshot(req).await.unwrap()
    }

    async fn json_body(response: axum::response::Response) -> Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        }
    }

    // ---------------------------------------------------------------------------
    // POST /api/v1/auth/signup
    // ---------------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn signup_returns_201_with_tokens(pool: PgPool) {
        let resp = post_json(
            setup_router(pool),
            "/api/v1/auth/signup",
            json!({"name": "Alice", "email": "alice@example.com", "password": "password123"}),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = json_body(resp).await;
        assert!(body["access_token"].is_string());
        assert!(body["refresh_token"].is_string());
        assert_eq!(body["token_type"], "Bearer");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn signup_response_never_includes_password(pool: PgPool) {
        let resp = post_json(
            setup_router(pool),
            "/api/v1/auth/signup",
            json!({"name": "Alice", "email": "alice@example.com", "password": "supersecret"}),
        )
        .await;

        let body = json_body(resp).await;
        assert!(
            body.get("password").is_none(),
            "password must not appear in response"
        );
        assert!(
            !body.to_string().contains("supersecret"),
            "plaintext password must not appear"
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn signup_duplicate_email_returns_409(pool: PgPool) {
        let payload =
            json!({"name": "Alice", "email": "alice@example.com", "password": "password123"});
        // First signup
        post_json(
            setup_router(pool.clone()),
            "/api/v1/auth/signup",
            payload.clone(),
        )
        .await;
        // Second signup — same email
        let resp = post_json(setup_router(pool), "/api/v1/auth/signup", payload).await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn signup_missing_required_fields_returns_400(pool: PgPool) {
        let resp = post_json(
            setup_router(pool),
            "/api/v1/auth/signup",
            json!({"email": "alice@example.com"}), // missing name and password
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn signup_invalid_email_returns_400(pool: PgPool) {
        let resp = post_json(
            setup_router(pool),
            "/api/v1/auth/signup",
            json!({"name": "Alice", "email": "not-an-email", "password": "password123"}),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // ---------------------------------------------------------------------------
    // POST /api/v1/auth/login
    // ---------------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn login_correct_credentials_returns_200_with_tokens(pool: PgPool) {
        // Register first
        post_json(
            setup_router(pool.clone()),
            "/api/v1/auth/signup",
            json!({"name": "Bob", "email": "bob@example.com", "password": "password123"}),
        )
        .await;

        let resp = post_json(
            setup_router(pool),
            "/api/v1/auth/login",
            json!({"email": "bob@example.com", "password": "password123"}),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert!(body["access_token"].is_string());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn login_wrong_password_returns_401(pool: PgPool) {
        post_json(
            setup_router(pool.clone()),
            "/api/v1/auth/signup",
            json!({"name": "Bob", "email": "bob@example.com", "password": "password123"}),
        )
        .await;

        let resp = post_json(
            setup_router(pool),
            "/api/v1/auth/login",
            json!({"email": "bob@example.com", "password": "wrongpassword"}),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn login_unknown_email_returns_401(pool: PgPool) {
        let resp = post_json(
            setup_router(pool),
            "/api/v1/auth/login",
            json!({"email": "nobody@example.com", "password": "password123"}),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ---------------------------------------------------------------------------
    // POST /api/v1/auth/refresh
    // ---------------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn refresh_with_valid_refresh_token_returns_200(pool: PgPool) {
        let signup_resp = post_json(
            setup_router(pool.clone()),
            "/api/v1/auth/signup",
            json!({"name": "Carol", "email": "carol@example.com", "password": "password123"}),
        )
        .await;
        let body = json_body(signup_resp).await;
        let refresh_token = body["refresh_token"].as_str().unwrap().to_string();

        let resp = post_json(
            setup_router(pool),
            "/api/v1/auth/refresh",
            json!({"refresh_token": refresh_token}),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert!(body["access_token"].is_string());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn refresh_with_access_token_returns_401(pool: PgPool) {
        let signup_resp = post_json(
            setup_router(pool.clone()),
            "/api/v1/auth/signup",
            json!({"name": "Dave", "email": "dave@example.com", "password": "password123"}),
        )
        .await;
        let body = json_body(signup_resp).await;
        // Intentionally pass the access token where a refresh token is expected.
        let access_token = body["access_token"].as_str().unwrap().to_string();

        let resp = post_json(
            setup_router(pool),
            "/api/v1/auth/refresh",
            json!({"refresh_token": access_token}),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ---------------------------------------------------------------------------
    // Protected routes — GET /api/v1/users
    // ---------------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_users_without_token_returns_401(pool: PgPool) {
        let req = Request::builder()
            .method("GET")
            .uri("/api/v1/users")
            .body(Body::empty())
            .unwrap();
        let resp = setup_router(pool).oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_users_with_valid_token_returns_200(pool: PgPool) {
        let signup_resp = post_json(
            setup_router(pool.clone()),
            "/api/v1/auth/signup",
            json!({"name": "Eve", "email": "eve@example.com", "password": "password123"}),
        )
        .await;
        let body = json_body(signup_resp).await;
        let token = body["access_token"].as_str().unwrap().to_string();

        let resp = get_authed(setup_router(pool), "/api/v1/users", &token).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_users_response_never_includes_passwords(pool: PgPool) {
        let signup_resp = post_json(
            setup_router(pool.clone()),
            "/api/v1/auth/signup",
            json!({"name": "Frank", "email": "frank@example.com", "password": "topsecret"}),
        )
        .await;
        let body = json_body(signup_resp).await;
        let token = body["access_token"].as_str().unwrap().to_string();

        let resp = get_authed(setup_router(pool), "/api/v1/users", &token).await;
        let users = json_body(resp).await;
        let users_str = users.to_string();
        assert!(
            !users_str.contains("topsecret"),
            "plaintext must not appear"
        );
        assert!(
            !users_str.contains("password"),
            "password field must not appear"
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_users_with_refresh_token_returns_401(pool: PgPool) {
        let signup_resp = post_json(
            setup_router(pool.clone()),
            "/api/v1/auth/signup",
            json!({"name": "Grace", "email": "grace@example.com", "password": "password123"}),
        )
        .await;
        let body = json_body(signup_resp).await;
        // Refresh token must be rejected — only access tokens are valid here.
        let refresh_token = body["refresh_token"].as_str().unwrap().to_string();

        let resp = get_authed(setup_router(pool), "/api/v1/users", &refresh_token).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
