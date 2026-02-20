-- Convert all TEXT UUID columns to native PostgreSQL UUID type
-- This improves storage (16 bytes vs 36), index performance, and type safety

-- Users table
ALTER TABLE users ALTER COLUMN id TYPE UUID USING id::uuid;

-- Channels table
ALTER TABLE channels ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE channels ALTER COLUMN created_by TYPE UUID USING created_by::uuid;

-- Messages table
ALTER TABLE messages ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE messages ALTER COLUMN author_id TYPE UUID USING author_id::uuid;
ALTER TABLE messages ALTER COLUMN channel_id TYPE UUID USING channel_id::uuid;
ALTER TABLE messages ALTER COLUMN reply_to_id TYPE UUID USING reply_to_id::uuid;

-- Reactions table
ALTER TABLE reactions ALTER COLUMN message_id TYPE UUID USING message_id::uuid;
ALTER TABLE reactions ALTER COLUMN user_id TYPE UUID USING user_id::uuid;

-- Link previews table
ALTER TABLE link_previews ALTER COLUMN id TYPE UUID USING id::uuid;

-- Message link previews junction table
ALTER TABLE message_link_previews ALTER COLUMN message_id TYPE UUID USING message_id::uuid;
ALTER TABLE message_link_previews ALTER COLUMN preview_id TYPE UUID USING preview_id::uuid;

-- Bans table
ALTER TABLE bans ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE bans ALTER COLUMN user_id TYPE UUID USING user_id::uuid;
ALTER TABLE bans ALTER COLUMN banned_by TYPE UUID USING banned_by::uuid;

-- Mutes table
ALTER TABLE mutes ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE mutes ALTER COLUMN user_id TYPE UUID USING user_id::uuid;
ALTER TABLE mutes ALTER COLUMN muted_by TYPE UUID USING muted_by::uuid;

-- Invites table
ALTER TABLE invites ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE invites ALTER COLUMN created_by TYPE UUID USING created_by::uuid;

-- Moderation log table
ALTER TABLE moderation_log ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE moderation_log ALTER COLUMN moderator_id TYPE UUID USING moderator_id::uuid;
ALTER TABLE moderation_log ALTER COLUMN target_user_id TYPE UUID USING target_user_id::uuid;
