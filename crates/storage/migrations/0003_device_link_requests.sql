ALTER TABLE oidc_login_states
    ADD COLUMN return_to TEXT NOT NULL DEFAULT '/account/downloads'
    CHECK (length(return_to) BETWEEN 1 AND 1024);

CREATE TABLE device_link_requests (
    id UUID PRIMARY KEY,
    device_code_hash TEXT NOT NULL UNIQUE CHECK (length(device_code_hash) = 64),
    user_code_hash TEXT NOT NULL UNIQUE CHECK (length(user_code_hash) = 64),
    device_label TEXT NOT NULL CHECK (length(device_label) BETWEEN 1 AND 80),
    platform TEXT NOT NULL CHECK (platform IN ('windows','macos','linux','unknown')),
    owner_user_id UUID REFERENCES app_users(id) ON DELETE CASCADE,
    device_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    approved_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    CHECK (expires_at > created_at),
    CHECK (
      (approved_at IS NULL AND owner_user_id IS NULL AND device_id IS NULL)
      OR (approved_at IS NOT NULL AND owner_user_id IS NOT NULL AND device_id IS NOT NULL)
    ),
    FOREIGN KEY (device_id, owner_user_id)
      REFERENCES user_devices(id, owner_user_id) ON DELETE CASCADE
);
CREATE INDEX device_link_requests_user_code_idx ON device_link_requests(user_code_hash) WHERE approved_at IS NULL;
CREATE INDEX device_link_requests_owner_idx ON device_link_requests(owner_user_id, created_at DESC) WHERE owner_user_id IS NOT NULL;
CREATE INDEX device_link_requests_expiry_idx ON device_link_requests(expires_at) WHERE approved_at IS NULL;
