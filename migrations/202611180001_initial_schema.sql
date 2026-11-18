-- ATLAS initial schema. Supabase Auth owns auth.users; this app owns public.users.
create extension if not exists pgcrypto;

create table if not exists public.users (
    id uuid primary key references auth.users(id) on delete cascade,
    email text unique,
    display_name text,
    role text not null default 'viewer'
        check (role in ('admin', 'editor', 'reviewer', 'viewer')),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists public.news_sources (
    id uuid primary key default gen_random_uuid(),
    name text not null,
    source_type text not null default 'website'
        check (source_type in ('website', 'rss', 'api', 'manual', 'social')),
    url text,
    feed_url text,
    is_active boolean not null default true,
    config jsonb not null default '{}'::jsonb,
    created_by uuid references public.users(id) on delete set null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists public.stories (
    id uuid primary key default gen_random_uuid(),
    source_id uuid references public.news_sources(id) on delete set null,
    external_id text,
    canonical_url text,
    title text not null,
    summary text,
    body text,
    author_name text,
    published_at timestamptz,
    discovered_at timestamptz not null default now(),
    language text not null default 'en',
    status text not null default 'new'
        check (status in ('new', 'analysing', 'selected', 'rejected', 'archived')),
    raw_payload jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (source_id, external_id)
);

create table if not exists public.story_claims (
    id uuid primary key default gen_random_uuid(),
    story_id uuid not null references public.stories(id) on delete cascade,
    claim_text text not null,
    claim_type text not null default 'factual',
    confidence numeric(5,4) check (confidence between 0 and 1),
    verification_status text not null default 'unverified'
        check (verification_status in ('unverified', 'supported', 'disputed', 'false', 'needs_context')),
    evidence jsonb not null default '[]'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists public.articles (
    id uuid primary key default gen_random_uuid(),
    story_id uuid references public.stories(id) on delete set null,
    title text not null,
    slug text not null unique,
    dek text,
    body_markdown text not null default '',
    body_html text,
    status text not null default 'draft'
        check (status in ('draft', 'in_review', 'approved', 'scheduled', 'published', 'rejected', 'archived')),
    author_id uuid references public.users(id) on delete set null,
    editor_id uuid references public.users(id) on delete set null,
    scheduled_at timestamptz,
    published_at timestamptz,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists public.article_revisions (
    id uuid primary key default gen_random_uuid(),
    article_id uuid not null references public.articles(id) on delete cascade,
    revision_number integer not null check (revision_number > 0),
    title text not null,
    body_markdown text not null,
    change_summary text,
    created_by uuid references public.users(id) on delete set null,
    created_at timestamptz not null default now(),
    unique (article_id, revision_number)
);

create table if not exists public.media_assets (
    id uuid primary key default gen_random_uuid(),
    article_id uuid references public.articles(id) on delete set null,
    storage_bucket text not null,
    storage_path text not null,
    public_url text,
    media_type text not null check (media_type in ('image', 'video', 'audio', 'document', 'other')),
    mime_type text,
    file_size_bytes bigint check (file_size_bytes is null or file_size_bytes >= 0),
    alt_text text,
    metadata jsonb not null default '{}'::jsonb,
    uploaded_by uuid references public.users(id) on delete set null,
    created_at timestamptz not null default now(),
    unique (storage_bucket, storage_path)
);

create table if not exists public.social_accounts (
    id uuid primary key default gen_random_uuid(),
    platform text not null,
    account_name text not null,
    platform_account_id text,
    access_token_encrypted text,
    refresh_token_encrypted text,
    token_expires_at timestamptz,
    is_active boolean not null default true,
    config jsonb not null default '{}'::jsonb,
    connected_by uuid references public.users(id) on delete set null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (platform, platform_account_id)
);

create table if not exists public.social_posts (
    id uuid primary key default gen_random_uuid(),
    article_id uuid references public.articles(id) on delete set null,
    social_account_id uuid not null references public.social_accounts(id) on delete cascade,
    platform_post_id text,
    content text not null,
    status text not null default 'draft'
        check (status in ('draft', 'pending_approval', 'scheduled', 'publishing', 'published', 'failed', 'cancelled')),
    scheduled_at timestamptz,
    published_at timestamptz,
    platform_url text,
    error_message text,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists public.approvals (
    id uuid primary key default gen_random_uuid(),
    resource_type text not null check (resource_type in ('article', 'social_post', 'media_asset')),
    resource_id uuid not null,
    requested_by uuid references public.users(id) on delete set null,
    reviewer_id uuid references public.users(id) on delete set null,
    status text not null default 'pending'
        check (status in ('pending', 'approved', 'rejected', 'changes_requested', 'cancelled')),
    review_notes text,
    reviewed_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists public.agents (
    id uuid primary key default gen_random_uuid(),
    name text not null unique,
    agent_type text not null,
    description text,
    model_provider text,
    model_name text,
    system_prompt text,
    configuration jsonb not null default '{}'::jsonb,
    is_enabled boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists public.agent_runs (
    id uuid primary key default gen_random_uuid(),
    agent_id uuid references public.agents(id) on delete set null,
    parent_run_id uuid references public.agent_runs(id) on delete set null,
    initiated_by uuid references public.users(id) on delete set null,
    task_type text not null,
    status text not null default 'queued'
        check (status in ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
    input jsonb not null default '{}'::jsonb,
    output jsonb,
    error_message text,
    started_at timestamptz,
    finished_at timestamptz,
    created_at timestamptz not null default now()
);

create table if not exists public.audit_logs (
    id uuid primary key default gen_random_uuid(),
    actor_id uuid references public.users(id) on delete set null,
    action text not null,
    resource_type text not null,
    resource_id uuid,
    before_state jsonb,
    after_state jsonb,
    ip_address inet,
    user_agent text,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);
