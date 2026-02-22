use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::User;
use crate::link_preview::LinkPreviewData;
use crate::models::{
    Attachment, Ban, Channel, ChannelType, Invite, LinkPreview, Message, ModLogEntry, Mute,
    Reaction, ReplyPreview, UserSummary,
};
use crate::permissions::Role;
use crate::shared::AppError;
use crate::shared::truncate_string;
use crate::shared::validation::REPLY_PREVIEW_LENGTH;

// --- Query row types (only for queries that don't map directly to model structs) ---

#[derive(FromRow)]
struct MessageRow {
    id: Uuid,
    content: String,
    author_username: String,
    author_id: Uuid,
    channel_id: Uuid,
    created_at: DateTime<Utc>,
    edited_at: Option<DateTime<Utc>>,
    reply_to_id: Option<Uuid>,
}

impl From<MessageRow> for Message {
    fn from(row: MessageRow) -> Self {
        Message {
            id: row.id,
            content: row.content,
            author: row.author_username,
            author_id: row.author_id,
            channel_id: row.channel_id,
            timestamp: row.created_at,
            edited_at: row.edited_at,
            reply_to_id: row.reply_to_id,
            reply_to: None,
            reactions: None,
            link_previews: None,
            attachments: None,
        }
    }
}

#[derive(FromRow)]
struct ReplyPreviewRow {
    id: Uuid,
    author_username: String,
    content: String,
}

#[derive(FromRow)]
struct LinkPreviewJoinRow {
    message_id: Uuid,
    id: Uuid,
    url: String,
    title: Option<String>,
    description: Option<String>,
    image_url: Option<String>,
    site_name: Option<String>,
}

fn require_rows_affected(
    result: sqlx::postgres::PgQueryResult,
    msg: &'static str,
) -> Result<(), AppError> {
    if result.rows_affected() == 0 {
        Err(AppError::not_found(msg))
    } else {
        Ok(())
    }
}

// --- Seed data ---

pub async fn seed_data(pool: &PgPool) -> Result<(), AppError> {
    let channel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM channels")
        .fetch_one(pool)
        .await?;

    if channel_count == 0 {
        let channels = [
            (Uuid::now_v7(), "general", "text"),
            (Uuid::now_v7(), "random", "text"),
            (Uuid::now_v7(), "announcements", "text"),
            (Uuid::now_v7(), "General Voice", "voice"),
        ];

        for (id, name, channel_type) in &channels {
            sqlx::query("INSERT INTO channels (id, name, channel_type) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(name)
                .bind(channel_type)
                .execute(pool)
                .await?;
        }

        tracing::info!("Seeded {} default channels", channels.len());
    }

    // Seed default settings if not present
    sqlx::query(
        "INSERT INTO server_settings (key, value, updated_at)
         VALUES ('server_name', 'Echora', NOW())
         ON CONFLICT (key) DO NOTHING",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO server_settings (key, value, updated_at)
         VALUES ('registration_mode', 'open', NOW())
         ON CONFLICT (key) DO NOTHING",
    )
    .execute(pool)
    .await?;

    // Ensure at least one owner exists (promote oldest user if none)
    let owner_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'owner'")
        .fetch_one(pool)
        .await?;

    if owner_count == 0 {
        let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await?;

        if user_count > 0 {
            sqlx::query(
                "UPDATE users SET role = 'owner' WHERE id = (SELECT id FROM users ORDER BY created_at ASC LIMIT 1)",
            )
            .execute(pool)
            .await?;

            tracing::info!("Promoted oldest user to owner role");
        }
    }

    Ok(())
}

// --- Channels ---

pub async fn get_channels(pool: &PgPool) -> Result<Vec<Channel>, AppError> {
    let channels: Vec<Channel> = sqlx::query_as("SELECT id, name, channel_type FROM channels")
        .fetch_all(pool)
        .await?;

    Ok(channels)
}

pub async fn get_channel_by_id(
    pool: &PgPool,
    channel_id: Uuid,
) -> Result<Option<Channel>, AppError> {
    let channel: Option<Channel> =
        sqlx::query_as("SELECT id, name, channel_type FROM channels WHERE id = $1")
            .bind(channel_id)
            .fetch_optional(pool)
            .await?;

    Ok(channel)
}

pub async fn get_channel_type(pool: &PgPool, channel_id: Uuid) -> Result<ChannelType, AppError> {
    let row: (ChannelType,) = sqlx::query_as("SELECT channel_type FROM channels WHERE id = $1")
        .bind(channel_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::not_found("Channel not found"))?;
    Ok(row.0)
}

pub async fn create_channel(
    pool: &PgPool,
    channel: &Channel,
    created_by: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO channels (id, name, channel_type, created_by, created_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(channel.id)
    .bind(&channel.name)
    .bind(&channel.channel_type)
    .bind(created_by)
    .bind(Utc::now())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_channel(pool: &PgPool, channel_id: Uuid, name: &str) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE channels SET name = $1 WHERE id = $2")
        .bind(name)
        .bind(channel_id)
        .execute(pool)
        .await?;

    require_rows_affected(result, "Channel not found")
}

pub async fn delete_channel(pool: &PgPool, channel_id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM messages WHERE channel_id = $1")
        .bind(channel_id)
        .execute(&mut *tx)
        .await?;

    let result = sqlx::query("DELETE FROM channels WHERE id = $1")
        .bind(channel_id)
        .execute(&mut *tx)
        .await?;

    require_rows_affected(result, "Channel not found")?;
    tx.commit().await?;
    Ok(())
}

// --- Messages ---

pub async fn get_messages(
    pool: &PgPool,
    channel_id: Uuid,
    limit: i64,
    before: Option<DateTime<Utc>>,
    requesting_user_id: Uuid,
) -> Result<Vec<Message>, AppError> {
    // Subquery fetches newest N messages DESC, outer query re-sorts ASC
    // to return messages in chronological order without a .reverse() in Rust.
    let rows: Vec<MessageRow> = if let Some(before_ts) = before {
        sqlx::query_as(
            "SELECT * FROM (
                 SELECT id, content, author_username, author_id, channel_id, created_at, edited_at, reply_to_id
                 FROM messages WHERE channel_id = $1 AND created_at < $2 ORDER BY created_at DESC LIMIT $3
             ) sub ORDER BY created_at ASC",
        )
        .bind(channel_id)
        .bind(before_ts)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT * FROM (
                 SELECT id, content, author_username, author_id, channel_id, created_at, edited_at, reply_to_id
                 FROM messages WHERE channel_id = $1 ORDER BY created_at DESC LIMIT $2
             ) sub ORDER BY created_at ASC",
        )
        .bind(channel_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    let mut messages: Vec<Message> = rows.into_iter().map(Message::from).collect();

    // Batch-fetch reply previews
    let reply_ids: Vec<Uuid> = messages.iter().filter_map(|m| m.reply_to_id).collect();
    if !reply_ids.is_empty() {
        let previews = get_reply_previews(pool, &reply_ids).await?;
        for msg in &mut messages {
            if let Some(reply_id) = msg.reply_to_id {
                msg.reply_to = previews.get(&reply_id).cloned();
            }
        }
    }

    // Batch-fetch reactions, link previews, and attachments concurrently
    let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();
    if !message_ids.is_empty() {
        let (reactions_map, previews_map, attachments_map) = tokio::join!(
            get_reactions_for_messages(pool, &message_ids, requesting_user_id),
            get_link_previews_for_messages(pool, &message_ids),
            get_attachments_for_messages(pool, &message_ids),
        );
        let reactions_map = reactions_map?;
        let previews_map = previews_map?;
        let attachments_map = attachments_map?;

        for msg in &mut messages {
            if let Some(reactions) = reactions_map.get(&msg.id)
                && !reactions.is_empty()
            {
                msg.reactions = Some(reactions.clone());
            }
            if let Some(previews) = previews_map.get(&msg.id)
                && !previews.is_empty()
            {
                msg.link_previews = Some(previews.clone());
            }
            if let Some(attachments) = attachments_map.get(&msg.id)
                && !attachments.is_empty()
            {
                msg.attachments = Some(attachments.clone());
            }
        }
    }

    Ok(messages)
}

pub async fn get_message_by_id(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<Option<Message>, AppError> {
    let row: Option<MessageRow> = sqlx::query_as(
        "SELECT id, content, author_username, author_id, channel_id, created_at, edited_at, reply_to_id
         FROM messages WHERE id = $1",
    )
    .bind(message_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(Message::from))
}

pub async fn get_full_message_by_id(
    pool: &PgPool,
    message_id: Uuid,
    requesting_user_id: Uuid,
) -> Result<Option<Message>, AppError> {
    let Some(mut msg) = get_message_by_id(pool, message_id).await? else {
        return Ok(None);
    };

    // Enrich with reply preview, reactions, link previews, and attachments
    if let Some(reply_id) = msg.reply_to_id {
        msg.reply_to = get_reply_preview(pool, reply_id).await?;
    }

    let ids = &[message_id];
    let (reactions_map, previews_map, attachments_map) = tokio::join!(
        get_reactions_for_messages(pool, ids, requesting_user_id),
        get_link_previews_for_messages(pool, ids),
        get_attachments_for_messages(pool, ids),
    );

    if let Some(reactions) = reactions_map?.get(&message_id)
        && !reactions.is_empty()
    {
        msg.reactions = Some(reactions.clone());
    }
    if let Some(previews) = previews_map?.get(&message_id)
        && !previews.is_empty()
    {
        msg.link_previews = Some(previews.clone());
    }
    if let Some(attachments) = attachments_map?.get(&message_id)
        && !attachments.is_empty()
    {
        msg.attachments = Some(attachments.clone());
    }

    Ok(Some(msg))
}

pub async fn create_message(
    pool: &PgPool,
    message: &Message,
    author_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO messages (id, content, author_id, author_username, channel_id, created_at, reply_to_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(message.id)
    .bind(&message.content)
    .bind(author_id)
    .bind(&message.author)
    .bind(message.channel_id)
    .bind(message.timestamp)
    .bind(message.reply_to_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_message(
    pool: &PgPool,
    message_id: Uuid,
    content: &str,
) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE messages SET content = $1, edited_at = $2 WHERE id = $3")
        .bind(content)
        .bind(Utc::now())
        .bind(message_id)
        .execute(pool)
        .await?;

    require_rows_affected(result, "Message not found")
}

pub async fn delete_message(pool: &PgPool, message_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM messages WHERE id = $1")
        .bind(message_id)
        .execute(pool)
        .await?;

    require_rows_affected(result, "Message not found")
}

pub async fn get_reply_previews(
    pool: &PgPool,
    reply_ids: &[Uuid],
) -> Result<HashMap<Uuid, ReplyPreview>, AppError> {
    let rows: Vec<ReplyPreviewRow> =
        sqlx::query_as("SELECT id, author_username, content FROM messages WHERE id = ANY($1)")
            .bind(reply_ids)
            .fetch_all(pool)
            .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.id,
                ReplyPreview {
                    id: row.id,
                    author: row.author_username,
                    content: truncate_string(&row.content, REPLY_PREVIEW_LENGTH),
                },
            )
        })
        .collect())
}

pub async fn get_reply_preview(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<Option<ReplyPreview>, AppError> {
    let row: Option<ReplyPreviewRow> =
        sqlx::query_as("SELECT id, author_username, content FROM messages WHERE id = $1")
            .bind(message_id)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|r| ReplyPreview {
        id: r.id,
        author: r.author_username,
        content: truncate_string(&r.content, REPLY_PREVIEW_LENGTH),
    }))
}

pub async fn get_reactions_for_messages(
    pool: &PgPool,
    message_ids: &[Uuid],
    requesting_user_id: Uuid,
) -> Result<HashMap<Uuid, Vec<Reaction>>, AppError> {
    let rows: Vec<(Uuid, String, i64, bool)> = sqlx::query_as(
        "SELECT r.message_id, r.emoji, COUNT(*) as count,
                BOOL_OR(r.user_id = $2) as reacted
         FROM reactions r
         WHERE r.message_id = ANY($1)
         GROUP BY r.message_id, r.emoji
         ORDER BY MIN(r.created_at)",
    )
    .bind(message_ids)
    .bind(requesting_user_id)
    .fetch_all(pool)
    .await?;

    let mut reactions_map: HashMap<Uuid, Vec<Reaction>> = HashMap::new();
    for (message_id, emoji, count, reacted) in rows {
        reactions_map.entry(message_id).or_default().push(Reaction {
            emoji,
            count,
            reacted,
        });
    }

    Ok(reactions_map)
}

pub async fn add_reaction(
    pool: &PgPool,
    message_id: Uuid,
    user_id: Uuid,
    emoji: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO reactions (message_id, user_id, emoji) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(message_id)
    .bind(user_id)
    .bind(emoji)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn remove_reaction(
    pool: &PgPool,
    message_id: Uuid,
    user_id: Uuid,
    emoji: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM reactions WHERE message_id = $1 AND user_id = $2 AND emoji = $3")
        .bind(message_id)
        .bind(user_id)
        .bind(emoji)
        .execute(pool)
        .await?;

    Ok(())
}

// --- Link previews ---

pub async fn upsert_link_preview(pool: &PgPool, data: &LinkPreviewData) -> Result<Uuid, AppError> {
    let id = Uuid::now_v7();
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO link_previews (id, url, title, description, image_url, site_name)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (url) DO UPDATE SET
           title = EXCLUDED.title,
           description = EXCLUDED.description,
           image_url = EXCLUDED.image_url,
           site_name = EXCLUDED.site_name,
           fetched_at = NOW()
         RETURNING id",
    )
    .bind(id)
    .bind(&data.url)
    .bind(&data.title)
    .bind(&data.description)
    .bind(&data.image_url)
    .bind(&data.site_name)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

pub async fn attach_preview_to_message(
    pool: &PgPool,
    message_id: Uuid,
    preview_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO message_link_previews (message_id, preview_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
    )
    .bind(message_id)
    .bind(preview_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_link_previews_for_messages(
    pool: &PgPool,
    message_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<LinkPreview>>, AppError> {
    let rows: Vec<LinkPreviewJoinRow> = sqlx::query_as(
        "SELECT mlp.message_id, lp.id, lp.url, lp.title, lp.description, lp.image_url, lp.site_name
         FROM message_link_previews mlp
         JOIN link_previews lp ON lp.id = mlp.preview_id
         WHERE mlp.message_id = ANY($1)",
    )
    .bind(message_ids)
    .fetch_all(pool)
    .await?;

    let mut previews_map: HashMap<Uuid, Vec<LinkPreview>> = HashMap::new();
    for row in rows {
        previews_map
            .entry(row.message_id)
            .or_default()
            .push(LinkPreview {
                id: row.id,
                url: row.url,
                title: row.title,
                description: row.description,
                image_url: row.image_url,
                site_name: row.site_name,
            });
    }

    Ok(previews_map)
}

// --- Attachments ---

pub async fn get_attachments_for_messages(
    pool: &PgPool,
    message_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<Attachment>>, AppError> {
    let rows: Vec<Attachment> = sqlx::query_as(
        "SELECT id, filename, content_type, size, storage_path, uploader_id, message_id, created_at
         FROM attachments WHERE message_id = ANY($1)
         ORDER BY created_at ASC",
    )
    .bind(message_ids)
    .fetch_all(pool)
    .await?;

    let mut map: HashMap<Uuid, Vec<Attachment>> = HashMap::new();
    for row in rows {
        if let Some(msg_id) = row.message_id {
            map.entry(msg_id).or_default().push(row);
        }
    }

    Ok(map)
}

pub async fn link_attachments_to_message(
    pool: &PgPool,
    attachment_ids: &[Uuid],
    message_id: Uuid,
    uploader_id: Uuid,
) -> Result<Vec<Attachment>, AppError> {
    let attachments: Vec<Attachment> = sqlx::query_as(
        "UPDATE attachments SET message_id = $1
         WHERE id = ANY($2) AND uploader_id = $3 AND message_id IS NULL
         RETURNING *",
    )
    .bind(message_id)
    .bind(attachment_ids)
    .bind(uploader_id)
    .fetch_all(pool)
    .await?;

    Ok(attachments)
}

// --- Users ---

pub async fn create_user(pool: &PgPool, user: &User) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, role, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.email)
    .bind(&user.password_hash)
    .bind(user.role)
    .bind(user.created_at)
    .execute(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            let msg = db_err.message();
            if msg.contains("username") {
                AppError::conflict("Username already taken")
            } else if msg.contains("email") {
                AppError::conflict("Email already in use")
            } else {
                AppError::conflict("User already exists")
            }
        }
        _ => AppError::from(e),
    })?;

    Ok(())
}

pub async fn get_user_by_id(pool: &PgPool, user_id: Uuid) -> Result<Option<User>, AppError> {
    let user: Option<User> = sqlx::query_as(
        "SELECT id, username, email, password_hash, role, created_at, avatar_path, display_name FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

pub async fn get_user_by_username(pool: &PgPool, username: &str) -> Result<Option<User>, AppError> {
    let user: Option<User> = sqlx::query_as(
        "SELECT id, username, email, password_hash, role, created_at, avatar_path, display_name FROM users WHERE LOWER(username) = LOWER($1)",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

pub async fn get_user_count(pool: &PgPool) -> Result<i64, AppError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

pub async fn get_user_role(pool: &PgPool, user_id: Uuid) -> Result<Role, AppError> {
    let row: (Role,) = sqlx::query_as("SELECT role FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::not_found("User not found"))?;
    Ok(row.0)
}

pub async fn set_user_role(pool: &PgPool, user_id: Uuid, role: Role) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
        .bind(role)
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "User not found")
}

pub async fn get_all_users(pool: &PgPool) -> Result<Vec<UserSummary>, AppError> {
    let rows: Vec<(Uuid, String, String, Role, chrono::DateTime<chrono::Utc>, Option<String>)> = sqlx::query_as(
        "SELECT id, username, email, role, created_at, avatar_path FROM users ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;

    let users = rows
        .into_iter()
        .map(
            |(id, username, email, role, created_at, avatar_path)| UserSummary {
                id,
                username,
                email,
                role,
                created_at,
                avatar_url: crate::models::avatar_url_from_path(id, &avatar_path),
            },
        )
        .collect();

    Ok(users)
}

pub async fn update_user_avatar(
    pool: &PgPool,
    user_id: Uuid,
    avatar_path: Option<&str>,
) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE users SET avatar_path = $1 WHERE id = $2")
        .bind(avatar_path)
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "User not found")
}

pub async fn update_user_display_name(
    pool: &PgPool,
    user_id: Uuid,
    display_name: Option<&str>,
) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE users SET display_name = $1 WHERE id = $2")
        .bind(display_name)
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "User not found")
}

pub async fn update_user_password(
    pool: &PgPool,
    user_id: Uuid,
    password_hash: &str,
) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(password_hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "User not found")
}

// --- Bans (atomic upsert) ---

pub async fn create_ban(pool: &PgPool, ban: &Ban) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO bans (id, user_id, banned_by, reason, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (user_id) DO UPDATE SET
           id = EXCLUDED.id,
           banned_by = EXCLUDED.banned_by,
           reason = EXCLUDED.reason,
           expires_at = EXCLUDED.expires_at,
           created_at = EXCLUDED.created_at",
    )
    .bind(ban.id)
    .bind(ban.user_id)
    .bind(ban.banned_by)
    .bind(&ban.reason)
    .bind(ban.expires_at)
    .bind(ban.created_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_active_ban(pool: &PgPool, user_id: Uuid) -> Result<Option<Ban>, AppError> {
    let ban: Option<Ban> = sqlx::query_as(
        "SELECT id, user_id, banned_by, reason, expires_at, created_at FROM bans
         WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(ban)
}

pub async fn remove_ban(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM bans WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "No active ban found for this user")
}

pub async fn get_all_bans(pool: &PgPool) -> Result<Vec<Ban>, AppError> {
    let bans: Vec<Ban> = sqlx::query_as(
        "SELECT id, user_id, banned_by, reason, expires_at, created_at FROM bans
         WHERE expires_at IS NULL OR expires_at > NOW()
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(bans)
}

pub async fn cleanup_expired_bans(pool: &PgPool) -> Result<u64, AppError> {
    let result =
        sqlx::query("DELETE FROM bans WHERE expires_at IS NOT NULL AND expires_at <= NOW()")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

// --- Mutes (atomic upsert) ---

pub async fn create_mute(pool: &PgPool, mute: &Mute) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO mutes (id, user_id, muted_by, reason, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (user_id) DO UPDATE SET
           id = EXCLUDED.id,
           muted_by = EXCLUDED.muted_by,
           reason = EXCLUDED.reason,
           expires_at = EXCLUDED.expires_at,
           created_at = EXCLUDED.created_at",
    )
    .bind(mute.id)
    .bind(mute.user_id)
    .bind(mute.muted_by)
    .bind(&mute.reason)
    .bind(mute.expires_at)
    .bind(mute.created_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_active_mute(pool: &PgPool, user_id: Uuid) -> Result<Option<Mute>, AppError> {
    let mute: Option<Mute> = sqlx::query_as(
        "SELECT id, user_id, muted_by, reason, expires_at, created_at FROM mutes
         WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(mute)
}

pub async fn remove_mute(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM mutes WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "No active mute found for this user")
}

pub async fn get_all_mutes(pool: &PgPool) -> Result<Vec<Mute>, AppError> {
    let mutes: Vec<Mute> = sqlx::query_as(
        "SELECT id, user_id, muted_by, reason, expires_at, created_at FROM mutes
         WHERE expires_at IS NULL OR expires_at > NOW()
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(mutes)
}

pub async fn cleanup_expired_mutes(pool: &PgPool) -> Result<u64, AppError> {
    let result =
        sqlx::query("DELETE FROM mutes WHERE expires_at IS NOT NULL AND expires_at <= NOW()")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

// --- Invites ---

pub async fn create_invite(pool: &PgPool, invite: &Invite) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO invites (id, code, created_by, max_uses, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(invite.id)
    .bind(&invite.code)
    .bind(invite.created_by)
    .bind(invite.max_uses)
    .bind(invite.expires_at)
    .bind(invite.created_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn use_invite_code(pool: &PgPool, code: &str) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE invites SET uses = uses + 1
         WHERE code = $1
           AND NOT revoked
           AND (max_uses IS NULL OR uses < max_uses)
           AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(code)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::bad_request(
            "Invalid, expired, or fully used invite code",
        ));
    }

    Ok(())
}

pub async fn revoke_invite(pool: &PgPool, invite_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE invites SET revoked = TRUE WHERE id = $1")
        .bind(invite_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "Invite not found")
}

pub async fn get_invite_by_code(pool: &PgPool, code: &str) -> Result<Option<Invite>, AppError> {
    let invite: Option<Invite> = sqlx::query_as(
        "SELECT id, code, created_by, max_uses, uses, expires_at, revoked, created_at
         FROM invites WHERE code = $1",
    )
    .bind(code)
    .fetch_optional(pool)
    .await?;

    Ok(invite)
}

pub async fn get_all_invites(pool: &PgPool) -> Result<Vec<Invite>, AppError> {
    let invites: Vec<Invite> = sqlx::query_as(
        "SELECT id, code, created_by, max_uses, uses, expires_at, revoked, created_at
         FROM invites ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(invites)
}

// --- Server settings ---

pub async fn get_server_setting(pool: &PgPool, key: &str) -> Result<String, AppError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM server_settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await?;

    row.map(|r| r.0)
        .ok_or_else(|| AppError::not_found(format!("Setting '{}' not found", key)))
}

pub async fn set_server_setting(pool: &PgPool, key: &str, value: &str) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO server_settings (key, value, updated_at) VALUES ($1, $2, NOW())
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_all_server_settings(pool: &PgPool) -> Result<HashMap<String, String>, AppError> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM server_settings")
        .fetch_all(pool)
        .await?;

    Ok(rows.into_iter().collect())
}

// --- Moderation log ---

pub async fn create_mod_log_entry(pool: &PgPool, entry: &ModLogEntry) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO moderation_log (id, action, moderator_id, target_user_id, reason, details, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(entry.id)
    .bind(entry.action)
    .bind(entry.moderator_id)
    .bind(entry.target_user_id)
    .bind(&entry.reason)
    .bind(&entry.details)
    .bind(entry.created_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_mod_log(pool: &PgPool, limit: i64) -> Result<Vec<ModLogEntry>, AppError> {
    let entries: Vec<ModLogEntry> = sqlx::query_as(
        "SELECT id, action, moderator_id, target_user_id, reason, details, created_at
         FROM moderation_log ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(entries)
}

// --- Passkeys ---

pub async fn create_user_passkey(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    credential_name: &str,
    credential_id: &str,
    credential: &webauthn_rs::prelude::Passkey,
) -> Result<(), AppError> {
    let credential_json = serde_json::to_value(credential)
        .map_err(|e| AppError::internal(format!("Failed to serialize passkey: {e}")))?;
    sqlx::query(
        "INSERT INTO user_passkeys (id, user_id, credential_name, credential_id, credential_json, created_at)
         VALUES ($1, $2, $3, $4, $5, NOW())",
    )
    .bind(id)
    .bind(user_id)
    .bind(credential_name)
    .bind(credential_id)
    .bind(&credential_json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_user_passkeys(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<
    Vec<(
        Uuid,
        String,
        webauthn_rs::prelude::Passkey,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )>,
    AppError,
> {
    let rows: Vec<(
        Uuid,
        String,
        serde_json::Value,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT id, credential_name, credential_json, created_at, last_used_at
             FROM user_passkeys WHERE user_id = $1 ORDER BY created_at ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|(id, name, json, created, last_used)| {
            let passkey: webauthn_rs::prelude::Passkey = serde_json::from_value(json)
                .map_err(|e| AppError::internal(format!("Failed to deserialize passkey: {e}")))?;
            Ok((id, name, passkey, created, last_used))
        })
        .collect()
}

pub async fn update_user_passkey(
    pool: &PgPool,
    user_id: Uuid,
    credential_id: &str,
    credential: &webauthn_rs::prelude::Passkey,
) -> Result<(), AppError> {
    let credential_json = serde_json::to_value(credential)
        .map_err(|e| AppError::internal(format!("Failed to serialize passkey: {e}")))?;
    sqlx::query(
        "UPDATE user_passkeys SET credential_json = $1, last_used_at = NOW()
         WHERE user_id = $2 AND credential_id = $3",
    )
    .bind(&credential_json)
    .bind(user_id)
    .bind(credential_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_user_passkey(
    pool: &PgPool,
    passkey_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM user_passkeys WHERE id = $1 AND user_id = $2")
        .bind(passkey_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "Passkey not found")
}
