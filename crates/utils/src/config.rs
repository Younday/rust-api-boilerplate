#[derive(clap::ValueEnum, Clone, Debug, Copy)]
pub enum CargoEnv {
    Development,
    Production,
}

#[derive(clap::Parser)]
#[allow(clippy::module_name_repetitions)]
pub struct AppConfig {
    #[clap(long, env, value_enum)]
    pub cargo_env: CargoEnv,

    #[clap(long, env, default_value = "127.0.0.1")]
    pub app_host: String,

    #[clap(long, env, default_value = "8080")]
    pub app_port: u16,

    #[clap(
        long,
        env,
        default_value = "postgresql://admin:admin@localhost:5432/rust?schema=public"
    )]
    pub postgres_uri: String,

    #[clap(long, env)]
    pub jwt_secret: String,

    #[clap(long, env, default_value = "3600")]
    pub jwt_access_expiration_secs: i64,

    #[clap(long, env, default_value = "604800")]
    pub jwt_refresh_expiration_secs: i64,
}

impl AppConfig {
    /// Validates security-sensitive config at startup.
    /// Returns an error if the config is unsafe for the current environment.
    ///
    /// # Errors
    /// Returns an error string if production config fails validation.
    pub fn validate(&self) -> Result<(), String> {
        if matches!(self.cargo_env, CargoEnv::Production) && self.jwt_secret.len() < 32 {
            return Err(
                "JWT_SECRET must be at least 32 characters in production".to_string(),
            );
        }
        Ok(())
    }
}
