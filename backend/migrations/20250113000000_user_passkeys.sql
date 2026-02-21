CREATE TABLE user_passkeys (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_name TEXT NOT NULL DEFAULT 'Passkey',
    credential_id TEXT NOT NULL UNIQUE,
    credential_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX idx_user_passkeys_user_id ON user_passkeys(user_id);
