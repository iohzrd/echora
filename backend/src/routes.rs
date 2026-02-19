use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{
    AppState, Channel, CreateChannelRequest, EditMessageRequest, Message, SendMessageRequest,
    UpdateChannelRequest, UserPresence,
};
use crate::shared::validation::{
    MAX_EMOJI_LENGTH, MAX_IMAGE_PROXY_SIZE, validate_channel_name, validate_message_content,
};
use crate::shared::{AppError, AppResult};
use serde::Serialize;

#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub limit: Option<i64>,
    pub before: Option<DateTime<Utc>>,
}

pub async fn get_channels(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Channel>>> {
    let channels = database::get_channels(&state.db).await?;
    Ok(Json(channels))
}

pub async fn create_channel(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateChannelRequest>,
) -> AppResult<Json<Channel>> {
    let user_id = auth_user.user_id()?;

    let name = validate_channel_name(&payload.name)?;

    let channel = Channel {
        id: Uuid::now_v7(),
        name,
        channel_type: payload.channel_type,
    };

    database::create_channel(&state.db, &channel, user_id).await?;

    state.broadcast_global("channel_created", serde_json::json!(channel));

    Ok(Json(channel))
}

pub async fn update_channel(
    _auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateChannelRequest>,
) -> AppResult<Json<Channel>> {
    let name = validate_channel_name(&payload.name)?;

    database::update_channel(&state.db, channel_id, &name).await?;

    let channel = database::get_channel_by_id(&state.db, channel_id)
        .await?
        .ok_or_else(|| AppError::not_found("Channel not found"))?;

    state.broadcast_global("channel_updated", serde_json::json!(channel));

    Ok(Json(channel))
}

pub async fn delete_channel(
    _auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> AppResult<()> {
    database::delete_channel(&state.db, channel_id).await?;

    // Clean up broadcast channel
    state.channel_broadcasts.remove(&channel_id);

    state.broadcast_global("channel_deleted", serde_json::json!({ "id": channel_id }));

    Ok(())
}

pub async fn get_online_users(
    _auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<UserPresence>>> {
    let users: Vec<UserPresence> = state
        .online_users
        .iter()
        .map(|entry| entry.value().clone())
        .collect();
    Ok(Json(users))
}

pub async fn get_messages(
    auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
    Query(query): Query<MessageQuery>,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Message>>> {
    let user_id = auth_user.user_id()?;

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let messages =
        database::get_messages(&state.db, channel_id, limit, query.before, user_id).await?;
    Ok(Json(messages))
}

pub async fn send_message(
    auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SendMessageRequest>,
) -> AppResult<Json<Message>> {
    let user_id = auth_user.user_id()?;

    validate_message_content(&payload.content)?;

    // Validate reply_to_id if provided
    let mut reply_to = None;
    if let Some(reply_id) = payload.reply_to_id {
        let replied_msg = database::get_message_by_id(&state.db, reply_id)
            .await?
            .ok_or_else(|| AppError::not_found("Replied-to message not found"))?;
        if replied_msg.channel_id != channel_id {
            return Err(AppError::bad_request(
                "Cannot reply to a message in a different channel",
            ));
        }
        reply_to = database::get_reply_preview(&state.db, reply_id).await?;
    }

    let new_message = Message::new(
        payload.content,
        auth_user.0.username,
        user_id,
        channel_id,
        payload.reply_to_id,
        reply_to,
    );

    database::create_message(&state.db, &new_message, user_id).await?;

    // Spawn async link preview fetch
    crate::link_preview::spawn_preview_fetch(
        state,
        new_message.id,
        channel_id,
        new_message.content.clone(),
    );

    Ok(Json(new_message))
}

pub async fn edit_message(
    auth_user: AuthUser,
    Path((channel_id, message_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EditMessageRequest>,
) -> AppResult<Json<Message>> {
    let user_id = auth_user.user_id()?;
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
    auth_user: AuthUser,
    Path((channel_id, message_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<AppState>>,
) -> AppResult<()> {
    let user_id = auth_user.user_id()?;
    verify_message_ownership(&state.db, message_id, channel_id, user_id).await?;

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
    auth_user: AuthUser,
    Path((channel_id, message_id, emoji)): Path<(Uuid, Uuid, String)>,
    State(state): State<Arc<AppState>>,
) -> AppResult<()> {
    let user_id = auth_user.user_id()?;

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
    auth_user: AuthUser,
    Path((channel_id, message_id, emoji)): Path<(Uuid, Uuid, String)>,
    State(state): State<Arc<AppState>>,
) -> AppResult<()> {
    let user_id = auth_user.user_id()?;

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

#[derive(Debug, Deserialize)]
pub struct ImageProxyQuery {
    pub url: String,
    pub sig: String,
}

pub async fn proxy_image(
    Query(query): Query<ImageProxyQuery>,
) -> Result<axum::response::Response, AppError> {
    use axum::body::Body;
    use axum::response::Response;
    use base64::Engine;

    let secret = std::str::from_utf8(crate::auth::jwt_secret())
        .map_err(|_| AppError::internal("JWT_SECRET is not valid UTF-8"))?;

    // Decode base64url-encoded URL
    let image_url = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&query.url)
        .map_err(|_| AppError::bad_request("Invalid URL encoding"))?;
    let image_url =
        String::from_utf8(image_url).map_err(|_| AppError::bad_request("Invalid URL encoding"))?;

    // Verify HMAC signature
    if !crate::link_preview::verify_image_signature(&image_url, &query.sig, secret) {
        return Err(AppError::forbidden("Invalid signature"));
    }

    // Fetch the image
    let client = crate::shared::http::create_http_client(10)
        .map_err(|e| AppError::internal(e.to_string()))?;

    let response = client
        .get(&image_url)
        .send()
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    // Only proxy image content types
    if !content_type.starts_with("image/") {
        return Err(AppError::bad_request("Not an image"));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;

    // Limit size to 10MB
    if bytes.len() > MAX_IMAGE_PROXY_SIZE {
        return Err(AppError::bad_request("Image too large"));
    }

    Response::builder()
        .header("Content-Type", content_type)
        .header("Cache-Control", "public, max-age=86400")
        .body(Body::from(bytes))
        .map_err(|e| AppError::internal(e.to_string()))
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
