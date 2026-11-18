-- Least-privilege default: promote trusted operators explicitly.
alter table public.users alter column role set default 'viewer';

-- Invalid payloads are terminal failures, not successfully published messages.
alter table public.job_outbox
    drop constraint if exists job_outbox_status_check;

alter table public.job_outbox
    add constraint job_outbox_status_check
    check (status in ('pending', 'publishing', 'published', 'failed'));
