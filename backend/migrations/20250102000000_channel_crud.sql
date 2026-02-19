-- Add ownership and timestamp tracking to channels
ALTER TABLE channels ADD COLUMN created_by TEXT REFERENCES users(id);
ALTER TABLE channels ADD COLUMN created_at TIMESTAMPTZ DEFAULT NOW();
