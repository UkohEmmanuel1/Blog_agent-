use crate::{
    auth::{require_auth, AuthenticatedUser},
    state::AppState,
};
use atlas_core::{
    jobs::AgentRunJob,
    models::{AgentRun, Article, Story, User},
};
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

async fn health() -> Json<Value> {
    Json(json!({"status":"ok","service":"atlas-api"}))
}

async fn ensure_user(state: &AppState, identity: &AuthenticatedUser) -> Result<User, ApiError> {
    sqlx::query_as::<_, User>(
        r#"insert into public.users (id, email)
           values ($1, $2)
           on conflict (id) do update
             set email = coalesce(excluded.email, public.users.email),
                 updated_at = now()
           returning id, email, display_name, role, created_at, updated_at"#,
    )
    .bind(identity.id)
    .bind(identity.email.as_deref())
    .fetch_one(&state.db)
    .await
    .map_err(ApiError::from)
}

async fn me(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
) -> Result<Json<User>, ApiError> {
    Ok(Json(ensure_user(&state, &identity).await?))
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    limit: Option<i64>,
}

async fn list_stories(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<Story>>, ApiError> {
    ensure_user(&state, &identity).await?;
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let rows = sqlx::query_as::<_, Story>(
        "select * from public.stories order by discovered_at desc limit $1",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn list_articles(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<Article>>, ApiError> {
    ensure_user(&state, &identity).await?;
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let rows = sqlx::query_as::<_, Article>(
        "select * from public.articles order by updated_at desc limit $1",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentRunRequest {
    pub task_type: String,
    #[serde(default = "empty_object")]
    pub input: Value,
}

fn empty_object() -> Value {
    json!({})
}

async fn create_agent_run(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Json(request): Json<CreateAgentRunRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user = ensure_user(&state, &identity).await?;
    if !matches!(user.role.as_str(), "admin" | "editor") {
        return Err(ApiError::Forbidden);
    }

    // Deterministic tasks verify the queue and persistence pipeline. LLM-backed
    // agent execution is intentionally added in Phase 4.
    if !matches!(request.task_type.as_str(), "system.health_check" | "system.echo" | "content.research" | "content.write_article" | "content.social_copy") {
        return Err(ApiError::Validation(
            "Supported tasks: system.health_check, system.echo, content.research, content.write_article, content.social_copy".into(),
        ));
    }

    let run_id = Uuid::new_v4();
    let outbox_id = Uuid::new_v4();
    let job = AgentRunJob {
        outbox_id,
        agent_run_id: run_id,
        task_type: request.task_type.clone(),
        input: request.input.clone(),
    };
    let payload = serde_json::to_value(&job)
        .map_err(|error| ApiError::Internal(error.into()))?;

    let mut tx = state.db.begin().await?;
    let run = sqlx::query_as::<_, AgentRun>(
        r#"insert into public.agent_runs
             (id, initiated_by, task_type, status, input)
           values ($1, $2, $3, 'queued', $4)
           returning id, agent_id, parent_run_id, initiated_by, task_type, status,
                     input, output, error_message, started_at, finished_at, created_at"#,
    )
    .bind(run_id)
    .bind(user.id)
    .bind(&request.task_type)
    .bind(&request.input)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"insert into public.job_outbox (id, topic, aggregate_id, payload)
           values ($1, 'agent_run.execute', $2, $3)"#,
    )
    .bind(outbox_id)
    .bind(run_id)
    .bind(payload)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok((StatusCode::ACCEPTED, Json(run)))
}

async fn get_agent_run(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<AgentRun>, ApiError> {
    ensure_user(&state, &identity).await?;
    let run = sqlx::query_as::<_, AgentRun>(
        r#"select id, agent_id, parent_run_id, initiated_by, task_type, status,
                  input, output, error_message, started_at, finished_at, created_at
           from public.agent_runs
           where id = $1 and initiated_by = $2"#,
    )
    .bind(run_id)
    .bind(identity.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(Json(run))
}

async fn list_agent_runs(
    State(state): State<AppState>,
    Extension(identity): Extension<AuthenticatedUser>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<AgentRun>>, ApiError> {
    ensure_user(&state, &identity).await?;
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let rows = sqlx::query_as::<_, AgentRun>(
        r#"select id, agent_id, parent_run_id, initiated_by, task_type, status,
                  input, output, error_message, started_at, finished_at, created_at
           from public.agent_runs
           where initiated_by = $1
           order by created_at desc
           limit $2"#,
    )
    .bind(identity.id)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

mod publishing;

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/me", get(me))
        .route("/stories", get(list_stories))
        .route("/articles", get(list_articles))
        .route("/agent-runs", get(list_agent_runs).post(create_agent_run))
        .route("/agent-runs/{run_id}", get(get_agent_run))
        .merge(publishing::router())
        .route_layer(middleware::from_fn_with_state(
            state.auth.clone(),
            require_auth,
        ));
    Router::new()
        .route("/health", get(health))
        .nest("/api/v1", protected)
        .with_state(state)
}

#[derive(Debug)]
pub enum ApiError {
    Forbidden,
    NotFound,
    Validation(String),
    Database(sqlx::Error),
    Internal(anyhow::Error),
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self { Self::Database(value) }
}

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self { Self::Internal(value) }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self {
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden", "Insufficient permissions".to_owned()),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found", "Resource not found".to_owned()),
            Self::Validation(message) => (StatusCode::BAD_REQUEST, "validation_error", message),
            Self::Database(error) => {
                tracing::error!(error = %error, "database operation failed");
                (StatusCode::INTERNAL_SERVER_ERROR, "database_error", "Database operation failed".to_owned())
            }
            Self::Internal(error) => {
                tracing::error!(error = %error, "internal operation failed");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "Internal server error".to_owned())
            }
        };
        (status, Json(json!({"error":code,"message":message}))).into_response()
    }
}
