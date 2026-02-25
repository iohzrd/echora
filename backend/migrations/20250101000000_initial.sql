-- Initial schema for EchoCell
-- Single-instance self-hosted chat platform

CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE channels (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    channel_type TEXT NOT NULL CHECK (channel_type IN ('text', 'voice'))
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    content TEXT NOT NULL,
    author_id TEXT NOT NULL,
    author_username TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (channel_id) REFERENCES channels (id),
    FOREIGN KEY (author_id) REFERENCES users (id)
);

CREATE INDEX idx_messages_channel_id ON messages(channel_id);
CREATE INDEX idx_messages_created_at ON messages(created_at);
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);
