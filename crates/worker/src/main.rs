use anyhow::{Context, Result};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{env, str::FromStr, time::Duration};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();

    let database_url = env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    // Disable prepared-statement caching for Supabase transaction-pooler compatibility.
    let options = PgConnectOptions::from_str(&database_url)?.statement_cache_capacity(0);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await
        .context("failed to connect to PostgreSQL")?;

    sqlx::query("SELECT 1").execute(&pool).await?;
    tracing::info!("ATLAS worker foundation initialized; queue consumers arrive in Phase 3");
    tokio::signal::ctrl_c().await?;
    pool.close().await;
    Ok(())
}
