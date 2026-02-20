use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::User;
use crate::link_preview::LinkPreviewData;
use crate::models::{
    Ban, Channel, ChannelType, Invite, LinkPreview, Message, ModLogEntry, Mute, Reaction,
    ReplyPreview, UserSummary,
};
use crate::shared::AppError;
use crate::shared::validation::REPLY_PREVIEW_LENGTH;

const CHANNEL_COLUMNS: &str = "SELECT id, name, channel_type FROM channels";
const MESSAGE_COLUMNS: &str = "SELECT id, content, author_username, author_id, channel_id, created_at, edited_at, reply_to_id FROM messages";

type ChannelRow = (Uuid, String, String);

fn channel_from_row((id, name, channel_type): ChannelRow) -> Channel {
    Channel {
        id,
        name,
        channel_type: ChannelType::from_str(&channel_type),
    }
}

type UserRow = (Uuid, String, String, String, String, DateTime<Utc>);

fn user_from_row((id, username, email, password_hash, role, created_at): UserRow) -> User {
    User {
        id,
        username,
        email,
        password_hash,
        role,
        created_at,
    }
}

type MessageRow = (
    Uuid,
    String,
    String,
    Uuid,
    Uuid,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<Uuid>,
);

fn message_from_row(row: MessageRow) -> Message {
    let (id, content, author, author_id, channel_id, timestamp, edited_at, reply_to_id) = row;
    Message {
        id,
        content,
        author,
        author_id,
        channel_id,
        timestamp,
        edited_at,
        reply_to_id,
        reply_to: None,
        reactions: None,
        link_previews: None,
    }
}

type BanMuteRow = (
    Uuid,
    Uuid,
    Uuid,
    Option<String>,
    Option<DateTime<Utc>>,
    DateTime<Utc>,
);

type ModLogRow = (
    Uuid,
    String,
    Uuid,
    Uuid,
    Option<String>,
    Option<String>,
    DateTime<Utc>,
);

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

use crate::shared::truncate_string;

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

    // Seed default server_name if not present
    let has_name: Option<(String,)> =
        sqlx::query_as("SELECT value FROM server_settings WHERE key = 'server_name'")
            .fetch_optional(pool)
            .await?;
    if has_name.is_none() {
        sqlx::query(
            "INSERT INTO server_settings (key, value, updated_at) VALUES ('server_name', 'Echora', NOW()) ON CONFLICT (key) DO NOTHING",
        )
        .execute(pool)
        .await?;
    }

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

pub async fn get_channels(pool: &PgPool) -> Result<Vec<Channel>, AppError> {
    let rows: Vec<ChannelRow> = sqlx::query_as(CHANNEL_COLUMNS).fetch_all(pool).await?;

    Ok(rows.into_iter().map(channel_from_row).collect())
}

pub async fn get_channel_by_id(
    pool: &PgPool,
    channel_id: Uuid,
) -> Result<Option<Channel>, AppError> {
    let row: Option<ChannelRow> = sqlx::query_as(&format!("{CHANNEL_COLUMNS} WHERE id = $1"))
        .bind(channel_id)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(channel_from_row))
}

pub async fn get_messages(
    pool: &PgPool,
    channel_id: Uuid,
    limit: i64,
    before: Option<DateTime<Utc>>,
    requesting_user_id: Uuid,
) -> Result<Vec<Message>, AppError> {
    let rows: Vec<MessageRow> = if let Some(before_ts) = before {
        sqlx::query_as(&format!(
            "{MESSAGE_COLUMNS} WHERE channel_id = $1 AND created_at < $2 ORDER BY created_at DESC LIMIT $3"
        ))
        .bind(channel_id)
        .bind(before_ts)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(&format!(
            "{MESSAGE_COLUMNS} WHERE channel_id = $1 ORDER BY created_at DESC LIMIT $2"
        ))
        .bind(channel_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    let mut messages: Vec<Message> = rows.into_iter().map(message_from_row).collect();

    // Reverse so messages are returned in chronological order (oldest first)
    messages.reverse();

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

    // Batch-fetch reactions
    let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();
    if !message_ids.is_empty() {
        let reactions_map =
            get_reactions_for_messages(pool, &message_ids, requesting_user_id).await?;
        for msg in &mut messages {
            if let Some(reactions) = reactions_map.get(&msg.id)
                && !reactions.is_empty()
            {
                msg.reactions = Some(reactions.clone());
            }
        }
    }

    // Batch-fetch link previews
    if !message_ids.is_empty() {
        let previews_map = get_link_previews_for_messages(pool, &message_ids).await?;
        for msg in &mut messages {
            if let Some(previews) = previews_map.get(&msg.id)
                && !previews.is_empty()
            {
                msg.link_previews = Some(previews.clone());
            }
        }
    }

    Ok(messages)
}

pub async fn get_message_by_id(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<Option<Message>, AppError> {
    let row: Option<MessageRow> = sqlx::query_as(&format!("{MESSAGE_COLUMNS} WHERE id = $1"))
        .bind(message_id)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(message_from_row))
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
    .bind(channel.channel_type.as_str())
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

    // Delete messages (reactions and link_previews cascade via ON DELETE CASCADE)
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

pub async fn create_user(pool: &PgPool, user: &User) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, role, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.email)
    .bind(&user.password_hash)
    .bind(&user.role)
    .bind(user.created_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_user_by_username(pool: &PgPool, username: &str) -> Result<Option<User>, AppError> {
    let row: Option<UserRow> = sqlx::query_as(
        "SELECT id, username, email, password_hash, role, created_at FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(user_from_row))
}

pub async fn get_reply_previews(
    pool: &PgPool,
    reply_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, ReplyPreview>, AppError> {
    use std::collections::HashMap;

    let mut previews = HashMap::new();

    let rows: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, author_username, content FROM messages WHERE id = ANY($1)")
            .bind(reply_ids)
            .fetch_all(pool)
            .await?;

    for (id, author, content) in rows {
        previews.insert(
            id,
            ReplyPreview {
                id,
                author,
                content: truncate_string(&content, REPLY_PREVIEW_LENGTH),
            },
        );
    }

    Ok(previews)
}

pub async fn get_reactions_for_messages(
    pool: &PgPool,
    message_ids: &[Uuid],
    requesting_user_id: Uuid,
) -> Result<std::collections::HashMap<Uuid, Vec<Reaction>>, AppError> {
    use std::collections::HashMap;

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

pub async fn get_reply_preview(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<Option<ReplyPreview>, AppError> {
    let row: Option<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, author_username, content FROM messages WHERE id = $1")
            .bind(message_id)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|(id, author, content)| ReplyPreview {
        id,
        author,
        content: truncate_string(&content, REPLY_PREVIEW_LENGTH),
    }))
}

pub async fn get_user_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, AppError> {
    let row: Option<UserRow> = sqlx::query_as(
        "SELECT id, username, email, password_hash, role, created_at FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(user_from_row))
}

/// Insert or update a link preview (deduped by URL), return its ID
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

/// Link a preview to a message via the junction table
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

/// Batch-fetch link previews for a set of messages
pub async fn get_link_previews_for_messages(
    pool: &PgPool,
    message_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<LinkPreview>>, AppError> {
    use std::collections::HashMap;

    type LinkPreviewRow = (
        Uuid,
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );

    let rows: Vec<LinkPreviewRow> = sqlx::query_as(
        "SELECT mlp.message_id, lp.id, lp.url, lp.title, lp.description, lp.image_url, lp.site_name
         FROM message_link_previews mlp
         JOIN link_previews lp ON lp.id = mlp.preview_id
         WHERE mlp.message_id = ANY($1)",
    )
    .bind(message_ids)
    .fetch_all(pool)
    .await?;

    let mut previews_map: HashMap<Uuid, Vec<LinkPreview>> = HashMap::new();

    for (message_id, id, url, title, description, image_url, site_name) in rows {
        previews_map
            .entry(message_id)
            .or_default()
            .push(LinkPreview {
                id,
                url,
                title,
                description,
                image_url,
                site_name,
            });
    }

    Ok(previews_map)
}

// --- User role management ---

pub async fn get_user_count(pool: &PgPool) -> Result<i64, AppError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

pub async fn get_user_role(pool: &PgPool, user_id: Uuid) -> Result<String, AppError> {
    let row: (String,) = sqlx::query_as("SELECT role FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::not_found("User not found"))?;
    Ok(row.0)
}

pub async fn set_user_role(pool: &PgPool, user_id: Uuid, role: &str) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
        .bind(role)
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "User not found")
}

pub async fn get_all_users(pool: &PgPool) -> Result<Vec<UserSummary>, AppError> {
    let rows: Vec<(Uuid, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, username, email, role, created_at FROM users ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, username, email, role, created_at)| UserSummary {
            id,
            username,
            email,
            role,
            created_at,
        })
        .collect())
}

// --- Ban management ---

pub async fn create_ban(pool: &PgPool, ban: &Ban) -> Result<(), AppError> {
    // Remove any existing ban first (UNIQUE on user_id)
    sqlx::query("DELETE FROM bans WHERE user_id = $1")
        .bind(ban.user_id)
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO bans (id, user_id, banned_by, reason, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
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
    let row: Option<BanMuteRow> = sqlx::query_as(
        "SELECT id, user_id, banned_by, reason, expires_at, created_at FROM bans
         WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, user_id, banned_by, reason, expires_at, created_at)| Ban {
            id,
            user_id,
            banned_by,
            reason,
            expires_at,
            created_at,
        },
    ))
}

pub async fn remove_ban(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM bans WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "No active ban found for this user")
}

pub async fn get_all_bans(pool: &PgPool) -> Result<Vec<Ban>, AppError> {
    let rows: Vec<BanMuteRow> = sqlx::query_as(
        "SELECT id, user_id, banned_by, reason, expires_at, created_at FROM bans
         WHERE expires_at IS NULL OR expires_at > NOW()
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, user_id, banned_by, reason, expires_at, created_at)| Ban {
                id,
                user_id,
                banned_by,
                reason,
                expires_at,
                created_at,
            },
        )
        .collect())
}

pub async fn cleanup_expired_bans(pool: &PgPool) -> Result<u64, AppError> {
    let result =
        sqlx::query("DELETE FROM bans WHERE expires_at IS NOT NULL AND expires_at <= NOW()")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

// --- Mute management ---

pub async fn create_mute(pool: &PgPool, mute: &Mute) -> Result<(), AppError> {
    sqlx::query("DELETE FROM mutes WHERE user_id = $1")
        .bind(mute.user_id)
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO mutes (id, user_id, muted_by, reason, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
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
    let row: Option<BanMuteRow> = sqlx::query_as(
        "SELECT id, user_id, muted_by, reason, expires_at, created_at FROM mutes
         WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, user_id, muted_by, reason, expires_at, created_at)| Mute {
            id,
            user_id,
            muted_by,
            reason,
            expires_at,
            created_at,
        },
    ))
}

pub async fn remove_mute(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM mutes WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    require_rows_affected(result, "No active mute found for this user")
}

pub async fn get_all_mutes(pool: &PgPool) -> Result<Vec<Mute>, AppError> {
    let rows: Vec<BanMuteRow> = sqlx::query_as(
        "SELECT id, user_id, muted_by, reason, expires_at, created_at FROM mutes
         WHERE expires_at IS NULL OR expires_at > NOW()
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, user_id, muted_by, reason, expires_at, created_at)| Mute {
                id,
                user_id,
                muted_by,
                reason,
                expires_at,
                created_at,
            },
        )
        .collect())
}

pub async fn cleanup_expired_mutes(pool: &PgPool) -> Result<u64, AppError> {
    let result =
        sqlx::query("DELETE FROM mutes WHERE expires_at IS NOT NULL AND expires_at <= NOW()")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

// --- Invite management ---

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
    type InviteRow = (
        Uuid,
        String,
        Uuid,
        Option<i32>,
        i32,
        Option<DateTime<Utc>>,
        bool,
        DateTime<Utc>,
    );

    let row: Option<InviteRow> = sqlx::query_as(
        "SELECT id, code, created_by, max_uses, uses, expires_at, revoked, created_at
         FROM invites WHERE code = $1",
    )
    .bind(code)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, code, created_by, max_uses, uses, expires_at, revoked, created_at)| Invite {
            id,
            code,
            created_by,
            max_uses,
            uses,
            expires_at,
            revoked,
            created_at,
        },
    ))
}

pub async fn get_all_invites(pool: &PgPool) -> Result<Vec<Invite>, AppError> {
    type InviteRow = (
        Uuid,
        String,
        Uuid,
        Option<i32>,
        i32,
        Option<DateTime<Utc>>,
        bool,
        DateTime<Utc>,
    );

    let rows: Vec<InviteRow> = sqlx::query_as(
        "SELECT id, code, created_by, max_uses, uses, expires_at, revoked, created_at
         FROM invites ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, code, created_by, max_uses, uses, expires_at, revoked, created_at)| Invite {
                id,
                code,
                created_by,
                max_uses,
                uses,
                expires_at,
                revoked,
                created_at,
            },
        )
        .collect())
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

pub async fn get_all_server_settings(
    pool: &PgPool,
) -> Result<std::collections::HashMap<String, String>, AppError> {
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
    .bind(&entry.action)
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
    let rows: Vec<ModLogRow> = sqlx::query_as(
        "SELECT id, action, moderator_id, target_user_id, reason, details, created_at
         FROM moderation_log ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, action, moderator_id, target_user_id, reason, details, created_at)| ModLogEntry {
                id,
                action,
                moderator_id,
                target_user_id,
                reason,
                details,
                created_at,
            },
        )
        .collect())
}
