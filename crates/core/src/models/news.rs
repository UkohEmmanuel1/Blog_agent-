use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NewsSource {
    pub id: Uuid,
    pub name: String,
    pub source_type: String,
    pub url: Option<String>,
    pub feed_url: Option<String>,
    pub is_active: bool,
    pub config: Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Story {
    pub id: Uuid,
    pub source_id: Option<Uuid>,
    pub external_id: Option<String>,
    pub canonical_url: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub author_name: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub discovered_at: DateTime<Utc>,
    pub language: String,
    pub status: String,
    pub raw_payload: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StoryClaim {
    pub id: Uuid,
    pub story_id: Uuid,
    pub claim_text: String,
    pub claim_type: String,
    pub confidence: Option<rust_decimal::Decimal>,
    pub verification_status: String,
    pub evidence: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
