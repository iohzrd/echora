use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::sfu::service::SfuService;
use crate::shared::validation::BROADCAST_CHANNEL_CAPACITY;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: Uuid,
    pub name: String,
    pub channel_type: ChannelType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Text,
    Voice,
}

impl ChannelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Voice => "voice",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "voice" => Self::Voice,
            _ => Self::Text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub content: String,
    pub author: String,
    pub author_id: Uuid,
    pub channel_id: Uuid,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<ReplyPreview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Vec<Reaction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_previews: Option<Vec<LinkPreview>>,
}

impl Message {
    pub fn new(
        content: String,
        author: String,
        author_id: Uuid,
        channel_id: Uuid,
        reply_to_id: Option<Uuid>,
        reply_to: Option<ReplyPreview>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            content,
            author,
            author_id,
            channel_id,
            timestamp: chrono::Utc::now(),
            edited_at: None,
            reply_to_id,
            reply_to,
            reactions: None,
            link_previews: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyPreview {
    pub id: Uuid,
    pub author: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub count: i64,
    pub reacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkPreview {
    pub id: Uuid,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    pub reply_to_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct EditMessageRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceState {
    pub user_id: Uuid,
    pub username: String,
    pub channel_id: Uuid,
    pub session_id: String,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_screen_sharing: bool,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSession {
    pub session_id: String,
    pub user_id: Uuid,
    pub channel_id: Uuid,
    pub peer_connection_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct JoinVoiceRequest {
    pub channel_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct LeaveVoiceRequest {
    pub channel_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPresence {
    pub user_id: Uuid,
    pub username: String,
    pub connected_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    pub channel_type: ChannelType,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelRequest {
    pub name: String,
}

pub struct AppState {
    pub db: PgPool,
    // Per-channel text chat broadcast
    pub channel_broadcasts: Arc<DashMap<Uuid, broadcast::Sender<String>>>,
    // Global broadcast for server-wide events (channel CRUD, presence, voice)
    pub global_broadcast: broadcast::Sender<String>,
    // Online users: user_id -> presence info
    pub online_users: Arc<DashMap<Uuid, UserPresence>>,
    // Voice state: channel_id -> user_id -> voice_state
    pub voice_states: Arc<DashMap<Uuid, DashMap<Uuid, VoiceState>>>,
    // Voice sessions: session_id -> session info
    pub voice_sessions: Arc<DashMap<String, VoiceSession>>,
    // mediasoup SFU service
    pub sfu_service: Arc<SfuService>,
}

impl AppState {
    pub fn new(db: PgPool, sfu_service: SfuService) -> Self {
        let (global_tx, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
        Self {
            db,
            channel_broadcasts: Arc::new(DashMap::new()),
            global_broadcast: global_tx,
            online_users: Arc::new(DashMap::new()),
            voice_states: Arc::new(DashMap::new()),
            voice_sessions: Arc::new(DashMap::new()),
            sfu_service: Arc::new(sfu_service),
        }
    }

    pub fn broadcast_global(&self, event_type: &str, data: serde_json::Value) {
        let msg = serde_json::json!({
            "type": event_type,
            "data": data,
        });
        let _ = self.global_broadcast.send(msg.to_string());
    }

    pub fn broadcast_channel(&self, channel_id: Uuid, event_type: &str, data: serde_json::Value) {
        let msg = serde_json::json!({
            "type": event_type,
            "data": data,
        });
        if let Some(tx) = self.channel_broadcasts.get(&channel_id) {
            let _ = tx.send(msg.to_string());
        }
    }
}
