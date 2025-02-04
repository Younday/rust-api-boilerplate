pub(crate) mod api;
pub(crate) mod app;
pub(crate) mod logger;
pub(crate) mod router;

use anyhow::{Context, Result};
use app::ApplicationServer;
use clap::Parser;
use dotenvy::dotenv;
use std::sync::Arc;
use utils::AppConfig;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();

    let config = Arc::new(AppConfig::parse());

    ApplicationServer::serve(config)
        .await
        .context("Failed to start server")?;

    Ok(())
}
