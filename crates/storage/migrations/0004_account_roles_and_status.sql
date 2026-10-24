-- Account roles are enforced by the API; new authenticated users start as regular_user.
ALTER TABLE app_users
    ADD COLUMN role TEXT NOT NULL DEFAULT 'regular_user'
        CHECK (role IN ('owner', 'admin', 'vip_user', 'regular_user')),
    ADD COLUMN account_status TEXT NOT NULL DEFAULT 'active'
        CHECK (account_status IN ('active', 'suspended'));

CREATE INDEX app_users_role_status_idx ON app_users(role, account_status, created_at DESC);

-- Preserve normal row scoping while allowing a verified Owner session to manage account profiles.
DROP POLICY user_profiles_owner_policy ON user_profiles;
CREATE POLICY user_profiles_owner_policy ON user_profiles
    USING (
        owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid
        OR EXISTS (
            SELECT 1 FROM app_users AS actor
            WHERE actor.id = NULLIF(current_setting('app.user_id', true), '')::uuid
              AND actor.role = 'owner'
              AND actor.account_status = 'active'
        )
    )
    WITH CHECK (
        owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid
        OR EXISTS (
            SELECT 1 FROM app_users AS actor
            WHERE actor.id = NULLIF(current_setting('app.user_id', true), '')::uuid
              AND actor.role = 'owner'
              AND actor.account_status = 'active'
        )
    );

-- A verified Owner may revoke another user's device when suspending that account.
-- This is an UPDATE-only policy; it does not grant the Owner broad device-row reads/inserts.
CREATE POLICY user_devices_owner_revoke_policy ON user_devices
    FOR UPDATE
    USING (
        EXISTS (
            SELECT 1 FROM app_users AS actor
            WHERE actor.id = NULLIF(current_setting('app.user_id', true), '')::uuid
              AND actor.role = 'owner'
              AND actor.account_status = 'active'
        )
    )
    WITH CHECK (
        EXISTS (
            SELECT 1 FROM app_users AS actor
            WHERE actor.id = NULLIF(current_setting('app.user_id', true), '')::uuid
              AND actor.role = 'owner'
              AND actor.account_status = 'active'
        )
    );
