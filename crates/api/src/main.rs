mod auth;
mod routes;
mod state;

use anyhow::{Context, Result};
use routes::router;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use state::AppState;
use std::{env, str::FromStr, time::Duration};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();

    let database_url = env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let options = PgConnectOptions::from_str(&database_url)?.statement_cache_capacity(0);
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await
        .context("failed to connect to PostgreSQL")?;
    sqlx::query("SELECT 1").execute(&pool).await?;

    let supabase_url = env::var("SUPABASE_URL").context("SUPABASE_URL must be set")?;
    let auth = auth::AuthVerifier::from_supabase_url(&supabase_url)
        .await
        .context("failed to load Supabase JWT signing keys")?;
    let app = router(AppState { db: pool, auth }).layer(TraceLayer::new_for_http());

    let host = env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_owned());
    let port = env::var("API_PORT").unwrap_or_else(|_| "8080".to_owned());
    let address = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&address).await?;
    tracing::info!(%address, "ATLAS API listening");
    axum::serve(listener, app).await?;
    Ok(())
}
