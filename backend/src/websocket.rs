use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};
use uuid::Uuid;

use crate::auth;
use crate::models::AppState;
use crate::permissions;
use crate::shared::validation;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: String,
}

#[derive(Debug, Deserialize)]
struct WsEnvelope {
    message_type: String,
    #[serde(flatten)]
    payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    channel_id: Uuid,
    content: String,
    reply_to_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct ChannelTarget {
    channel_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct VoiceStateUpdate {
    channel_id: Uuid,
    is_muted: Option<bool>,
    is_deafened: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct VoiceSpeakingUpdate {
    channel_id: Uuid,
    is_speaking: bool,
}

#[derive(Debug, Deserialize)]
struct ScreenShareUpdate {
    channel_id: Uuid,
    is_screen_sharing: bool,
}

#[derive(Debug, Deserialize)]
struct CameraUpdate {
    channel_id: Uuid,
    is_camera_sharing: bool,
}

pub async fn websocket_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let claims = match auth::decode_jwt(&query.token) {
        Ok(c) => c,
        Err(_) => {
            return (axum::http::StatusCode::UNAUTHORIZED, "Invalid token").into_response();
        }
    };

    let user_id = claims.sub;

    match permissions::check_not_banned(&state.db, user_id).await {
        Ok(()) => ws.on_upgrade(move |socket| websocket(socket, state, user_id, claims.username)),
        Err(crate::shared::AppError::Forbidden(_)) => {
            (axum::http::StatusCode::FORBIDDEN, "You are banned").into_response()
        }
        Err(_) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Server error",
        )
            .into_response(),
    }
}

async fn websocket(socket: WebSocket, state: Arc<AppState>, user_id: Uuid, username: String) {
    info!("WebSocket connected: {} ({})", username, user_id);

    // Track online presence
    let presence = crate::models::UserPresence {
        user_id,
        username: username.clone(),
        connected_at: chrono::Utc::now(),
    };
    state.online_users.insert(user_id, presence);
    state.broadcast_global(
        "user_online",
        serde_json::json!({ "user_id": user_id, "username": &username }),
    );

    let (mut sender, mut receiver) = socket.split();
    let mut global_rx = state.global_broadcast.subscribe();
    let mut current_channel: Option<Uuid> = None;
    let mut broadcast_rx: Option<broadcast::Receiver<String>> = None;

    // Ping interval to keep ALB from closing idle connections (ALB default timeout = 60s)
    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    ping_interval.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let Ok(envelope) = serde_json::from_str::<WsEnvelope>(&text) else {
                            continue;
                        };

                        match envelope.message_type.as_str() {
                            "message" => {
                                handle_chat_message(
                                    &state, envelope.payload, user_id, &username,
                                    &mut current_channel, &mut broadcast_rx,
                                ).await;
                            }
                            "join" => {
                                handle_join(&state, envelope.payload, &mut current_channel, &mut broadcast_rx);
                            }
                            "leave" => {
                                current_channel = None;
                                broadcast_rx = None;
                            }
                            "typing" => {
                                handle_typing(&state, envelope.payload, user_id, &username, current_channel);
                            }
                            "voice_state_update" => {
                                handle_voice_state_update(&state, envelope.payload, user_id);
                            }
                            "voice_speaking" => {
                                handle_voice_speaking(&state, envelope.payload, user_id);
                            }
                            "screen_share_update" => {
                                handle_screen_share_update(&state, envelope.payload, user_id);
                            }
                            "camera_update" => {
                                handle_camera_update(&state, envelope.payload, user_id);
                            }
                            "ping" => {
                                let _ = sender.send(Message::Text("{\"type\":\"pong\"}".into())).await;
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sender.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }

            _ = ping_interval.tick() => {
                if sender.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }

            msg = async {
                match &mut broadcast_rx {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                match msg {
                    Ok(text) => {
                        if sender.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Channel broadcast: client {} lagged by {} messages", user_id, n);
                        let _ = sender.send(Message::Text(
                            r#"{"type":"sync_required","data":{"reason":"lagged"}}"#.into()
                        )).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        broadcast_rx = None;
                    }
                }
            }

            msg = global_rx.recv() => {
                match msg {
                    Ok(ref text) => {
                        // Check if this is a kick/ban targeting us
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                            let event_type = parsed.get("type").and_then(|t| t.as_str());
                            let target_id = parsed
                                .get("data")
                                .and_then(|d| d.get("user_id"))
                                .and_then(|u| u.as_str());

                            if matches!(event_type, Some("user_kicked") | Some("user_banned"))
                                && target_id == Some(&user_id.to_string())
                            {
                                let _ = sender.send(Message::Text(text.clone().into())).await;
                                break;
                            }
                        }
                        if sender.send(Message::Text(text.clone().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Global broadcast: client {} lagged by {} messages", user_id, n);
                        let _ = sender.send(Message::Text(
                            r#"{"type":"sync_required","data":{"reason":"lagged"}}"#.into()
                        )).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    // Clean up voice states on disconnect
    let mut left_channels = Vec::new();
    for mut channel_entry in state.voice_states.iter_mut() {
        let channel_id = *channel_entry.key();
        if channel_entry.value_mut().remove(&user_id).is_some() {
            left_channels.push(channel_id);
        }
    }
    for channel_id in &left_channels {
        if let Some(entry) = state.voice_states.get(channel_id)
            && entry.is_empty()
        {
            drop(entry);
            state.voice_states.remove(channel_id);
        }
        state.voice_sessions.retain(|_, session| {
            !(session.user_id == user_id && session.channel_id == *channel_id)
        });
        state
            .sfu_service
            .close_user_connections(*channel_id, user_id)
            .await;
        state.broadcast_global(
            "voice_user_left",
            serde_json::json!({
                "user_id": user_id,
                "channel_id": channel_id,
            }),
        );
    }

    // Clean up presence on disconnect
    state.online_users.remove(&user_id);
    state.broadcast_global(
        "user_offline",
        serde_json::json!({ "user_id": user_id, "username": &username }),
    );

    info!("WebSocket disconnected: {} ({})", username, user_id);
}

async fn handle_chat_message(
    state: &Arc<AppState>,
    payload: serde_json::Value,
    user_id: Uuid,
    username: &str,
    current_channel: &mut Option<Uuid>,
    broadcast_rx: &mut Option<broadcast::Receiver<String>>,
) {
    let Ok(chat_msg) = serde_json::from_value::<ChatMessage>(payload) else {
        return;
    };

    if permissions::is_muted(&state.db, user_id).await {
        return;
    }

    // Auto-subscribe to channel if not already
    if *current_channel != Some(chat_msg.channel_id) {
        *current_channel = Some(chat_msg.channel_id);
        let tx = get_or_create_broadcast(state, chat_msg.channel_id);
        *broadcast_rx = Some(tx.subscribe());
    }

    match crate::services::message::create_message(
        state,
        &state.db,
        crate::services::message::CreateMessageParams {
            user_id,
            username: username.to_string(),
            channel_id: chat_msg.channel_id,
            content: chat_msg.content,
            reply_to_id: chat_msg.reply_to_id,
            validate_reply_channel: false,
        },
    )
    .await
    {
        Ok(result) => {
            state.broadcast_channel(
                result.channel_id,
                "message",
                serde_json::json!(result.message),
            );
        }
        Err(e) => {
            error!("Failed to create message: {}", e);
        }
    }
}

fn handle_join(
    state: &Arc<AppState>,
    payload: serde_json::Value,
    current_channel: &mut Option<Uuid>,
    broadcast_rx: &mut Option<broadcast::Receiver<String>>,
) {
    let Ok(target) = serde_json::from_value::<ChannelTarget>(payload) else {
        return;
    };
    *current_channel = Some(target.channel_id);
    let tx = get_or_create_broadcast(state, target.channel_id);
    *broadcast_rx = Some(tx.subscribe());
}

fn handle_typing(
    state: &Arc<AppState>,
    payload: serde_json::Value,
    user_id: Uuid,
    username: &str,
    current_channel: Option<Uuid>,
) {
    let Ok(target) = serde_json::from_value::<ChannelTarget>(payload) else {
        return;
    };
    let channel_id = current_channel.unwrap_or(target.channel_id);
    state.broadcast_channel(
        channel_id,
        "typing",
        serde_json::json!({
            "user_id": user_id,
            "username": username,
            "channel_id": channel_id,
        }),
    );
}

fn modify_voice_state(
    state: &Arc<AppState>,
    channel_id: Uuid,
    user_id: Uuid,
    event_type: &str,
    update_fn: impl FnOnce(&mut crate::models::VoiceState),
) {
    let Some(channel_users) = state.voice_states.get(&channel_id) else {
        return;
    };
    let Some(mut voice_state) = channel_users.get_mut(&user_id) else {
        return;
    };

    update_fn(&mut voice_state);
    let updated = voice_state.clone();
    drop(voice_state);
    drop(channel_users);

    state.broadcast_global(event_type, serde_json::json!(updated));
}

fn handle_voice_state_update(state: &Arc<AppState>, payload: serde_json::Value, user_id: Uuid) {
    let Ok(update) = serde_json::from_value::<VoiceStateUpdate>(payload) else {
        return;
    };
    modify_voice_state(
        state,
        update.channel_id,
        user_id,
        "voice_state_updated",
        |vs| {
            if let Some(muted) = update.is_muted {
                vs.is_muted = muted;
            }
            if let Some(deafened) = update.is_deafened {
                vs.is_deafened = deafened;
            }
        },
    );
}

fn handle_voice_speaking(state: &Arc<AppState>, payload: serde_json::Value, user_id: Uuid) {
    let Ok(update) = serde_json::from_value::<VoiceSpeakingUpdate>(payload) else {
        return;
    };
    state.broadcast_global(
        "voice_speaking",
        serde_json::json!({
            "user_id": user_id,
            "channel_id": update.channel_id,
            "is_speaking": update.is_speaking,
        }),
    );
}

fn handle_screen_share_update(state: &Arc<AppState>, payload: serde_json::Value, user_id: Uuid) {
    let Ok(update) = serde_json::from_value::<ScreenShareUpdate>(payload) else {
        return;
    };
    modify_voice_state(
        state,
        update.channel_id,
        user_id,
        "screen_share_updated",
        |vs| {
            vs.is_screen_sharing = update.is_screen_sharing;
        },
    );
}

fn handle_camera_update(state: &Arc<AppState>, payload: serde_json::Value, user_id: Uuid) {
    let Ok(update) = serde_json::from_value::<CameraUpdate>(payload) else {
        return;
    };
    modify_voice_state(state, update.channel_id, user_id, "camera_updated", |vs| {
        vs.is_camera_sharing = update.is_camera_sharing;
    });
}

fn get_or_create_broadcast(state: &Arc<AppState>, channel_id: Uuid) -> broadcast::Sender<String> {
    state
        .channel_broadcasts
        .entry(channel_id)
        .or_insert_with(|| broadcast::channel(validation::BROADCAST_CHANNEL_CAPACITY).0)
        .clone()
}
