use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub before_state: Option<Value>,
    pub after_state: Option<Value>,
    pub ip_address: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}
