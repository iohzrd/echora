use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{
    AppState, Channel, CreateChannelRequest, EditMessageRequest, Message, SendMessageRequest,
    UpdateChannelRequest, UserPresence,
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
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user ID"))?;

    let name = validate_channel_name(&payload.name)?;

    let channel = Channel {
        id: Uuid::now_v7(),
        name,
        channel_type: payload.channel_type,
    };

    database::create_channel(&state.db, &channel, user_id).await?;

    let broadcast_msg = serde_json::json!({
        "type": "channel_created",
        "data": channel
    });
    let _ = state.global_broadcast.send(broadcast_msg.to_string());

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

    // Fetch the updated channel to return full data
    let channels = database::get_channels(&state.db).await?;
    let channel = channels
        .into_iter()
        .find(|c| c.id == channel_id)
        .ok_or_else(|| AppError::not_found("Channel not found"))?;

    let broadcast_msg = serde_json::json!({
        "type": "channel_updated",
        "data": channel
    });
    let _ = state.global_broadcast.send(broadcast_msg.to_string());

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

    let broadcast_msg = serde_json::json!({
        "type": "channel_deleted",
        "data": { "id": channel_id }
    });
    let _ = state.global_broadcast.send(broadcast_msg.to_string());

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
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user ID"))?;

    let limit = query.limit.unwrap_or(50).min(100).max(1);
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
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user ID"))?;

    if payload.content.trim().is_empty() || payload.content.len() > 4000 {
        return Err(AppError::bad_request(
            "Message must be between 1 and 4000 characters",
        ));
    }

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

    let new_message = Message {
        id: Uuid::now_v7(),
        content: payload.content,
        author: auth_user.0.username,
        author_id: user_id,
        channel_id,
        timestamp: Utc::now(),
        edited_at: None,
        reply_to_id: payload.reply_to_id,
        reply_to,
        reactions: None,
        link_previews: None,
    };

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
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user ID"))?;

    let message = database::get_message_by_id(&state.db, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found"))?;

    if message.author_id != user_id {
        return Err(AppError::forbidden("You can only edit your own messages"));
    }

    if message.channel_id != channel_id {
        return Err(AppError::not_found("Message not found in this channel"));
    }

    if payload.content.trim().is_empty() || payload.content.len() > 4000 {
        return Err(AppError::bad_request(
            "Message must be between 1 and 4000 characters",
        ));
    }

    database::update_message(&state.db, message_id, &payload.content).await?;

    let updated_message = database::get_message_by_id(&state.db, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found"))?;

    // Broadcast edit to channel subscribers
    let broadcast_msg = serde_json::json!({
        "type": "message_edited",
        "data": updated_message
    });

    if let Some(tx) = state.channel_broadcasts.get(&channel_id) {
        let _ = tx.send(broadcast_msg.to_string());
    }

    Ok(Json(updated_message))
}

pub async fn delete_message(
    auth_user: AuthUser,
    Path((channel_id, message_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<AppState>>,
) -> AppResult<()> {
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user ID"))?;

    let message = database::get_message_by_id(&state.db, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found"))?;

    if message.author_id != user_id {
        return Err(AppError::forbidden("You can only delete your own messages"));
    }

    if message.channel_id != channel_id {
        return Err(AppError::not_found("Message not found in this channel"));
    }

    database::delete_message(&state.db, message_id).await?;

    // Broadcast deletion to channel subscribers
    let broadcast_msg = serde_json::json!({
        "type": "message_deleted",
        "data": { "id": message_id, "channel_id": channel_id }
    });

    if let Some(tx) = state.channel_broadcasts.get(&channel_id) {
        let _ = tx.send(broadcast_msg.to_string());
    }

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
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user ID"))?;

    if emoji.is_empty() || emoji.len() > 32 {
        return Err(AppError::bad_request(
            "Emoji must be between 1 and 32 characters",
        ));
    }

    // Verify message exists in this channel
    let message = database::get_message_by_id(&state.db, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found"))?;

    if message.channel_id != channel_id {
        return Err(AppError::not_found("Message not found in this channel"));
    }

    database::add_reaction(&state.db, message_id, user_id, &emoji).await?;

    let broadcast_msg = serde_json::json!({
        "type": "reaction_added",
        "data": ReactionEvent {
            message_id,
            emoji,
            user_id,
            username: auth_user.0.username,
        }
    });

    if let Some(tx) = state.channel_broadcasts.get(&channel_id) {
        let _ = tx.send(broadcast_msg.to_string());
    }

    Ok(())
}

pub async fn remove_reaction(
    auth_user: AuthUser,
    Path((channel_id, message_id, emoji)): Path<(Uuid, Uuid, String)>,
    State(state): State<Arc<AppState>>,
) -> AppResult<()> {
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user ID"))?;

    // Verify message exists in this channel
    let message = database::get_message_by_id(&state.db, message_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found"))?;

    if message.channel_id != channel_id {
        return Err(AppError::not_found("Message not found in this channel"));
    }

    database::remove_reaction(&state.db, message_id, user_id, &emoji).await?;

    let broadcast_msg = serde_json::json!({
        "type": "reaction_removed",
        "data": ReactionEvent {
            message_id,
            emoji,
            user_id,
            username: auth_user.0.username,
        }
    });

    if let Some(tx) = state.channel_broadcasts.get(&channel_id) {
        let _ = tx.send(broadcast_msg.to_string());
    }

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

    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    // Decode base64url-encoded URL
    let image_url = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&query.url)
        .map_err(|_| AppError::bad_request("Invalid URL encoding"))?;
    let image_url =
        String::from_utf8(image_url).map_err(|_| AppError::bad_request("Invalid URL encoding"))?;

    // Verify HMAC signature
    if !crate::link_preview::verify_image_signature(&image_url, &query.sig, &jwt_secret) {
        return Err(AppError::forbidden("Invalid signature"));
    }

    // Fetch the image
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("EchoraBot/1.0")
        .build()
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
    if bytes.len() > 10 * 1024 * 1024 {
        return Err(AppError::bad_request("Image too large"));
    }

    Ok(Response::builder()
        .header("Content-Type", content_type)
        .header("Cache-Control", "public, max-age=86400")
        .body(Body::from(bytes))
        .unwrap())
}

fn validate_channel_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() || trimmed.len() > 50 {
        return Err(AppError::bad_request(
            "Channel name must be between 1 and 50 characters",
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ' ')
    {
        return Err(AppError::bad_request(
            "Channel name can only contain letters, numbers, hyphens, underscores, and spaces",
        ));
    }
    Ok(trimmed)
}
