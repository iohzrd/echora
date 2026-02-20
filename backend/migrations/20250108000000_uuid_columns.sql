-- Convert all TEXT UUID columns to native PostgreSQL UUID type
-- This improves storage (16 bytes vs 36), index performance, and type safety

-- Step 1: Drop all foreign key constraints that reference columns being converted
ALTER TABLE messages DROP CONSTRAINT IF EXISTS messages_channel_id_fkey;
ALTER TABLE messages DROP CONSTRAINT IF EXISTS messages_author_id_fkey;
ALTER TABLE messages DROP CONSTRAINT IF EXISTS messages_reply_to_id_fkey;
ALTER TABLE reactions DROP CONSTRAINT IF EXISTS reactions_message_id_fkey;
ALTER TABLE reactions DROP CONSTRAINT IF EXISTS reactions_user_id_fkey;
ALTER TABLE message_link_previews DROP CONSTRAINT IF EXISTS message_link_previews_message_id_fkey;
ALTER TABLE message_link_previews DROP CONSTRAINT IF EXISTS message_link_previews_preview_id_fkey;
ALTER TABLE bans DROP CONSTRAINT IF EXISTS bans_user_id_fkey;
ALTER TABLE bans DROP CONSTRAINT IF EXISTS bans_banned_by_fkey;
ALTER TABLE mutes DROP CONSTRAINT IF EXISTS mutes_user_id_fkey;
ALTER TABLE mutes DROP CONSTRAINT IF EXISTS mutes_muted_by_fkey;
ALTER TABLE invites DROP CONSTRAINT IF EXISTS invites_created_by_fkey;
ALTER TABLE moderation_log DROP CONSTRAINT IF EXISTS moderation_log_moderator_id_fkey;
ALTER TABLE moderation_log DROP CONSTRAINT IF EXISTS moderation_log_target_user_id_fkey;
ALTER TABLE channels DROP CONSTRAINT IF EXISTS channels_created_by_fkey;

-- Step 2: Convert primary key columns first (referenced by FKs)
ALTER TABLE users ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE channels ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE channels ALTER COLUMN created_by TYPE UUID USING created_by::uuid;
ALTER TABLE messages ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE link_previews ALTER COLUMN id TYPE UUID USING id::uuid;

-- Step 3: Convert foreign key columns
ALTER TABLE messages ALTER COLUMN author_id TYPE UUID USING author_id::uuid;
ALTER TABLE messages ALTER COLUMN channel_id TYPE UUID USING channel_id::uuid;
ALTER TABLE messages ALTER COLUMN reply_to_id TYPE UUID USING reply_to_id::uuid;
ALTER TABLE reactions ALTER COLUMN message_id TYPE UUID USING message_id::uuid;
ALTER TABLE reactions ALTER COLUMN user_id TYPE UUID USING user_id::uuid;
ALTER TABLE message_link_previews ALTER COLUMN message_id TYPE UUID USING message_id::uuid;
ALTER TABLE message_link_previews ALTER COLUMN preview_id TYPE UUID USING preview_id::uuid;
ALTER TABLE bans ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE bans ALTER COLUMN user_id TYPE UUID USING user_id::uuid;
ALTER TABLE bans ALTER COLUMN banned_by TYPE UUID USING banned_by::uuid;
ALTER TABLE mutes ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE mutes ALTER COLUMN user_id TYPE UUID USING user_id::uuid;
ALTER TABLE mutes ALTER COLUMN muted_by TYPE UUID USING muted_by::uuid;
ALTER TABLE invites ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE invites ALTER COLUMN created_by TYPE UUID USING created_by::uuid;
ALTER TABLE moderation_log ALTER COLUMN id TYPE UUID USING id::uuid;
ALTER TABLE moderation_log ALTER COLUMN moderator_id TYPE UUID USING moderator_id::uuid;
ALTER TABLE moderation_log ALTER COLUMN target_user_id TYPE UUID USING target_user_id::uuid;

-- Step 4: Re-add foreign key constraints
ALTER TABLE messages ADD CONSTRAINT messages_channel_id_fkey FOREIGN KEY (channel_id) REFERENCES channels(id);
ALTER TABLE messages ADD CONSTRAINT messages_author_id_fkey FOREIGN KEY (author_id) REFERENCES users(id);
ALTER TABLE messages ADD CONSTRAINT messages_reply_to_id_fkey FOREIGN KEY (reply_to_id) REFERENCES messages(id) ON DELETE SET NULL;
ALTER TABLE reactions ADD CONSTRAINT reactions_message_id_fkey FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE;
ALTER TABLE reactions ADD CONSTRAINT reactions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE message_link_previews ADD CONSTRAINT message_link_previews_message_id_fkey FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE;
ALTER TABLE message_link_previews ADD CONSTRAINT message_link_previews_preview_id_fkey FOREIGN KEY (preview_id) REFERENCES link_previews(id) ON DELETE CASCADE;
ALTER TABLE bans ADD CONSTRAINT bans_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE bans ADD CONSTRAINT bans_banned_by_fkey FOREIGN KEY (banned_by) REFERENCES users(id);
ALTER TABLE mutes ADD CONSTRAINT mutes_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE mutes ADD CONSTRAINT mutes_muted_by_fkey FOREIGN KEY (muted_by) REFERENCES users(id);
ALTER TABLE invites ADD CONSTRAINT invites_created_by_fkey FOREIGN KEY (created_by) REFERENCES users(id);
ALTER TABLE moderation_log ADD CONSTRAINT moderation_log_moderator_id_fkey FOREIGN KEY (moderator_id) REFERENCES users(id);
ALTER TABLE moderation_log ADD CONSTRAINT moderation_log_target_user_id_fkey FOREIGN KEY (target_user_id) REFERENCES users(id);
