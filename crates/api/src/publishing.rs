use crate::{auth::AuthenticatedUser, state::AppState};
use atlas_core::models::{Approval, ArticleRevision, SocialPost};
use axum::{
    extract::{DefaultBodyLimit, Extension, Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
struct SocialAccountView {
    id: Uuid,
    platform: String,
    account_name: String,
    platform_account_id: Option<String>,
    is_active: bool,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ConnectAccountRequest {
    platform: String,
    account_name: String,
    platform_account_id: Option<String>,
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct RotateTokenRequest {
    access_token: String,
    token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct CreateSocialPostRequest {
    social_account_id: Uuid,
    article_id: Option<Uuid>,
    content: String,
    scheduled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct CreateRevisionRequest {
    title: String,
    body_markdown: String,
    change_summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DecisionRequest {
    decision: String,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PageQuery {
    limit: Option<i64>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UploadMediaRequest {
    filename: String,
    content_type: String,
    base64_data: String,
    article_id: Option<Uuid>,
    alt_text: Option<String>,
}

async fn app_user(state: &AppState, identity: &AuthenticatedUser) -> Result<atlas_core::models::User, (StatusCode, Json<Value>)> {
    sqlx::query_as::<_, atlas_core::models::User>(
        "insert into public.users (id,email) values ($1,$2) on conflict (id) do update set email=coalesce(excluded.email,public.users.email), updated_at=now() returning id,email,display_name,role,created_at,updated_at"
    ).bind(identity.id).bind(identity.email.as_deref()).fetch_one(&state.db).await
      .map_err(internal)
}

fn internal<E: std::fmt::Display>(error: E) -> (StatusCode, Json<Value>) {
    tracing::error!(%error, "editorial API operation failed");
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"internal_error","message":"Operation failed"})))
}

fn forbidden() -> (StatusCode, Json<Value>) {
    (StatusCode::FORBIDDEN, Json(json!({"error":"forbidden","message":"Insufficient permissions"})))
}

fn bad_request(message: &str) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({"error":"validation_error","message":message})))
}


async fn list_approvals(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Vec<Approval>>, (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "reviewer") { return Err(forbidden()); }
    if let Some(status) = query.status.as_deref() {
        if !matches!(status, "pending" | "approved" | "rejected" | "changes_requested" | "cancelled") {
            return Err(bad_request("Invalid approval status filter."));
        }
    }
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let rows = sqlx::query_as::<_, Approval>(
        "select * from public.approvals where ($1::text is null or status=$1) order by created_at desc limit $2"
    ).bind(query.status).bind(limit).fetch_all(&state.db).await.map_err(internal)?;
    Ok(Json(rows))
}

async fn list_social_posts(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Vec<SocialPost>>, (StatusCode, Json<Value>)> {
    app_user(&state, &identity).await?;
    if let Some(status) = query.status.as_deref() {
        if !matches!(status, "draft" | "pending_approval" | "scheduled" | "publishing" | "published" | "failed" | "cancelled") {
            return Err(bad_request("Invalid social post status filter."));
        }
    }
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let rows = sqlx::query_as::<_, SocialPost>(
        r#"select * from public.social_posts
           where ($1::text is null or status=$1)
           order by created_at desc limit $2"#
    ).bind(query.status).bind(limit).fetch_all(&state.db).await.map_err(internal)?;
    Ok(Json(rows))
}

async fn list_article_revisions(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Path(article_id): Path<Uuid>,
) -> Result<Json<Vec<ArticleRevision>>, (StatusCode, Json<Value>)> {
    app_user(&state, &identity).await?;
    let rows = sqlx::query_as::<_, ArticleRevision>(
        "select * from public.article_revisions where article_id=$1 order by revision_number desc"
    ).bind(article_id).fetch_all(&state.db).await.map_err(internal)?;
    Ok(Json(rows))
}

async fn list_accounts(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
) -> Result<Json<Vec<SocialAccountView>>, (StatusCode, Json<Value>)> {
    app_user(&state, &identity).await?;
    let rows = sqlx::query_as::<_, SocialAccountView>(
        "select id,platform,account_name,platform_account_id,is_active,created_at from public.social_accounts order by created_at desc"
    ).fetch_all(&state.db).await.map_err(internal)?;
    Ok(Json(rows))
}

async fn connect_account(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Json(request): Json<ConnectAccountRequest>,
) -> Result<(StatusCode, Json<SocialAccountView>), (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "editor") { return Err(forbidden()); }
    let platform = request.platform.to_ascii_lowercase();
    if !matches!(platform.as_str(), "x" | "twitter" | "linkedin") {
        return Err(bad_request("Supported platforms are x and linkedin."));
    }
    if request.access_token.trim().is_empty() || request.account_name.trim().is_empty() {
        return Err(bad_request("account_name and access_token are required."));
    }
    let key = std::env::var("SOCIAL_TOKEN_ENCRYPTION_KEY")
        .map_err(|_| bad_request("SOCIAL_TOKEN_ENCRYPTION_KEY is not configured on the API server."))?;
    let row = sqlx::query_as::<_, SocialAccountView>(
        r#"insert into public.social_accounts
             (platform,account_name,platform_account_id,access_token_encrypted,connected_by)
           values ($1,$2,$3,encode(pgp_sym_encrypt($4,$5),'base64'),$6)
           returning id,platform,account_name,platform_account_id,is_active,created_at"#
    ).bind(platform).bind(request.account_name).bind(request.platform_account_id)
     .bind(request.access_token).bind(key).bind(user.id)
     .fetch_one(&state.db).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(row)))
}


async fn rotate_account_token(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Path(account_id): Path<Uuid>,
    Json(request): Json<RotateTokenRequest>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "editor") { return Err(forbidden()); }
    if request.access_token.trim().is_empty() { return Err(bad_request("access_token is required.")); }
    let key = std::env::var("SOCIAL_TOKEN_ENCRYPTION_KEY")
        .map_err(|_| bad_request("SOCIAL_TOKEN_ENCRYPTION_KEY is not configured on the API server."))?;
    let changed = sqlx::query(
        "update public.social_accounts set access_token_encrypted=encode(pgp_sym_encrypt($2,$3),'base64'),token_expires_at=$4,updated_at=now() where id=$1 and is_active=true"
    ).bind(account_id).bind(request.access_token).bind(key).bind(request.token_expires_at)
     .execute(&state.db).await.map_err(internal)?;
    if changed.rows_affected() == 0 { return Err(bad_request("Social account not found or inactive.")); }
    Ok(StatusCode::NO_CONTENT)
}

async fn create_social_post(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Json(request): Json<CreateSocialPostRequest>,
) -> Result<(StatusCode, Json<SocialPost>), (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "editor") { return Err(forbidden()); }
    if request.content.trim().is_empty() || request.content.len() > 20_000 {
        return Err(bad_request("Post content must contain between 1 and 20,000 bytes."));
    }
    let row = sqlx::query_as::<_, SocialPost>(
        r#"insert into public.social_posts (article_id,social_account_id,content,status,scheduled_at)
           select $1,id,$3,'draft',$4 from public.social_accounts where id=$2 and is_active=true
           returning id,article_id,social_account_id,platform_post_id,content,status,scheduled_at,
                     published_at,platform_url,error_message,metadata,created_at,updated_at"#
    ).bind(request.article_id).bind(request.social_account_id).bind(request.content)
     .bind(request.scheduled_at).fetch_optional(&state.db).await.map_err(internal)?
     .ok_or_else(|| bad_request("The selected social account does not exist or is inactive."))?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn submit_article(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Path(article_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "editor") { return Err(forbidden()); }
    let mut tx = state.db.begin().await.map_err(internal)?;
    let changed = sqlx::query("update public.articles set status='in_review', updated_at=now() where id=$1 and status in ('draft','rejected') returning id")
        .bind(article_id).fetch_optional(&mut *tx).await.map_err(internal)?;
    if changed.is_none() { return Err(bad_request("Article not found or not eligible for review.")); }
    let approval_id = Uuid::new_v4();
    sqlx::query("insert into public.approvals (id,resource_type,resource_id,requested_by,status) values ($1,'article',$2,$3,'pending')")
        .bind(approval_id).bind(article_id).bind(user.id).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok((StatusCode::ACCEPTED, Json(json!({"approval_id":approval_id,"article_id":article_id,"status":"pending"}))))
}

async fn submit_social_post(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Path(post_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "editor") { return Err(forbidden()); }
    let mut tx = state.db.begin().await.map_err(internal)?;
    let changed = sqlx::query("update public.social_posts set status='pending_approval', updated_at=now() where id=$1 and status='draft' returning id")
        .bind(post_id).fetch_optional(&mut *tx).await.map_err(internal)?;
    if changed.is_none() { return Err(bad_request("Social post not found or not eligible for approval.")); }
    let approval_id = Uuid::new_v4();
    sqlx::query("insert into public.approvals (id,resource_type,resource_id,requested_by,status) values ($1,'social_post',$2,$3,'pending')")
        .bind(approval_id).bind(post_id).bind(user.id).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok((StatusCode::ACCEPTED, Json(json!({"approval_id":approval_id,"social_post_id":post_id,"status":"pending"}))))
}

async fn decide_approval(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Path(approval_id): Path<Uuid>,
    Json(request): Json<DecisionRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "reviewer") { return Err(forbidden()); }
    if !matches!(request.decision.as_str(), "approved" | "rejected" | "changes_requested") {
        return Err(bad_request("decision must be approved, rejected, or changes_requested."));
    }

    let mut tx = state.db.begin().await.map_err(internal)?;
    let approval = sqlx::query_as::<_, (String, Uuid, String, Option<Uuid>)>(
        "select resource_type,resource_id,status,requested_by from public.approvals where id=$1 for update"
    ).bind(approval_id).fetch_optional(&mut *tx).await.map_err(internal)?
      .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))))?;
    if approval.2 != "pending" { return Err(bad_request("This approval has already been decided.")); }
    if approval.3 == Some(user.id) { return Err(bad_request("A requester cannot approve their own submission.")); }

    sqlx::query("update public.approvals set status=$2,reviewer_id=$3,review_notes=$4,reviewed_at=now(),updated_at=now() where id=$1")
        .bind(approval_id).bind(&request.decision).bind(user.id).bind(&request.notes)
        .execute(&mut *tx).await.map_err(internal)?;

    match approval.0.as_str() {
        "article" => {
            let status = match request.decision.as_str() {
                "approved" => "approved",
                "rejected" => "rejected",
                _ => "draft",
            };
            sqlx::query("update public.articles set status=$2,editor_id=$3,updated_at=now() where id=$1")
                .bind(approval.1).bind(status).bind(user.id).execute(&mut *tx).await.map_err(internal)?;
        }
        "social_post" => {
            if request.decision == "approved" {
                let post = sqlx::query_as::<_, (Option<DateTime<Utc>>,)>(
                    "update public.social_posts set status='scheduled',approved_at=now(),approved_by=$2,updated_at=now() where id=$1 and status='pending_approval' returning scheduled_at"
                ).bind(approval.1).bind(user.id).fetch_optional(&mut *tx).await.map_err(internal)?
                 .ok_or_else(|| bad_request("Social post is no longer awaiting approval."))?;
                let run_id = Uuid::new_v4();
                let outbox_id = Uuid::new_v4();
                let job = atlas_core::jobs::AgentRunJob {
                    outbox_id,
                    agent_run_id: run_id,
                    task_type: "social.publish".to_owned(),
                    input: json!({"social_post_id":approval.1}),
                };
                let payload = serde_json::to_value(job).map_err(internal)?;
                sqlx::query("insert into public.agent_runs (id,initiated_by,task_type,status,input) values ($1,$2,'social.publish','queued',$3)")
                    .bind(run_id).bind(user.id).bind(json!({"social_post_id":approval.1}))
                    .execute(&mut *tx).await.map_err(internal)?;
                sqlx::query("insert into public.job_outbox (id,topic,aggregate_id,payload,available_at) values ($1,'social.publish',$2,$3,coalesce($4,now()))")
                    .bind(outbox_id).bind(run_id).bind(payload).bind(post.0)
                    .execute(&mut *tx).await.map_err(internal)?;
            } else {
                let status = if request.decision == "rejected" { "cancelled" } else { "draft" };
                sqlx::query("update public.social_posts set status=$2,updated_at=now() where id=$1")
                    .bind(approval.1).bind(status).execute(&mut *tx).await.map_err(internal)?;
            }
        }
        _ => return Err(bad_request("Unsupported approval resource type.")),
    }
    tx.commit().await.map_err(internal)?;
    Ok(Json(json!({"approval_id":approval_id,"status":request.decision})))
}

async fn create_revision(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Path(article_id): Path<Uuid>,
    Json(request): Json<CreateRevisionRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "editor") { return Err(forbidden()); }
    if request.title.trim().is_empty() || request.body_markdown.trim().is_empty() {
        return Err(bad_request("title and body_markdown are required."));
    }
    let mut tx = state.db.begin().await.map_err(internal)?;
    let locked = sqlx::query("select id from public.articles where id=$1 and status in ('draft','rejected','approved') for update")
        .bind(article_id).fetch_optional(&mut *tx).await.map_err(internal)?;
    if locked.is_none() { return Err(bad_request("Article not found or locked after publication.")); }
    let revision_number: i32 = sqlx::query_scalar(
        "select coalesce(max(revision_number),0)+1 from public.article_revisions where article_id=$1"
    ).bind(article_id).fetch_one(&mut *tx).await.map_err(internal)?;
    let changed = sqlx::query("update public.articles set title=$2,body_markdown=$3,status='draft',updated_at=now() where id=$1 and status in ('draft','rejected','approved') returning id")
        .bind(article_id).bind(&request.title).bind(&request.body_markdown)
        .fetch_optional(&mut *tx).await.map_err(internal)?;
    if changed.is_none() { return Err(bad_request("Article not found or locked after publication.")); }
    let revision_id = Uuid::new_v4();
    sqlx::query("insert into public.article_revisions (id,article_id,revision_number,title,body_markdown,change_summary,created_by) values ($1,$2,$3,$4,$5,$6,$7)")
        .bind(revision_id).bind(article_id).bind(revision_number).bind(request.title)
        .bind(request.body_markdown).bind(request.change_summary).bind(user.id)
        .execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(json!({"revision_id":revision_id,"article_id":article_id,"revision_number":revision_number,"status":"draft"}))))
}

async fn upload_media(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Json(request): Json<UploadMediaRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "editor") { return Err(forbidden()); }
    if request.base64_data.len() > 14_000_000 { return Err(bad_request("Media upload exceeds the 10 MB limit.")); }
    let allowed = ["image/jpeg","image/png","image/webp","video/mp4","audio/mpeg","application/pdf"];
    if !allowed.contains(&request.content_type.as_str()) { return Err(bad_request("Unsupported media content type.")); }
    let bytes = BASE64.decode(&request.base64_data).map_err(|_| bad_request("base64_data is invalid."))?;
    if bytes.len() > 10 * 1024 * 1024 { return Err(bad_request("Media upload exceeds the 10 MB limit.")); }
    let extension = match request.content_type.as_str() {
        "image/jpeg" => "jpg", "image/png" => "png", "image/webp" => "webp",
        "video/mp4" => "mp4", "audio/mpeg" => "mp3", _ => "pdf",
    };
    let media_type = match request.content_type.split('/').next().unwrap_or("") {
        "image" => "image", "video" => "video", "audio" => "audio", _ => "document",
    };
    let asset_id = Uuid::new_v4();
    let key = format!("media/{}/{}.{}", user.id, asset_id, extension);
    let storage = atlas_agents::storage::S3Storage::from_env().await.map_err(internal)?;
    storage.put(&key, bytes.clone(), &request.content_type).await.map_err(internal)?;
    let row = sqlx::query_as::<_, (Uuid, String, String, String, Option<String>, Option<i64>)>(
        "insert into public.media_assets (id,article_id,storage_bucket,storage_path,media_type,mime_type,file_size_bytes,alt_text,uploaded_by,metadata) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) returning id,storage_bucket,storage_path,media_type,mime_type,file_size_bytes"
    ).bind(asset_id).bind(request.article_id).bind(storage.bucket()).bind(&key)
     .bind(media_type).bind(&request.content_type).bind(bytes.len() as i64)
     .bind(request.alt_text).bind(user.id).bind(json!({"original_filename":request.filename})).fetch_one(&state.db).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(json!({
        "id":row.0,"bucket":row.1,"path":row.2,"media_type":row.3,
        "mime_type":row.4,"file_size_bytes":row.5,"status":"stored"
    }))))
}


async fn signed_media_url(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Path(asset_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user = app_user(&state, &identity).await?;
    let asset = sqlx::query_as::<_, (String, Option<Uuid>)>(
        "select storage_path,uploaded_by from public.media_assets where id=$1"
    ).bind(asset_id).fetch_optional(&state.db).await.map_err(internal)?
      .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))))?;
    if !matches!(user.role.as_str(), "admin" | "editor" | "reviewer") && asset.1 != Some(user.id) {
        return Err(forbidden());
    }
    let storage = atlas_agents::storage::S3Storage::from_env().await.map_err(internal)?;
    let url = storage.signed_get_url(&asset.0).await.map_err(internal)?;
    Ok(Json(json!({"asset_id":asset_id,"url":url,"expires_in_seconds":900})))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/social-accounts", get(list_accounts).post(connect_account))
        .route("/social-accounts/{account_id}/credentials", put(rotate_account_token))
        .route("/social-posts", get(list_social_posts).post(create_social_post))
        .route("/approvals", get(list_approvals))
        .route("/articles/{article_id}/revisions", get(list_article_revisions).post(create_revision))
        .route("/social-posts/{post_id}/submit-for-review", post(submit_social_post))
        .route("/articles/{article_id}/submit-for-review", post(submit_article))
        .route("/approvals/{approval_id}/decision", post(decide_approval))
        .route("/media", post(upload_media).layer(DefaultBodyLimit::max(15 * 1024 * 1024)))
        .route("/media/{asset_id}/url", get(signed_media_url))
}
