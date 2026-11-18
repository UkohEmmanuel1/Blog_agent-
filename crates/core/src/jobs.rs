use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Serializable payload persisted in the PostgreSQL outbox and delivered through Apalis/Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunJob {
    pub outbox_id: Uuid,
    pub agent_run_id: Uuid,
    pub task_type: String,
    pub input: Value,
}
