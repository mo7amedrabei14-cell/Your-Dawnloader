-- Short-lived single-use OIDC authorization states; raw state is not stored.
CREATE TABLE oidc_login_states (
    state_hash TEXT PRIMARY KEY CHECK (length(state_hash) = 64),
    nonce TEXT NOT NULL CHECK (length(nonce) BETWEEN 16 AND 256),
    pkce_verifier TEXT NOT NULL CHECK (length(pkce_verifier) BETWEEN 43 AND 128),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '10 minutes'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX oidc_login_states_expiry_idx ON oidc_login_states(expires_at);

-- Sessions persist token hashes rather than browser session secrets.
CREATE TABLE account_sessions (
    id UUID PRIMARY KEY,
    owner_user_id UUID NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
    session_token_hash TEXT NOT NULL UNIQUE CHECK (length(session_token_hash) = 64),
    csrf_token_hash TEXT NOT NULL CHECK (length(csrf_token_hash) = 64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    last_seen_at TIMESTAMPTZ,
    user_agent_hash TEXT,
    CHECK (expires_at > created_at)
);
CREATE INDEX account_sessions_owner_idx ON account_sessions(owner_user_id, created_at DESC);
CREATE INDEX account_sessions_expiry_idx ON account_sessions(expires_at) WHERE revoked_at IS NULL;

ALTER TABLE user_profiles
    ADD COLUMN email TEXT CHECK (email IS NULL OR length(email) <= 320),
    ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT FALSE;
