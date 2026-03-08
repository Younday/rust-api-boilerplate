pub mod refresh_token;
pub mod user;

use std::sync::Arc;

use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tracing::info;
use utils::{AppConfig, AppResult};

#[derive(Clone, Debug)]
pub struct Database {
    pub db: Pool<Postgres>,
}

impl Database {
    /// Creates a new `Database` instance.
    ///
    /// # Arguments
    ///
    /// * `config` - An `Arc` containing the application configuration.
    ///
    /// # Returns
    ///
    /// * `AppResult<Self>` - A result containing the `Database` instance or an error.
    ///
    /// # Errors
    ///
    /// This function will return an error if the PostgreSQL connection pool cannot be initialized.
    pub async fn new(config: Arc<AppConfig>) -> AppResult<Self> {
        let database_url = &config.postgres_uri;
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|err| {
                tracing::error!("🔥 Failed to connect to the database: {:?}", err);
                err
            })?;

        info!("✅Connection to the database is successful!");
        Ok(Database { db: pool })
    }
}
