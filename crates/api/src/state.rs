use apalis_redis::RedisStorage;
use sqlx::PgPool;
use crate::auth::AuthVerifier;
use atlas_core::jobs::AgentRunJob;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub auth: AuthVerifier,
    pub queue: Arc<Mutex<RedisStorage<AgentRunJob>>>,
}
