use anyhow::{Context, Result};
use apalis::prelude::*;
use apalis_redis::{connect, RedisStorage};
use atlas_core::jobs::AgentRunJob;
use serde_json::{json, Value};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::{env, error::Error, str::FromStr, time::Duration};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

type JobResult = Result<(), Box<dyn Error + Send + Sync>>;

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

    let redis_url = env::var("REDIS_URL").context("REDIS_URL must be set")?;
    let connection = connect(redis_url).await.context("failed to connect to Redis")?;
    let storage = RedisStorage::new(connection);

    // PostgreSQL outbox makes creation of a run and its queue intent atomic.
    let dispatcher_pool = pool.clone();
    let dispatcher_storage = storage.clone();
    tokio::spawn(async move {
        dispatch_outbox(dispatcher_pool, dispatcher_storage).await;
    });

    tracing::info!("ATLAS worker online; consuming agent-run jobs");
    let worker_pool = pool.clone();
    WorkerBuilder::new("atlas-agent-runs")
        .concurrency(4)
        .backend(storage)
        .build_fn(move |job: AgentRunJob| {
            let pool = worker_pool.clone();
            async move { process_agent_run(job, pool).await }
        })
        .run()
        .await
        .context("Apalis worker stopped unexpectedly")?;

    Ok(())
}

async fn dispatch_outbox(pool: PgPool, mut storage: RedisStorage<AgentRunJob>) {
    loop {
        let claimed = sqlx::query_as::<_, (Uuid, Value)>(
            r#"with picked as (
                   select id
                   from public.job_outbox
                   where (status = 'pending' and available_at <= now())
                      or (status = 'publishing' and locked_at < now() - interval '2 minutes')
                   order by created_at
                   for update skip locked
                   limit 10
               )
               update public.job_outbox as outbox
               set status = 'publishing',
                   attempts = outbox.attempts + 1,
                   locked_at = now(),
                   updated_at = now()
               from picked
               where outbox.id = picked.id
               returning outbox.id, outbox.payload"#,
        )
        .fetch_all(&pool)
        .await;

        match claimed {
            Ok(rows) => {
                for (outbox_id, payload) in rows {
                    let job = match serde_json::from_value::<AgentRunJob>(payload) {
                        Ok(job) => job,
                        Err(error) => {
                            tracing::error!(%outbox_id, %error, "invalid outbox payload; moving it out of the dispatch loop");
                            let _ = sqlx::query(
                                "update public.job_outbox set status='published', last_error=$2, locked_at=null, updated_at=now() where id=$1",
                            )
                            .bind(outbox_id)
                            .bind(format!("invalid payload: {error}"))
                            .execute(&pool)
                            .await;
                            continue;
                        }
                    };

                    match storage.push(job).await {
                        Ok(_) => {
                            if let Err(error) = sqlx::query(
                                "update public.job_outbox set status='published', published_at=now(), locked_at=null, last_error=null, updated_at=now() where id=$1",
                            )
                            .bind(outbox_id)
                            .execute(&pool)
                            .await
                            {
                                // An expired lease may redeliver this message. The consumer's
                                // queued -> running transition makes duplicate delivery harmless.
                                tracing::error!(%outbox_id, %error, "could not mark outbox message published");
                            }
                        }
                        Err(error) => {
                            tracing::warn!(%outbox_id, %error, "Redis publish failed; outbox message will retry");
                            let _ = sqlx::query(
                                r#"update public.job_outbox
                                   set status='pending',
                                       available_at=now() + (least(power(2, attempts), 300) * interval '1 second'),
                                       locked_at=null,
                                       last_error=$2,
                                       updated_at=now()
                                   where id=$1"#,
                            )
                            .bind(outbox_id)
                            .bind(error.to_string())
                            .execute(&pool)
                            .await;
                        }
                    }
                }
            }
            Err(error) => tracing::error!(%error, "outbox polling failed"),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn process_agent_run(job: AgentRunJob, pool: PgPool) -> JobResult {
    let claimed = sqlx::query_scalar::<_, Uuid>(
        r#"update public.agent_runs
           set status='running', started_at=coalesce(started_at, now()), error_message=null
           where id=$1 and status='queued'
           returning id"#,
    )
    .bind(job.agent_run_id)
    .fetch_optional(&pool)
    .await?;

    if claimed.is_none() {
        tracing::info!(run_id=%job.agent_run_id, "duplicate or already-processed job ignored");
        return Ok(());
    }

    let output = match job.task_type.as_str() {
        "system.health_check" => {
            sqlx::query("select 1").execute(&pool).await?;
            json!({"ok":true,"database":"reachable"})
        }
        "system.echo" => json!({"echo":job.input}),
        other => {
            let message = format!("Unsupported Phase 3 task type: {other}");
            sqlx::query(
                "update public.agent_runs set status='failed', error_message=$2, finished_at=now() where id=$1",
            )
            .bind(job.agent_run_id)
            .bind(&message)
            .execute(&pool)
            .await?;
            tracing::warn!(run_id=%job.agent_run_id, %message, "job type not implemented");
            return Ok(());
        }
    };

    sqlx::query(
        "update public.agent_runs set status='succeeded', output=$2, finished_at=now(), error_message=null where id=$1",
    )
    .bind(job.agent_run_id)
    .bind(output)
    .execute(&pool)
    .await?;
    tracing::info!(run_id=%job.agent_run_id, "agent-run pipeline task completed");
    Ok(())
}
