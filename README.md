# ATLAS

ATLAS is a Rust multi-agent content research, writing, review, media, and social publishing platform.

## Workspace

- `crates/core` — shared domain models, errors, and serializable queue payloads
- `crates/api` — Axum HTTP API, Supabase JWT authentication, editorial workflows
- `crates/worker` — Apalis/Redis worker, transactional outbox, AI jobs, publishing adapters
- `crates/agents` — Rig LLM runtime and Supabase S3-compatible media storage
- `migrations` — Supabase/PostgreSQL schema, indexes, and queue outbox

## Requirements

- Rust stable (Rust 2021 edition)
- Supabase project with PostgreSQL, Auth, and Storage
- Redis 6+ compatible service (local Redis or managed Redis/Valkey)
- OpenAI and/or Anthropic API key
- X developer app and/or LinkedIn developer app with the relevant publishing permissions
- SQLx CLI

## Configure

Copy `.env.example` to `.env` and set:

- `DATABASE_URL`, `SUPABASE_URL`, `REDIS_URL`
- `OPENAI_API_KEY` and/or `ANTHROPIC_API_KEY`
- `S3_ENDPOINT`, `S3_REGION`, `S3_BUCKET`, `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`
- `SOCIAL_TOKEN_ENCRYPTION_KEY`
- `CORS_ALLOWED_ORIGIN` to your frontend's exact origin

The API fetches Supabase asymmetric signing keys from `/auth/v1/.well-known/jwks.json` and refreshes the key set when it encounters an unknown key ID. Set `SUPABASE_JWKS_URL` only for a custom JWKS endpoint. Legacy projects can set `SUPABASE_JWT_SECRET` for HS256; asymmetric signing is preferred.

Use the Supabase Transaction Pooler (commonly port 6543) for pooled application traffic. Use a direct or Session Pooler connection for migrations where required. Never commit `.env` or credentials.

### Supabase Storage

Create the `atlas-media` bucket in Supabase Storage and enable the S3 protocol under Storage configuration. Generate an S3 access-key/secret-key pair from the Supabase dashboard; do not use the Supabase service-role key as S3 credentials. Copy the exact project S3 endpoint and region from the dashboard. ATLAS stores uploaded objects in the private bucket and returns 15-minute signed download URLs.

### Social token encryption

Generate a stable encryption passphrase and keep it backed up:

```sh
openssl rand -base64 48
```

Set the result as `SOCIAL_TOKEN_ENCRYPTION_KEY` in both API and worker environments. ATLAS uses PostgreSQL `pgcrypto` to encrypt social access tokens at rest. Changing this key without re-encrypting stored credentials will make existing connected accounts unusable.

## Database migrations

Install SQLx CLI:

```sh
cargo install sqlx-cli --no-default-features --features rustls,postgres
sqlx migrate run
```

The schema references Supabase-managed `auth.users(id)`; user identities must be created through Supabase Auth.

## Phase 2 — API and authentication

All `/api/v1/*` endpoints require `Authorization: Bearer <Supabase access token>`. JWT signatures are checked against the project's JWKS; issuer, audience, expiry, subject, and the Supabase `authenticated` role are validated. Application permissions come from `public.users.role`, not user-editable JWT metadata.

New application profiles default to `viewer`. First sign in and call `GET /api/v1/me` to provision the profile, then assign trusted operators explicitly from the Supabase SQL editor after verifying their identity:

```sql
update public.users set role = 'admin' where email = 'your-verified-email@example.com';
```

Only `admin` and `editor` roles can initiate AI jobs and create editorial content. `admin` and `reviewer` roles can make approval decisions. Do not expose the Supabase service-role key to clients.

## Phase 3 — Apalis and Redis

The API writes `agent_runs` and `job_outbox` records in the same PostgreSQL transaction. The worker dispatcher claims outbox rows with `FOR UPDATE SKIP LOCKED`, publishes them to Apalis/Redis, and retries failed Redis publication with exponential backoff. Expired publishing leases are reclaimed after two minutes.

Infrastructure test jobs:

- `system.health_check` — verifies PostgreSQL connectivity
- `system.echo` — returns submitted JSON input

## Phase 4 — Rig agents and media

Rig currently supports OpenAI and Anthropic model providers in the ATLAS agent runtime. Agent API keys are read from environment variables; they are not stored in the database. Each queued job may specify `provider` and `model`, otherwise the defaults are OpenAI `gpt-5.5` and Anthropic `claude-sonnet-4-6`.

Supported content jobs are submitted through `POST /api/v1/agent-runs`:

### Research a story

```http
POST /api/v1/agent-runs
Authorization: Bearer <access-token>
Content-Type: application/json

{
  "task_type": "content.research",
  "input": {
    "story_id": "STORY_UUID",
    "provider": "openai",
    "model": "gpt-5.5"
  }
}
```

The research agent updates the story summary and saves extracted claims as `unverified` in `story_claims`. AI-generated claims are not automatically fact-checked or promoted to verified.

### Draft an article

```json
{
  "task_type": "content.write_article",
  "input": {
    "story_id": "STORY_UUID",
    "provider": "anthropic",
    "model": "claude-sonnet-4-6"
  }
}
```

The writer creates an article with `draft` status and an initial article revision. It does not publish the article or bypass editorial review.

### Generate social copy

```json
{
  "task_type": "content.social_copy",
  "input": {
    "article_text": "The article text to adapt...",
    "platforms": ["x", "linkedin"]
  }
}
```

This returns platform-specific draft text in the agent run output. It does not create a live social post.

### Upload media

`POST /api/v1/media` accepts JSON with `filename`, `content_type`, `base64_data`, optional `article_id`, and optional `alt_text`. Supported MIME types are JPEG, PNG, WebP, MP4, MPEG audio, and PDF. Maximum decoded file size is 10 MB.

`GET /api/v1/media/{asset_id}/url` returns a private, signed download URL valid for 15 minutes. The object remains in Supabase Storage; the database stores its bucket, path, MIME type, and metadata.

## Phase 5 — Editorial approval and social publishing

### Editorial revisions and approvals

| Method | Endpoint | Purpose |
|---|---|---|
| GET | `/api/v1/articles/{id}/revisions` | Retrieve revision history |
| POST | `/api/v1/articles/{id}/revisions` | Save an article revision and return it to draft |
| POST | `/api/v1/articles/{id}/submit-for-review` | Submit an article for review |
| GET | `/api/v1/approvals?status=pending` | List approval queue (reviewer/admin) |
| POST | `/api/v1/approvals/{id}/decision` | Approve, reject, or request changes |
| GET | `/api/v1/social-accounts` | List connected accounts without exposing credentials |
| POST | `/api/v1/social-accounts` | Connect an X or LinkedIn account |
| GET | `/api/v1/social-posts?status=draft` | List social post drafts and statuses |
| POST | `/api/v1/social-posts` | Create a social post draft |
| POST | `/api/v1/social-posts/{id}/submit-for-review` | Submit a post for approval |
| POST | `/api/v1/media` | Upload media to private object storage |

A reviewer or admin cannot approve their own submission. Approval decisions are persisted before publishing is queued. Rejected posts are cancelled; requests for changes return the post to draft. An approved social post becomes scheduled and is published by the worker at its requested `scheduled_at` time (or immediately when no schedule was supplied).

### Connect social accounts

Send an access token to the authenticated backend endpoint over HTTPS; tokens are encrypted at rest using `pgcrypto` and are never returned by account-list endpoints.

Example request:

```json
{
  "platform": "x",
  "account_name": "@your_account",
  "platform_account_id": "optional-platform-id",
  "access_token": "USER_CONTEXT_ACCESS_TOKEN"
}
```

For X, use an OAuth user-context access token with the appropriate posting permissions (for OAuth 2.0 PKCE, commonly `tweet.read`, `tweet.write`, and `users.read`). For LinkedIn, provide an access token with the required member or organization posting permission and set `platform_account_id` to the author URN, such as `urn:li:person:...` or `urn:li:organization:...`.

ATLAS implements text-post publishing adapters for X and LinkedIn. Social posts and approval queues can be listed through the read endpoints above. LinkedIn's API version is configurable through `LINKEDIN_VERSION`. Provider access, approved scopes, app review, rate limits, and platform terms remain controlled by the respective platforms. Token refresh and OAuth consent screens are not implemented in this phase; replace expired tokens through a secure credential-rotation workflow.

### Approval example

```http
POST /api/v1/social-posts/SOCIAL_POST_UUID/submit-for-review
Authorization: Bearer <editor-token>
```

A reviewer/admin then submits:

```http
POST /api/v1/approvals/APPROVAL_UUID/decision
Authorization: Bearer <reviewer-token>
Content-Type: application/json

{
  "decision": "approved",
  "notes": "Reviewed and approved for publication."
}
```

The worker publishes approved posts and records the platform post ID, permalink, publication time, and final status. A post already in the publishing state is not blindly re-submitted, reducing duplicate external posts after worker restarts. Ambiguous external failures require editorial reconciliation before a manual retry.

## Run locally

Start Supabase/PostgreSQL and Redis, apply migrations, then run the API and worker in separate terminals:

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
