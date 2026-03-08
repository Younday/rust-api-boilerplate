use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use async_trait::async_trait;
use chrono::Utc;
use database::user::repository::DynUserRepository;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use tracing::{error, info};
use utils::{AppError, AppResult};
use uuid::Uuid;

use crate::dtos::{
    auth_dto::{AuthResponse, LoginDto, RefreshDto, TokenClaims},
    user_dto::SignUpUserDto,
};

#[allow(clippy::module_name_repetitions)]
pub type DynAuthService = Arc<dyn AuthServiceTrait + Send + Sync>;

#[async_trait]
#[allow(clippy::module_name_repetitions)]
pub trait AuthServiceTrait {
    async fn register(&self, dto: SignUpUserDto) -> AppResult<AuthResponse>;
    async fn login(&self, dto: LoginDto) -> AppResult<AuthResponse>;
    async fn refresh_token(&self, dto: RefreshDto) -> AppResult<AuthResponse>;
    fn verify_token(&self, token: &str) -> AppResult<TokenClaims>;
}

#[derive(Clone)]
pub struct AuthService {
    repository: DynUserRepository,
    jwt_secret: String,
    access_exp_secs: i64,
    refresh_exp_secs: i64,
}

impl AuthService {
    pub fn new(
        repository: DynUserRepository,
        jwt_secret: String,
        access_exp_secs: i64,
        refresh_exp_secs: i64,
    ) -> Self {
        Self {
            repository,
            jwt_secret,
            access_exp_secs,
            refresh_exp_secs,
        }
    }

    fn hash_password(password: &str) -> AppResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| {
                error!("failed to hash password: {e}");
                AppError::InternalServerError
            })
    }

    fn verify_password(hash: &str, password: &str) -> AppResult<()> {
        let parsed_hash = PasswordHash::new(hash).map_err(|e| {
            error!("invalid password hash stored: {e}");
            AppError::InternalServerError
        })?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| AppError::Unauthorized)
    }

    fn generate_tokens(&self, user_id: Uuid) -> AppResult<AuthResponse> {
        let now = Utc::now().timestamp();
        let encoding_key = EncodingKey::from_secret(self.jwt_secret.as_bytes());

        let access_claims = TokenClaims {
            sub: user_id,
            exp: (now + self.access_exp_secs) as usize,
            iat: now as usize,
            token_type: "access".to_string(),
        };
        let refresh_claims = TokenClaims {
            sub: user_id,
            exp: (now + self.refresh_exp_secs) as usize,
            iat: now as usize,
            token_type: "refresh".to_string(),
        };

        let access_token =
            encode(&Header::default(), &access_claims, &encoding_key).map_err(|e| {
                AppError::InternalServerErrorWithContext(format!("token encode error: {e}"))
            })?;
        let refresh_token =
            encode(&Header::default(), &refresh_claims, &encoding_key).map_err(|e| {
                AppError::InternalServerErrorWithContext(format!("token encode error: {e}"))
            })?;

        Ok(AuthResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
        })
    }
}

#[async_trait]
impl AuthServiceTrait for AuthService {
    async fn register(&self, dto: SignUpUserDto) -> AppResult<AuthResponse> {
        let email = dto
            .email
            .ok_or_else(|| AppError::BadRequest("email is required".to_string()))?;
        let name = dto
            .name
            .ok_or_else(|| AppError::BadRequest("name is required".to_string()))?;
        let password = dto
            .password
            .ok_or_else(|| AppError::BadRequest("password is required".to_string()))?;

        if self.repository.get_user_by_email(&email).await?.is_some() {
            error!("registration attempt with duplicate email: {email}");
            return Err(AppError::Conflict(format!("email {email} is taken")));
        }

        let hash = tokio::task::spawn_blocking(move || Self::hash_password(&password))
            .await
            .map_err(|e| {
                AppError::InternalServerErrorWithContext(format!("spawn_blocking error: {e}"))
            })??;

        let user = self.repository.create_user(&name, &email, &hash).await?;
        info!("registered new user: {}", user.id);

        self.generate_tokens(user.id)
    }

    async fn login(&self, dto: LoginDto) -> AppResult<AuthResponse> {
        let email = dto
            .email
            .ok_or_else(|| AppError::BadRequest("email is required".to_string()))?;
        let password = dto
            .password
            .ok_or_else(|| AppError::BadRequest("password is required".to_string()))?;

        let user = self
            .repository
            .get_user_by_email(&email)
            .await?
            .ok_or(AppError::Unauthorized)?;

        let hash = user.password.clone();
        tokio::task::spawn_blocking(move || Self::verify_password(&hash, &password))
            .await
            .map_err(|e| {
                AppError::InternalServerErrorWithContext(format!("spawn_blocking error: {e}"))
            })??;

        info!("user {} logged in", user.id);
        self.generate_tokens(user.id)
    }

    async fn refresh_token(&self, dto: RefreshDto) -> AppResult<AuthResponse> {
        let claims = self.verify_token(&dto.refresh_token)?;

        if claims.token_type != "refresh" {
            return Err(AppError::InvalidToken(
                "expected a refresh token".to_string(),
            ));
        }

        // Confirm the user still exists.
        self.repository.get_user_by_id(claims.sub).await?;

        self.generate_tokens(claims.sub)
    }

    fn verify_token(&self, token: &str) -> AppResult<TokenClaims> {
        let decoding_key = DecodingKey::from_secret(self.jwt_secret.as_bytes());
        decode::<TokenClaims>(token, &decoding_key, &Validation::default())
            .map(|data| data.claims)
            .map_err(|e| AppError::InvalidToken(format!("invalid token: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use database::user::{model::User, repository::UserRepositoryTrait};
    use std::sync::Arc;
    use uuid::Uuid;

    // ---------------------------------------------------------------------------
    // Minimal mock repository
    // ---------------------------------------------------------------------------

    struct MockRepo {
        /// The user returned by `get_user_by_email` / `get_user_by_id`.
        /// `None` simulates "user not found".
        user: Option<User>,
    }

    #[async_trait::async_trait]
    impl UserRepositoryTrait for MockRepo {
        async fn create_user(&self, name: &str, email: &str, password: &str) -> AppResult<User> {
            Ok(User {
                id: Uuid::new_v4(),
                name: name.to_string(),
                email: email.to_string(),
                password: password.to_string(),
                created_at: Some(Utc::now()),
            })
        }

        async fn get_user_by_email(&self, _email: &str) -> AppResult<Option<User>> {
            Ok(self.user.clone())
        }

        async fn get_user_by_id(&self, _id: Uuid) -> AppResult<User> {
            self.user
                .clone()
                .ok_or_else(|| AppError::NotFound("not found".to_string()))
        }

        async fn update_user(&self, _id: Uuid, _name: &str, _email: &str) -> AppResult<User> {
            unimplemented!()
        }

        async fn delete_user(&self, _id: Uuid) -> AppResult<()> {
            unimplemented!()
        }

        async fn get_all_users(&self) -> AppResult<Vec<User>> {
            unimplemented!()
        }
    }

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn make_service(user: Option<User>) -> AuthService {
        AuthService::new(
            Arc::new(MockRepo { user }),
            "test-secret".to_string(),
            3600,
            604800,
        )
    }

    fn user_with_password_hash(plaintext: &str) -> User {
        User {
            id: Uuid::new_v4(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            password: AuthService::hash_password(plaintext).unwrap(),
            created_at: Some(Utc::now()),
        }
    }

    // ---------------------------------------------------------------------------
    // Password hashing
    // ---------------------------------------------------------------------------

    #[test]
    fn hash_produces_argon2_string() {
        let hash = AuthService::hash_password("password123").unwrap();
        assert!(hash.starts_with("$argon2"), "expected argon2 PHC string");
    }

    #[test]
    fn hash_and_verify_roundtrip() {
        let hash = AuthService::hash_password("secret").unwrap();
        assert!(AuthService::verify_password(&hash, "secret").is_ok());
    }

    #[test]
    fn verify_wrong_password_returns_unauthorized() {
        let hash = AuthService::hash_password("correct").unwrap();
        let err = AuthService::verify_password(&hash, "wrong").unwrap_err();
        assert!(matches!(err, AppError::Unauthorized));
    }

    // ---------------------------------------------------------------------------
    // Token generation and verification
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn access_token_claims_are_correct() {
        let svc = make_service(None);
        let id = Uuid::new_v4();
        let tokens = svc.generate_tokens(id).unwrap();
        let claims = svc.verify_token(&tokens.access_token).unwrap();
        assert_eq!(claims.sub, id);
        assert_eq!(claims.token_type, "access");
    }

    #[tokio::test]
    async fn refresh_token_claims_are_correct() {
        let svc = make_service(None);
        let id = Uuid::new_v4();
        let tokens = svc.generate_tokens(id).unwrap();
        let claims = svc.verify_token(&tokens.refresh_token).unwrap();
        assert_eq!(claims.sub, id);
        assert_eq!(claims.token_type, "refresh");
    }

    #[test]
    fn verify_garbage_token_returns_invalid_token_error() {
        let svc = make_service(None);
        let err = svc.verify_token("not.a.jwt").unwrap_err();
        assert!(matches!(err, AppError::InvalidToken(_)));
    }

    #[test]
    fn verify_token_signed_with_wrong_secret_returns_error() {
        let svc = make_service(None);
        let other = AuthService::new(
            Arc::new(MockRepo { user: None }),
            "different-secret".to_string(),
            3600,
            604800,
        );
        let tokens = svc.generate_tokens(Uuid::new_v4()).unwrap();
        let err = other.verify_token(&tokens.access_token).unwrap_err();
        assert!(matches!(err, AppError::InvalidToken(_)));
    }

    // ---------------------------------------------------------------------------
    // register
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn register_returns_bearer_tokens() {
        let svc = make_service(None);
        let dto = crate::dtos::user_dto::SignUpUserDto {
            name: Some("Alice".to_string()),
            email: Some("alice@example.com".to_string()),
            password: Some("password123".to_string()),
        };
        let auth = svc.register(dto).await.unwrap();
        assert!(!auth.access_token.is_empty());
        assert!(!auth.refresh_token.is_empty());
        assert_eq!(auth.token_type, "Bearer");
    }

    #[tokio::test]
    async fn register_duplicate_email_returns_conflict() {
        let existing = user_with_password_hash("password123");
        let svc = make_service(Some(existing));
        let dto = crate::dtos::user_dto::SignUpUserDto {
            name: Some("Alice".to_string()),
            email: Some("alice@example.com".to_string()),
            password: Some("password123".to_string()),
        };
        let err = svc.register(dto).await.unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)));
    }

    // ---------------------------------------------------------------------------
    // login
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn login_correct_credentials_returns_tokens() {
        let existing = user_with_password_hash("password123");
        let svc = make_service(Some(existing));
        let dto = crate::dtos::auth_dto::LoginDto {
            email: Some("test@example.com".to_string()),
            password: Some("password123".to_string()),
        };
        let auth = svc.login(dto).await.unwrap();
        assert!(!auth.access_token.is_empty());
    }

    #[tokio::test]
    async fn login_wrong_password_returns_unauthorized() {
        let existing = user_with_password_hash("correct");
        let svc = make_service(Some(existing));
        let dto = crate::dtos::auth_dto::LoginDto {
            email: Some("test@example.com".to_string()),
            password: Some("wrong".to_string()),
        };
        let err = svc.login(dto).await.unwrap_err();
        assert!(matches!(err, AppError::Unauthorized));
    }

    #[tokio::test]
    async fn login_unknown_email_returns_unauthorized() {
        let svc = make_service(None);
        let dto = crate::dtos::auth_dto::LoginDto {
            email: Some("nobody@example.com".to_string()),
            password: Some("password123".to_string()),
        };
        let err = svc.login(dto).await.unwrap_err();
        assert!(matches!(err, AppError::Unauthorized));
    }

    // ---------------------------------------------------------------------------
    // refresh_token
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn refresh_with_valid_refresh_token_succeeds() {
        let user_id = Uuid::new_v4();
        let user = User {
            id: user_id,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            password: "hash".to_string(),
            created_at: Some(Utc::now()),
        };
        let svc = make_service(Some(user));
        let tokens = svc.generate_tokens(user_id).unwrap();
        let dto = crate::dtos::auth_dto::RefreshDto {
            refresh_token: tokens.refresh_token,
        };
        let result = svc.refresh_token(dto).await;
        assert!(result.is_ok());
        let new_tokens = result.unwrap();
        assert!(!new_tokens.access_token.is_empty());
    }

    #[tokio::test]
    async fn refresh_with_access_token_returns_invalid_token_error() {
        let svc = make_service(None);
        let tokens = svc.generate_tokens(Uuid::new_v4()).unwrap();
        let dto = crate::dtos::auth_dto::RefreshDto {
            // Intentionally passing an access token where a refresh token is expected.
            refresh_token: tokens.access_token,
        };
        let err = svc.refresh_token(dto).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidToken(_)));
    }
}
