CREATE TABLE custom_emojis (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    uploaded_by UUID NOT NULL REFERENCES users(id),
    storage_path TEXT NOT NULL,
    content_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_custom_emojis_name ON custom_emojis(name);
