-- Instagram single-image publishing support.
alter table public.social_posts
    add column if not exists media_asset_id uuid
        references public.media_assets(id) on delete set null;

create index if not exists idx_social_posts_media_asset
    on public.social_posts(media_asset_id)
    where media_asset_id is not null;
