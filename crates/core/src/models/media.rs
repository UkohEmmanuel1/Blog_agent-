use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MediaAsset {
    pub id: Uuid,
    pub article_id: Option<Uuid>,
    pub storage_bucket: String,
    pub storage_path: String,
    pub public_url: Option<String>,
    pub media_type: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub alt_text: Option<String>,
    pub metadata: Value,
    pub uploaded_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
