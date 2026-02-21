-- Make username uniqueness case-insensitive to prevent spoofing
-- while preserving the user's preferred casing for display.

-- Drop the old case-sensitive unique constraint
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_username_key;

-- Drop the old plain index
DROP INDEX IF EXISTS idx_users_username;

-- Add a case-insensitive unique index
CREATE UNIQUE INDEX idx_users_username_lower ON users (LOWER(username));
