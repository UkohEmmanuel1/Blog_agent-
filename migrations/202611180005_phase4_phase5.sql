-- Phase 4/5: explicit operational state for publishing, idempotency, and auditability.
alter table public.social_posts
    add column if not exists approved_at timestamptz,
    add column if not exists approved_by uuid references public.users(id) on delete set null;

create index if not exists idx_agent_runs_status_created
    on public.agent_runs(status, created_at desc);

create index if not exists idx_approvals_resource_status
    on public.approvals(resource_type, resource_id, status);

create index if not exists idx_social_posts_publish_queue
    on public.social_posts(status, scheduled_at)
    where status in ('scheduled', 'pending_approval');

create index if not exists idx_articles_status_updated
    on public.articles(status, updated_at desc);

create unique index if not exists idx_social_accounts_platform_identity
    on public.social_accounts(platform, platform_account_id)
    where platform_account_id is not null;

-- A worker may be reclaimed after a process crash. Keep article creation idempotent
-- for one agent_run so a retry cannot create duplicate drafts.
create unique index if not exists idx_articles_generated_agent_run
    on public.articles ((metadata->>'agent_run_id'))
    where metadata ? 'agent_run_id';
