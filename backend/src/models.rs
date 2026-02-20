use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::fmt;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::permissions::Role;
use crate::sfu::service::SfuService;
use crate::shared::validation::BROADCAST_CHANNEL_CAPACITY;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Channel {
    pub id: Uuid,
    pub name: String,
    pub channel_type: ChannelType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Text,
    Voice,
}

impl fmt::Display for ChannelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => f.write_str("text"),
            Self::Voice => f.write_str("voice"),
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
            timestamp: Utc::now(),
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
    pub session_id: Uuid,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_screen_sharing: bool,
    pub is_camera_sharing: bool,
    pub joined_at: DateTime<Utc>,
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

// --- Admin / Moderation models ---

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Ban {
    pub id: Uuid,
    pub user_id: Uuid,
    pub banned_by: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Mute {
    pub id: Uuid,
    pub user_id: Uuid,
    pub muted_by: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Invite {
    pub id: Uuid,
    pub code: String,
    pub created_by: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<i32>,
    pub uses: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModLogEntry {
    pub id: Uuid,
    pub action: String,
    pub moderator_id: Uuid,
    pub target_user_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl ModLogEntry {
    pub fn new(
        action: &str,
        moderator_id: Uuid,
        target_user_id: Uuid,
        reason: Option<String>,
        details: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            action: action.to_string(),
            moderator_id,
            target_user_id,
            reason,
            details,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BanRequest {
    pub user_id: Uuid,
    pub reason: Option<String>,
    pub duration_hours: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MuteRequest {
    pub user_id: Uuid,
    pub reason: Option<String>,
    pub duration_hours: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct KickRequest {
    pub user_id: Uuid,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RoleChangeRequest {
    pub role: Role,
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub max_uses: Option<i32>,
    pub expires_in_hours: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ServerSettingUpdate {
    pub key: String,
    pub value: String,
}

pub struct AppState {
    pub db: PgPool,
    pub http_client: reqwest::Client,
    pub channel_broadcasts: DashMap<Uuid, broadcast::Sender<String>>,
    pub global_broadcast: broadcast::Sender<String>,
    pub online_users: DashMap<Uuid, UserPresence>,
    pub voice_states: DashMap<Uuid, DashMap<Uuid, VoiceState>>,
    pub sfu_service: Arc<SfuService>,
}

impl AppState {
    pub fn new(db: PgPool, sfu_service: SfuService, http_client: reqwest::Client) -> Self {
        let (global_tx, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
        Self {
            db,
            http_client,
            channel_broadcasts: DashMap::new(),
            global_broadcast: global_tx,
            online_users: DashMap::new(),
            voice_states: DashMap::new(),
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

    pub fn all_voice_states(&self) -> Vec<VoiceState> {
        self.voice_states
            .iter()
            .flat_map(|entry| {
                entry
                    .value()
                    .iter()
                    .map(|r| r.value().clone())
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}
