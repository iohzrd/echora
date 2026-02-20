-- Add role column to users (default: 'member')
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'member'
    CHECK (role IN ('owner', 'admin', 'moderator', 'member'));

-- Server settings key-value table
CREATE TABLE server_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed default settings
INSERT INTO server_settings (key, value) VALUES
    ('registration_mode', 'open');

-- Bans table (one active ban per user)
CREATE TABLE bans (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id),
    banned_by TEXT NOT NULL REFERENCES users(id),
    reason TEXT,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id)
);
CREATE INDEX idx_bans_user_id ON bans(user_id);
CREATE INDEX idx_bans_expires_at ON bans(expires_at);

-- Mutes table (one active mute per user)
CREATE TABLE mutes (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id),
    muted_by TEXT NOT NULL REFERENCES users(id),
    reason TEXT,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id)
);
CREATE INDEX idx_mutes_user_id ON mutes(user_id);
CREATE INDEX idx_mutes_expires_at ON mutes(expires_at);

-- Invite codes table
CREATE TABLE invites (
    id TEXT PRIMARY KEY NOT NULL,
    code TEXT NOT NULL UNIQUE,
    created_by TEXT NOT NULL REFERENCES users(id),
    max_uses INTEGER,
    uses INTEGER NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ,
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_invites_code ON invites(code);

-- Moderation log for audit trail
CREATE TABLE moderation_log (
    id TEXT PRIMARY KEY NOT NULL,
    action TEXT NOT NULL,
    moderator_id TEXT NOT NULL REFERENCES users(id),
    target_user_id TEXT NOT NULL REFERENCES users(id),
    reason TEXT,
    details TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_moderation_log_target ON moderation_log(target_user_id);
CREATE INDEX idx_moderation_log_created_at ON moderation_log(created_at);
