pub mod migrations;
pub mod models;

use homepanel_core::error::Result;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

pub async fn connect(database_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .map_err(|err| homepanel_core::error::ApiError::Database(err.to_string()))?;
    migrations::run(&pool).await?;
    Ok(pool)
}
