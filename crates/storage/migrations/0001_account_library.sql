-- The identity mapping is private to the API identity adapter; issuer/subject come from verified OIDC.
CREATE TABLE app_users (
    id UUID PRIMARY KEY,
    identity_issuer TEXT NOT NULL,
    identity_subject TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (identity_issuer, identity_subject)
);

CREATE TABLE user_profiles (
    owner_user_id UUID PRIMARY KEY REFERENCES app_users(id) ON DELETE CASCADE,
    display_name TEXT CHECK (display_name IS NULL OR length(display_name) <= 80),
    locale TEXT NOT NULL DEFAULT 'en' CHECK (locale IN ('en', 'ar')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE account_preferences (
    owner_user_id UUID PRIMARY KEY REFERENCES app_users(id) ON DELETE CASCADE,
    sync_download_history BOOLEAN NOT NULL DEFAULT FALSE,
    theme TEXT NOT NULL DEFAULT 'system' CHECK (theme IN ('system', 'light', 'dark')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE user_devices (
    id UUID PRIMARY KEY,
    owner_user_id UUID NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
    device_label TEXT CHECK (device_label IS NULL OR length(device_label) <= 80),
    platform TEXT NOT NULL CHECK (platform IN ('windows', 'macos', 'linux', 'android', 'ios', 'browser', 'unknown')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    UNIQUE (id, owner_user_id)
);
CREATE INDEX user_devices_owner_idx ON user_devices(owner_user_id, created_at DESC);

CREATE TABLE download_records (
    id UUID PRIMARY KEY,
    owner_user_id UUID NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
    client_record_id UUID NOT NULL,
    source_domain TEXT NOT NULL CHECK (length(source_domain) BETWEEN 1 AND 253),
    provider_item_key TEXT CHECK (provider_item_key IS NULL OR length(provider_item_key) <= 256),
    display_name TEXT CHECK (display_name IS NULL OR length(display_name) <= 300),
    media_kind TEXT NOT NULL CHECK (media_kind IN ('audio', 'video', 'image', 'document', 'unknown')),
    container TEXT CHECK (container IS NULL OR length(container) <= 32),
    quality_label TEXT CHECK (quality_label IS NULL OR length(quality_label) <= 80),
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed', 'cancelled')),
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (owner_user_id, client_record_id),
    UNIQUE (id, owner_user_id)
);
CREATE INDEX download_records_owner_created_idx ON download_records(owner_user_id, created_at DESC);
CREATE INDEX download_records_owner_source_idx ON download_records(owner_user_id, source_domain, created_at DESC);

-- A synced history record does not prove the file exists on the device viewing the web page.
CREATE TABLE device_download_presence (
    owner_user_id UUID NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
    download_record_id UUID NOT NULL,
    device_id UUID NOT NULL,
    is_present BOOLEAN NOT NULL DEFAULT FALSE,
    last_confirmed_at TIMESTAMPTZ,
    PRIMARY KEY (download_record_id, device_id),
    FOREIGN KEY (download_record_id, owner_user_id) REFERENCES download_records(id, owner_user_id) ON DELETE CASCADE,
    FOREIGN KEY (device_id, owner_user_id) REFERENCES user_devices(id, owner_user_id) ON DELETE CASCADE
);
CREATE INDEX device_download_presence_owner_idx ON device_download_presence(owner_user_id, device_id);

-- Defense in depth only. Use a non-superuser runtime role without BYPASSRLS.
-- Context must be set transaction-locally from a verified principal in every request.
-- The identity mapping is queried only through a narrow server-side repository method.
ALTER TABLE user_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_profiles FORCE ROW LEVEL SECURITY;
CREATE POLICY user_profiles_owner_policy ON user_profiles
 USING (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid)
 WITH CHECK (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid);

ALTER TABLE account_preferences ENABLE ROW LEVEL SECURITY;
ALTER TABLE account_preferences FORCE ROW LEVEL SECURITY;
CREATE POLICY account_preferences_owner_policy ON account_preferences
 USING (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid)
 WITH CHECK (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid);

ALTER TABLE user_devices ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_devices FORCE ROW LEVEL SECURITY;
CREATE POLICY user_devices_owner_policy ON user_devices
 USING (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid)
 WITH CHECK (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid);

ALTER TABLE download_records ENABLE ROW LEVEL SECURITY;
ALTER TABLE download_records FORCE ROW LEVEL SECURITY;
CREATE POLICY download_records_owner_policy ON download_records
 USING (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid)
 WITH CHECK (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid);

ALTER TABLE device_download_presence ENABLE ROW LEVEL SECURITY;
ALTER TABLE device_download_presence FORCE ROW LEVEL SECURITY;
CREATE POLICY device_download_presence_owner_policy ON device_download_presence
 USING (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid)
 WITH CHECK (owner_user_id = NULLIF(current_setting('app.user_id', true), '')::uuid);
