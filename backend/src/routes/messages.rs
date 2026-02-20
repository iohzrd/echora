use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, EditMessageRequest, Message, SendMessageRequest};
use crate::permissions::{self, Role};
use crate::shared::validation::{MAX_EMOJI_LENGTH, validate_message_content};
use crate::shared::{AppError, AppResult};

#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub limit: Option<i64>,
    pub before: Option<DateTime<Utc>>,
}

pub async fn get_messages(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
    Query(query): Query<MessageQuery>,
) -> AppResult<Json<Vec<Message>>> {
    let user_id = auth_user.user_id();

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let messages =
        database::get_messages(&state.db, channel_id, limit, query.before, user_id).await?;
    Ok(Json(messages))
}

pub async fn send_message(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
    Json(payload): Json<SendMessageRequest>,
) -> AppResult<Json<Message>> {
    let user_id = auth_user.user_id();
    permissions::check_not_muted(&state.db, user_id).await?;

    let result = crate::services::message::create_message(
        &state,
        &state.db,
        crate::services::message::CreateMessageParams {
            user_id,
            username: auth_user.0.username,
            channel_id,
            content: payload.content,
            reply_to_id: payload.reply_to_id,
            validate_reply_channel: true,
        },
    )
    .await?;

    Ok(Json(result.message))
}

pub async fn edit_message(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((channel_id, message_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<EditMessageRequest>,
) -> AppResult<Json<Message>> {
    let user_id = auth_user.user_id();
    verify_message_ownership(&state.db, message_id, channel_id, user_id).await?;

    validate_message_content(&payload.content)?;

    database::update_message(&state.db, message_id, &payload.content).await?;

    let updated_message = database::get_message_by_id(&state.db, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found"))?;

    state.broadcast_channel(
        channel_id,
        "message_edited",
        serde_json::json!(updated_message),
    );

    Ok(Json(updated_message))
}

pub async fn delete_message(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((channel_id, message_id)): Path<(Uuid, Uuid)>,
) -> AppResult<()> {
    let user_id = auth_user.user_id();
    let actor_role_str = database::get_user_role(&state.db, user_id).await?;
    let role: permissions::Role = actor_role_str.parse().unwrap();

    if role >= Role::Moderator {
        verify_message_in_channel(&state.db, message_id, channel_id).await?;
    } else {
        verify_message_ownership(&state.db, message_id, channel_id, user_id).await?;
    }

    database::delete_message(&state.db, message_id).await?;

    state.broadcast_channel(
        channel_id,
        "message_deleted",
        serde_json::json!({ "id": message_id, "channel_id": channel_id }),
    );

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ReactionEvent {
    pub message_id: Uuid,
    pub emoji: String,
    pub user_id: Uuid,
    pub username: String,
}

pub async fn add_reaction(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((channel_id, message_id, emoji)): Path<(Uuid, Uuid, String)>,
) -> AppResult<()> {
    let user_id = auth_user.user_id();

    if emoji.is_empty() || emoji.len() > MAX_EMOJI_LENGTH {
        return Err(AppError::bad_request(
            "Emoji must be between 1 and 32 characters",
        ));
    }

    verify_message_in_channel(&state.db, message_id, channel_id).await?;

    database::add_reaction(&state.db, message_id, user_id, &emoji).await?;

    state.broadcast_channel(
        channel_id,
        "reaction_added",
        serde_json::json!(ReactionEvent {
            message_id,
            emoji,
            user_id,
            username: auth_user.0.username,
        }),
    );

    Ok(())
}

pub async fn remove_reaction(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((channel_id, message_id, emoji)): Path<(Uuid, Uuid, String)>,
) -> AppResult<()> {
    let user_id = auth_user.user_id();

    verify_message_in_channel(&state.db, message_id, channel_id).await?;

    database::remove_reaction(&state.db, message_id, user_id, &emoji).await?;

    state.broadcast_channel(
        channel_id,
        "reaction_removed",
        serde_json::json!(ReactionEvent {
            message_id,
            emoji,
            user_id,
            username: auth_user.0.username,
        }),
    );

    Ok(())
}

async fn verify_message_in_channel(
    db: &PgPool,
    message_id: Uuid,
    channel_id: Uuid,
) -> AppResult<()> {
    let message = database::get_message_by_id(db, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found"))?;

    if message.channel_id != channel_id {
        return Err(AppError::not_found("Message not found in this channel"));
    }

    Ok(())
}

async fn verify_message_ownership(
    db: &PgPool,
    message_id: Uuid,
    channel_id: Uuid,
    user_id: Uuid,
) -> AppResult<()> {
    let message = database::get_message_by_id(db, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found"))?;

    if message.author_id != user_id {
        return Err(AppError::forbidden("You can only modify your own messages"));
    }

    if message.channel_id != channel_id {
        return Err(AppError::not_found("Message not found in this channel"));
    }

    Ok(())
}
