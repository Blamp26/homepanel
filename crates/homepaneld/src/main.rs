mod api;
mod app;
mod auth;
mod config;
mod db;
mod state;
mod static_files;
mod ws;

use app::run;
use homepanel_core::load_config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = load_config(None)?;
    run(config).await?;
    Ok(())
}
