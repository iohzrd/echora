CREATE TABLE attachments (
    id UUID PRIMARY KEY,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size BIGINT NOT NULL,
    storage_path TEXT NOT NULL,
    uploader_id UUID NOT NULL REFERENCES users(id),
    message_id UUID REFERENCES messages(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_attachments_message_id ON attachments(message_id) WHERE message_id IS NOT NULL;
CREATE INDEX idx_attachments_uploader_id ON attachments(uploader_id);
