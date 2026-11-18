# ATLAS

ATLAS is a Rust-based multi-agent content research, writing, review, media, and publishing platform.

## Phase 1 — Foundation

This repository now has a Rust 2021 workspace, initial PostgreSQL schema, domain model crate, API and worker entry points, and environment configuration. Later phases will add authentication, agent orchestration, queues, storage workflows, and publishing integrations.

## Workspace

- `crates/core` — shared domain models and application errors
- `crates/api` — Axum HTTP API
- `crates/worker` — background worker process
- `crates/agents` — agent implementation boundary
- `migrations` — Supabase/PostgreSQL schema and indexes

## Prerequisites

- Rust toolchain (stable; Rust 2021 edition)
- Supabase project with PostgreSQL access
- Redis for background jobs in a later phase

## Configure

1. Copy `.env.example` to `.env`.
2. Fill in your Supabase project values and database password.
3. For Supabase Transaction Pooler, use port `6543`. Use a direct connection or Session Pooler for migrations if your network or Supabase plan requires it.

Never commit `.env` or service-role / provider secrets.

## Database migrations

Install SQLx CLI:

```sh
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

Run migrations against the configured database:

```sh
sqlx migrate run
```

Review migrations before applying them to a production database. The `users.id` column references Supabase-managed `auth.users(id)`; create/authenticate users through Supabase Auth rather than inserting into `auth.users` yourself.

## Run locally

```sh
cargo run -p atlas-api
cargo run -p atlas-worker
```

The API exposes `GET /health`. Both processes require `DATABASE_URL`.

## SQLx offline mode

The workspace is configured for SQLx's compile-time query support. When `query!` / `query_as!` macros are introduced, prepare metadata against a reachable development database and commit the generated `.sqlx/` directory:

```sh
cargo sqlx prepare --workspace
```

Then CI/builds can set `SQLX_OFFLINE=true`. Phase 1 uses runtime query APIs only, so no query metadata is required yet.

## Phases

1. Foundation and database schema
2. API, Supabase Auth JWT validation, and application services
3. Apalis/Redis queue and worker orchestration
4. Rig-powered research/writing agents and S3 media storage
5. Approval workflows and social publishing integrations
6. Observability, tests, deployment, and hardening
