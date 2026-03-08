use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use async_trait::async_trait;
use chrono::Utc;
use database::{
    refresh_token::repository::DynRefreshTokenRepository, user::repository::DynUserRepository,
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use tracing::{error, info, warn};
use utils::{AppError, AppResult};
use uuid::Uuid;

use crate::dtos::{
    auth_dto::{AuthResponse, LoginDto, RefreshDto, TokenClaims, TokenType},
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
    refresh_token_repo: DynRefreshTokenRepository,
    jwt_secret: String,
    access_exp_secs: i64,
    refresh_exp_secs: i64,
}

impl AuthService {
    pub fn new(
        repository: DynUserRepository,
        refresh_token_repo: DynRefreshTokenRepository,
        jwt_secret: String,
        access_exp_secs: i64,
        refresh_exp_secs: i64,
    ) -> Self {
        Self {
            repository,
            refresh_token_repo,
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

    /// Returns a stable dummy hash used for timing equalization during login
    /// when no matching user exists — prevents email enumeration via timing.
    fn dummy_hash() -> &'static str {
        use std::sync::OnceLock;
        static DUMMY: OnceLock<String> = OnceLock::new();
        DUMMY.get_or_init(|| {
            Self::hash_password("dummy_timing_password_placeholder_xyz!")
                .expect("dummy hash must succeed")
        })
    }

    async fn generate_tokens(&self, user_id: Uuid) -> AppResult<AuthResponse> {
        let now = Utc::now();
        let now_ts = now.timestamp();
        let encoding_key = EncodingKey::from_secret(self.jwt_secret.as_bytes());

        let jti = Uuid::new_v4();

        let access_claims = TokenClaims {
            sub: user_id,
            exp: usize::try_from(now_ts + self.access_exp_secs).unwrap_or(usize::MAX),
            iat: usize::try_from(now_ts).unwrap_or(0),
            token_type: TokenType::Access,
            jti: None,
        };
        let refresh_claims = TokenClaims {
            sub: user_id,
            exp: usize::try_from(now_ts + self.refresh_exp_secs).unwrap_or(usize::MAX),
            iat: usize::try_from(now_ts).unwrap_or(0),
            token_type: TokenType::Refresh,
            jti: Some(jti),
        };

        let access_token =
            encode(&Header::default(), &access_claims, &encoding_key).map_err(|e| {
                error!("failed to encode access token: {e}");
                AppError::InternalServerError
            })?;
        let refresh_token =
            encode(&Header::default(), &refresh_claims, &encoding_key).map_err(|e| {
                error!("failed to encode refresh token: {e}");
                AppError::InternalServerError
            })?;

        let expires_at = now + chrono::Duration::seconds(self.refresh_exp_secs);
        self.refresh_token_repo
            .store(jti, user_id, expires_at)
            .await?;

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
            warn!("registration attempt with duplicate email");
            return Err(AppError::Conflict(format!("email {email} is taken")));
        }

        let hash = tokio::task::spawn_blocking(move || Self::hash_password(&password))
            .await
            .map_err(|e| {
                error!("spawn_blocking error: {e}");
                AppError::InternalServerError
            })??;

        let user = self.repository.create_user(&name, &email, &hash).await?;
        info!("registered new user: {}", user.id);

        self.generate_tokens(user.id).await
    }

    async fn login(&self, dto: LoginDto) -> AppResult<AuthResponse> {
        let email = dto
            .email
            .ok_or_else(|| AppError::BadRequest("email is required".to_string()))?;
        let password = dto
            .password
            .ok_or_else(|| AppError::BadRequest("password is required".to_string()))?;

        let user_opt = self.repository.get_user_by_email(&email).await?;

        if let Some(user) = user_opt {
            let hash = user.password.clone();
            tokio::task::spawn_blocking(move || Self::verify_password(&hash, &password))
                .await
                .map_err(|e| {
                    error!("spawn_blocking error: {e}");
                    AppError::InternalServerError
                })??;

            info!("user {} logged in", user.id);
            self.generate_tokens(user.id).await
        } else {
            // Run a dummy verify to equalize timing and prevent email enumeration.
            let dummy = Self::dummy_hash().to_string();
            let _ =
                tokio::task::spawn_blocking(move || Self::verify_password(&dummy, &password)).await;
            Err(AppError::Unauthorized)
        }
    }

    async fn refresh_token(&self, dto: RefreshDto) -> AppResult<AuthResponse> {
        let claims = self.verify_token(&dto.refresh_token)?;

        if claims.token_type != TokenType::Refresh {
            return Err(AppError::InvalidToken(
                "expected a refresh token".to_string(),
            ));
        }

        let jti = claims
            .jti
            .ok_or_else(|| AppError::InvalidToken("missing jti".to_string()))?;

        // Revoke the JTI atomically — fails if already used or expired.
        let revoked = self.refresh_token_repo.revoke(jti).await?;
        if !revoked {
            return Err(AppError::InvalidToken(
                "refresh token has already been used or has expired".to_string(),
            ));
        }

        // Confirm the user still exists.
        self.repository.get_user_by_id(claims.sub).await?;

        self.generate_tokens(claims.sub).await
    }

    fn verify_token(&self, token: &str) -> AppResult<TokenClaims> {
        let decoding_key = DecodingKey::from_secret(self.jwt_secret.as_bytes());
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_required_spec_claims(&["exp", "sub"]);
        decode::<TokenClaims>(token, &decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| {
                error!("token verification failed: {e}");
                AppError::InvalidToken("invalid token".to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use database::user::{model::User, repository::UserRepositoryTrait};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    // ---------------------------------------------------------------------------
    // Minimal mock repositories
    // ---------------------------------------------------------------------------

    struct MockUserRepo {
        user: Option<User>,
    }

    #[async_trait::async_trait]
    impl UserRepositoryTrait for MockUserRepo {
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

    struct MockRefreshTokenRepo {
        stored: Mutex<std::collections::HashSet<Uuid>>,
    }

    impl MockRefreshTokenRepo {
        fn new() -> Self {
            Self {
                stored: Mutex::new(std::collections::HashSet::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl database::refresh_token::repository::RefreshTokenRepositoryTrait for MockRefreshTokenRepo {
        async fn store(
            &self,
            jti: Uuid,
            _user_id: Uuid,
            _expires_at: chrono::DateTime<Utc>,
        ) -> AppResult<()> {
            self.stored.lock().unwrap().insert(jti);
            Ok(())
        }

        async fn revoke(&self, jti: Uuid) -> AppResult<bool> {
            Ok(self.stored.lock().unwrap().remove(&jti))
        }
    }

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn make_service(user: Option<User>) -> AuthService {
        AuthService::new(
            Arc::new(MockUserRepo { user }),
            Arc::new(MockRefreshTokenRepo::new()),
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
        let tokens = svc.generate_tokens(id).await.unwrap();
        let claims = svc.verify_token(&tokens.access_token).unwrap();
        assert_eq!(claims.sub, id);
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[tokio::test]
    async fn refresh_token_claims_are_correct() {
        let svc = make_service(None);
        let id = Uuid::new_v4();
        let tokens = svc.generate_tokens(id).await.unwrap();
        let claims = svc.verify_token(&tokens.refresh_token).unwrap();
        assert_eq!(claims.sub, id);
        assert_eq!(claims.token_type, TokenType::Refresh);
        assert!(claims.jti.is_some(), "refresh token must carry a jti");
    }

    #[test]
    fn verify_garbage_token_returns_invalid_token_error() {
        let svc = make_service(None);
        let err = svc.verify_token("not.a.jwt").unwrap_err();
        assert!(matches!(err, AppError::InvalidToken(_)));
    }

    #[test]
    fn verify_token_signed_with_wrong_secret_returns_error() {
        let _svc = make_service(None);
        let other = AuthService::new(
            Arc::new(MockUserRepo { user: None }),
            Arc::new(MockRefreshTokenRepo::new()),
            "different-secret".to_string(),
            3600,
            604800,
        );
        let id = Uuid::new_v4();
        // Can't call generate_tokens here (async) — build a token manually.
        let encoding_key = EncodingKey::from_secret(b"test-secret");
        let claims = TokenClaims {
            sub: id,
            exp: usize::MAX,
            iat: 0,
            token_type: TokenType::Access,
            jti: None,
        };
        let token =
            encode(&Header::default(), &claims, &encoding_key).expect("encoding must succeed");
        let err = other.verify_token(&token).unwrap_err();
        assert!(matches!(err, AppError::InvalidToken(_)));
    }

    #[test]
    fn verify_expired_token_returns_invalid_token_error() {
        let svc = make_service(None);
        let encoding_key = EncodingKey::from_secret(b"test-secret");
        let claims = TokenClaims {
            sub: Uuid::new_v4(),
            exp: 0, // already expired
            iat: 0,
            token_type: TokenType::Access,
            jti: None,
        };
        let token =
            encode(&Header::default(), &claims, &encoding_key).expect("encoding must succeed");
        let err = svc.verify_token(&token).unwrap_err();
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
        let tokens = svc.generate_tokens(user_id).await.unwrap();
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
        let tokens = svc.generate_tokens(Uuid::new_v4()).await.unwrap();
        let dto = crate::dtos::auth_dto::RefreshDto {
            // Intentionally passing an access token where a refresh token is expected.
            refresh_token: tokens.access_token,
        };
        let err = svc.refresh_token(dto).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidToken(_)));
    }

    #[tokio::test]
    async fn refresh_token_rotation_prevents_reuse() {
        let user_id = Uuid::new_v4();
        let user = User {
            id: user_id,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            password: "hash".to_string(),
            created_at: Some(Utc::now()),
        };
        let svc = make_service(Some(user));
        let tokens = svc.generate_tokens(user_id).await.unwrap();
        let dto = crate::dtos::auth_dto::RefreshDto {
            refresh_token: tokens.refresh_token.clone(),
        };
        // First use should succeed.
        svc.refresh_token(dto).await.unwrap();
        // Second use with the same token must be rejected.
        let dto2 = crate::dtos::auth_dto::RefreshDto {
            refresh_token: tokens.refresh_token,
        };
        let err = svc.refresh_token(dto2).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidToken(_)));
    }
}
