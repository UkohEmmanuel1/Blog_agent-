-- Security hardening: new Supabase Auth users must start with least privilege.
-- Existing role assignments are preserved; operators can explicitly promote verified users.
alter table public.users alter column role set default 'viewer';
