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
    /// This function will return an error if the `MongoDB` client cannot be initialized
    /// or if the specified database or collection cannot be accessed.
    pub async fn new(config: Arc<AppConfig>) -> AppResult<Self> {
        let database_url = &config.postgres_uri;
        let pool = match PgPoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
        {
            Ok(pool) => {
                info!("✅Connection to the database is successful!");
                pool
            }
            Err(err) => {
                info!("🔥 Failed to connect to the database: {:?}", err);
                std::process::exit(1);
            }
        };
        Ok(Database { db: pool })
    }
}
