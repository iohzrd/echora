CREATE TABLE link_previews (
    id TEXT PRIMARY KEY NOT NULL,
    url TEXT UNIQUE NOT NULL,
    title TEXT,
    description TEXT,
    image_url TEXT,
    site_name TEXT,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE message_link_previews (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    preview_id TEXT NOT NULL REFERENCES link_previews(id) ON DELETE CASCADE,
    PRIMARY KEY (message_id, preview_id)
);

CREATE INDEX idx_message_link_previews_message_id ON message_link_previews(message_id);
