-- Transactional outbox: API writes domain state and queue intent in one PostgreSQL transaction.
create table if not exists public.job_outbox (
    id uuid primary key default gen_random_uuid(),
    topic text not null,
    aggregate_id uuid,
    payload jsonb not null,
    status text not null default 'pending'
        check (status in ('pending', 'publishing', 'published')),
    attempts integer not null default 0 check (attempts >= 0),
    available_at timestamptz not null default now(),
    locked_at timestamptz,
    published_at timestamptz,
    last_error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_job_outbox_dispatch
    on public.job_outbox(status, available_at, created_at)
    where status in ('pending', 'publishing');

create index if not exists idx_job_outbox_aggregate
    on public.job_outbox(aggregate_id, created_at desc);
