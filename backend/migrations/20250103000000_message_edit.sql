-- Add edited_at timestamp to messages for edit tracking
ALTER TABLE messages ADD COLUMN edited_at TIMESTAMPTZ;
