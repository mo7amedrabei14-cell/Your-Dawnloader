-- Account roles are enforced by the API; new authenticated users start as regular_user.
ALTER TABLE app_users
    ADD COLUMN role TEXT NOT NULL DEFAULT 'regular_user'
        CHECK (role IN ('owner', 'admin', 'vip_user', 'regular_user')),
    ADD COLUMN account_status TEXT NOT NULL DEFAULT 'active'
        CHECK (account_status IN ('active', 'suspended'));

CREATE INDEX app_users_role_status_idx ON app_users(role, account_status, created_at DESC);
