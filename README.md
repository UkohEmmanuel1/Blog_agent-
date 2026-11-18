# ATLAS

ATLAS is a Rust multi-agent content research, writing, review, media, and publishing platform.

## Workspace

- `crates/core` — shared domain models, errors, and serializable queue payloads
- `crates/api` — Axum HTTP API and Supabase JWT authentication
- `crates/worker` — Apalis/Redis worker and transactional-outbox dispatcher
- `crates/agents` — agent runtime boundary (actual LLM agents arrive in Phase 4)
- `migrations` — Supabase/PostgreSQL schema, indexes, and queue outbox

## Requirements

- Stable Rust toolchain (Rust 2021 edition)
- Supabase project with PostgreSQL and Supabase Auth enabled
- Redis 6+ compatible service (local Redis or managed Redis/Valkey)
- Rust SQLx CLI for migrations

## Configure

Copy `.env.example` to `.env` and set `DATABASE_URL`, `SUPABASE_URL`, and `REDIS_URL`. The API fetches the Supabase project's asymmetric signing keys from `/auth/v1/.well-known/jwks.json` at startup. Set `SUPABASE_JWKS_URL` only if you use a custom JWKS endpoint.

Use the Supabase Transaction Pooler (commonly port 6543) for pooled application traffic. Use a direct or Session Pooler connection for migrations where required. Never commit `.env` or credentials.

## Database migrations

Install SQLx CLI:

```sh
cargo install sqlx-cli --no-default-features --features rustls,postgres
sqlx migrate run
```

The schema references Supabase-managed `auth.users(id)`; user identities must be created through Supabase Auth.

## Phase 2 — API and authentication

The API exposes:

| Method | Endpoint | Purpose |
|---|---|---|
| GET | `/health` | Liveness |
| GET | `/api/v1/me` | Provision/return the authenticated application profile |
| GET | `/api/v1/stories?limit=25` | List recent stories |
| GET | `/api/v1/articles?limit=25` | List recent articles |
| POST | `/api/v1/agent-runs` | Create a run and transactionally enqueue its outbox record |
| GET | `/api/v1/agent-runs?limit=25` | List the caller's runs |
| GET | `/api/v1/agent-runs/{id}` | Read one of the caller's runs |

All `/api/v1/*` endpoints require `Authorization: Bearer <Supabase access token>`. JWT signatures are checked against the project's JWKS; issuer, audience, expiry, subject, and the Supabase `authenticated` database role are validated. Application permissions come from `public.users.role`, not user-editable JWT metadata.

Only `admin` and `editor` application roles can submit jobs. Assign trusted operators explicitly from the Supabase SQL editor after verifying their identity:

```sql
update public.users set role = 'admin' where email = 'your-verified-email@example.com';
```

New application profiles should be provisioned with the least privilege appropriate to your deployment. Do not expose the Supabase service-role key to clients.

## Phase 3 — Apalis and Redis

The API writes `agent_runs` and `job_outbox` records in the same PostgreSQL transaction. The worker dispatcher claims outbox rows with `FOR UPDATE SKIP LOCKED`, publishes them to Apalis/Redis, and retries failed Redis publication with exponential backoff. Expired publishing leases are reclaimed after two minutes.

The worker currently supports two deterministic pipeline jobs for infrastructure verification:

- `system.health_check` — verifies PostgreSQL connectivity
- `system.echo` — returns the submitted JSON input

Example request (after signing in through Supabase Auth):

```http
POST /api/v1/agent-runs
Authorization: Bearer <access-token>
Content-Type: application/json

{
  "task_type": "system.health_check",
  "input": {}
}
```

The response is `202 Accepted` with an `agent_runs` record. Poll `GET /api/v1/agent-runs/{id}` for its state. Actual LLM-powered agent execution is intentionally deferred to Phase 4; arbitrary task types are rejected rather than reported as successful.

## Run locally

Start PostgreSQL/Supabase and Redis, apply migrations, then run the API and worker in separate terminals:

```sh
cargo run -p atlas-api
cargo run -p atlas-worker
```

## SQLx offline mode

The current code uses runtime SQL queries, so no `.sqlx` query metadata is needed yet. When compile-time `query!` macros are introduced, generate and commit metadata against a development database:

```sh
cargo sqlx prepare --workspace
```

Then build with `SQLX_OFFLINE=true`.

## Roadmap

1. Foundation and schema
2. API, Supabase Auth JWT validation, and application services
3. Apalis/Redis queue and worker orchestration
4. Rig-powered research/writing agents and S3 media storage
5. Approval workflows and social publishing integrations
6. Observability, tests, deployment, and hardening
