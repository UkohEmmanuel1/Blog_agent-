use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Article {
    pub id: Uuid,
    pub story_id: Option<Uuid>,
    pub title: String,
    pub slug: String,
    pub dek: Option<String>,
    pub body_markdown: String,
    pub body_html: Option<String>,
    pub status: String,
    pub author_id: Option<Uuid>,
    pub editor_id: Option<Uuid>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ArticleRevision {
    pub id: Uuid,
    pub article_id: Uuid,
    pub revision_number: i32,
    pub title: String,
    pub body_markdown: String,
    pub change_summary: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Approval {
    pub id: Uuid,
    pub resource_type: String,
    pub resource_id: Uuid,
    pub requested_by: Option<Uuid>,
    pub reviewer_id: Option<Uuid>,
    pub status: String,
    pub review_notes: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
