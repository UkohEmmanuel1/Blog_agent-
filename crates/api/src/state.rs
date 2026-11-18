use crate::auth::AuthVerifier;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub auth: AuthVerifier,
}
