-- Add composite index for the common message query pattern:
-- WHERE channel_id = $1 AND created_at < $2 ORDER BY created_at DESC
-- This replaces the two separate single-column indexes for this query path.
CREATE INDEX idx_messages_channel_created ON messages(channel_id, created_at DESC);

-- The individual idx_messages_channel_id index is now redundant (the composite
-- index covers channel_id-only lookups), but we keep idx_messages_created_at
-- for any queries that filter on created_at alone.
DROP INDEX idx_messages_channel_id;
