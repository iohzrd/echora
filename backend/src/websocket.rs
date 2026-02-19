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
use crate::database;
use crate::models::AppState;
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

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    match auth::decode_jwt(&query.token) {
        Ok(claims) => ws.on_upgrade(move |socket| websocket(socket, state, claims)),
        Err(_) => (axum::http::StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
    }
}

async fn websocket(socket: WebSocket, state: Arc<AppState>, claims: auth::Claims) {
    let user_id: Uuid = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => {
            error!("Invalid UUID in JWT claims: {}", claims.sub);
            return;
        }
    };
    let username = claims.username;

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
                            _ => {}
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }

            msg = async {
                match &mut broadcast_rx {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Ok(text) = msg
                    && sender.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }

            msg = global_rx.recv() => {
                if let Ok(text) = msg
                    && sender.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
        }
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

    // Auto-subscribe to channel if not already
    if *current_channel != Some(chat_msg.channel_id) {
        *current_channel = Some(chat_msg.channel_id);
        let tx = get_or_create_broadcast(state, chat_msg.channel_id);
        *broadcast_rx = Some(tx.subscribe());
    }

    if validation::validate_message_content(&chat_msg.content).is_err() {
        return;
    }

    let reply_to = if let Some(reply_id) = chat_msg.reply_to_id {
        database::get_reply_preview(&state.db, reply_id)
            .await
            .unwrap_or_default()
    } else {
        None
    };

    let new_message = crate::models::Message::new(
        chat_msg.content.clone(),
        username.to_string(),
        user_id,
        chat_msg.channel_id,
        chat_msg.reply_to_id,
        reply_to,
    );

    if let Err(e) = database::create_message(&state.db, &new_message, user_id).await {
        error!("Failed to save message: {}", e);
        return;
    }

    state.broadcast_channel(
        chat_msg.channel_id,
        "message",
        serde_json::json!(new_message),
    );

    crate::link_preview::spawn_preview_fetch(
        state.clone(),
        new_message.id,
        chat_msg.channel_id,
        new_message.content.clone(),
    );
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

fn get_or_create_broadcast(state: &Arc<AppState>, channel_id: Uuid) -> broadcast::Sender<String> {
    state
        .channel_broadcasts
        .entry(channel_id)
        .or_insert_with(|| broadcast::channel(validation::BROADCAST_CHANNEL_CAPACITY).0)
        .clone()
}
