use homepanel_core::error::{ApiError, Result};
use sqlx::SqlitePool;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

pub async fn run(pool: &SqlitePool) -> Result<()> {
    MIGRATOR
        .run(pool)
        .await
        .map_err(|err| ApiError::Database(err.to_string()))?;
    Ok(())
}
