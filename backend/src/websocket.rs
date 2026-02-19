use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};
use uuid::Uuid;

use crate::auth;
use crate::database;
use crate::models::AppState;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub message_type: MessageType,
    pub channel_id: Uuid,
    pub content: String,
    pub reply_to_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Message,
    Join,
    Leave,
    Typing,
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    // Validate JWT before upgrading to WebSocket
    match auth::decode_jwt(&query.token) {
        Ok(claims) => ws.on_upgrade(move |socket| websocket(socket, state, claims)),
        Err(_) => Response::builder()
            .status(401)
            .body("Invalid token".into())
            .unwrap(),
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
    let online_msg = serde_json::json!({
        "type": "user_online",
        "data": { "user_id": user_id, "username": &username }
    });
    let _ = state.global_broadcast.send(online_msg.to_string());

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to global broadcast for server-wide events
    let mut global_rx = state.global_broadcast.subscribe();

    // Current channel subscription
    let mut current_channel: Option<Uuid> = None;
    let mut broadcast_rx: Option<broadcast::Receiver<String>> = None;

    loop {
        tokio::select! {
            // Handle incoming messages from the client
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(ws_msg) = serde_json::from_str::<WebSocketMessage>(&text) {
                            match ws_msg.message_type {
                                MessageType::Message => {
                                    // Auto-subscribe to channel if not already
                                    if current_channel != Some(ws_msg.channel_id) {
                                        current_channel = Some(ws_msg.channel_id);
                                        let tx = get_or_create_broadcast(&state, ws_msg.channel_id);
                                        broadcast_rx = Some(tx.subscribe());
                                    }

                                    if ws_msg.content.trim().is_empty() || ws_msg.content.len() > 4000 {
                                        continue;
                                    }

                                    // Fetch reply preview if replying
                                    let reply_to = if let Some(reply_id) = ws_msg.reply_to_id {
                                        database::get_reply_preview(&state.db, reply_id).await.unwrap_or_default()
                                    } else {
                                        None
                                    };

                                    let new_message = crate::models::Message {
                                        id: Uuid::now_v7(),
                                        content: ws_msg.content.clone(),
                                        author: username.clone(),
                                        author_id: user_id,
                                        channel_id: ws_msg.channel_id,
                                        timestamp: chrono::Utc::now(),
                                        edited_at: None,
                                        reply_to_id: ws_msg.reply_to_id,
                                        reply_to,
                                        reactions: None,
                                        link_previews: None,
                                    };

                                    if let Err(e) = database::create_message(&state.db, &new_message, user_id).await {
                                        error!("Failed to save message: {}", e);
                                        continue;
                                    }

                                    let broadcast_msg = serde_json::json!({
                                        "type": "message",
                                        "data": new_message
                                    });

                                    let tx = get_or_create_broadcast(&state, ws_msg.channel_id);
                                    if let Err(e) = tx.send(broadcast_msg.to_string()) {
                                        error!("Failed to broadcast message: {}", e);
                                    }

                                    // Spawn async link preview fetch
                                    crate::link_preview::spawn_preview_fetch(
                                        state.clone(),
                                        new_message.id,
                                        ws_msg.channel_id,
                                        new_message.content.clone(),
                                    );
                                }
                                MessageType::Join => {
                                    current_channel = Some(ws_msg.channel_id);
                                    let tx = get_or_create_broadcast(&state, ws_msg.channel_id);
                                    broadcast_rx = Some(tx.subscribe());
                                }
                                MessageType::Leave => {
                                    current_channel = None;
                                    broadcast_rx = None;
                                }
                                MessageType::Typing => {
                                    let channel_id = current_channel.unwrap_or(ws_msg.channel_id);
                                    let tx = get_or_create_broadcast(&state, channel_id);
                                    let typing_msg = serde_json::json!({
                                        "type": "typing",
                                        "data": {
                                            "user_id": user_id.to_string(),
                                            "username": &username,
                                            "channel_id": channel_id.to_string()
                                        }
                                    });
                                    let _ = tx.send(typing_msg.to_string());
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }

            // Handle per-channel broadcast messages
            msg = async {
                match &mut broadcast_rx {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Ok(text) = msg {
                    if sender.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
            }

            // Handle global broadcast messages (channel CRUD, presence)
            msg = global_rx.recv() => {
                if let Ok(text) = msg {
                    if sender.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    // Clean up presence on disconnect
    state.online_users.remove(&user_id);
    let offline_msg = serde_json::json!({
        "type": "user_offline",
        "data": { "user_id": user_id, "username": &username }
    });
    let _ = state.global_broadcast.send(offline_msg.to_string());

    info!("WebSocket disconnected: {} ({})", username, user_id);
}

fn get_or_create_broadcast(state: &Arc<AppState>, channel_id: Uuid) -> broadcast::Sender<String> {
    state
        .channel_broadcasts
        .entry(channel_id)
        .or_insert_with(|| broadcast::channel(256).0)
        .clone()
}
