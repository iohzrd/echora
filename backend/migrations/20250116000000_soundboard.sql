CREATE TABLE soundboard_sounds (
    id UUID PRIMARY KEY,
    name VARCHAR(32) NOT NULL,
    volume DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    file_size INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    content_type TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT soundboard_name_length CHECK (char_length(name) BETWEEN 2 AND 32),
    CONSTRAINT soundboard_volume_range CHECK (volume >= 0.0 AND volume <= 1.0),
    CONSTRAINT soundboard_file_size_limit CHECK (file_size <= 524288),
    CONSTRAINT soundboard_duration_limit CHECK (duration_ms <= 5200)
);

CREATE INDEX idx_soundboard_sounds_created_by ON soundboard_sounds(created_by);
CREATE INDEX idx_soundboard_sounds_name ON soundboard_sounds(name);

CREATE TABLE soundboard_favorites (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sound_id UUID NOT NULL REFERENCES soundboard_sounds(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, sound_id)
);
