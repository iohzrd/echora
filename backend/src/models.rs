use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::fmt;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;
use webauthn_rs::prelude::*;

use object_store::ObjectStore;

use crate::permissions::Role;
use crate::sfu::service::SfuService;
use crate::shared::validation::{
    BROADCAST_CHANNEL_CAPACITY, MESSAGE_RATE_LIMIT, MESSAGE_RATE_REFILL_PER_SEC,
};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
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
            attachments: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomEmoji {
    pub id: Uuid,
    pub name: String,
    pub uploaded_by: Uuid,
    pub storage_path: String,
    pub content_type: String,
    pub created_at: DateTime<Utc>,
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Attachment {
    pub id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub storage_path: String,
    pub uploader_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: Option<String>,
    pub reply_to_id: Option<Uuid>,
    #[serde(default)]
    pub attachment_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct EditMessageRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceState {
    pub user_id: Uuid,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

pub fn avatar_url_from_path(user_id: Uuid, path: &Option<String>) -> Option<String> {
    path.as_ref()
        .map(|_| format!("/api/users/{}/avatar", user_id))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ModAction {
    Kick,
    Ban,
    Unban,
    Mute,
    Unmute,
    RoleChange,
}

impl fmt::Display for ModAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kick => f.write_str("kick"),
            Self::Ban => f.write_str("ban"),
            Self::Unban => f.write_str("unban"),
            Self::Mute => f.write_str("mute"),
            Self::Unmute => f.write_str("unmute"),
            Self::RoleChange => f.write_str("role_change"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModLogEntry {
    pub id: Uuid,
    pub action: ModAction,
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
        action: ModAction,
        moderator_id: Uuid,
        target_user_id: Uuid,
        reason: Option<String>,
        details: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            action,
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

pub struct RateLimitState {
    pub tokens: f64,
    pub last_refill: std::time::Instant,
}

pub struct AppState {
    pub db: PgPool,
    pub http_client: reqwest::Client,
    pub file_store: Option<Arc<dyn ObjectStore>>,
    pub channel_broadcasts: DashMap<Uuid, broadcast::Sender<String>>,
    pub global_broadcast: broadcast::Sender<String>,
    pub online_users: DashMap<Uuid, UserPresence>,
    pub voice_states: DashMap<Uuid, DashMap<Uuid, VoiceState>>,
    pub sfu_service: Arc<SfuService>,
    pub message_rate_limits: DashMap<Uuid, RateLimitState>,
    pub webauthn: Arc<Webauthn>,
    pub webauthn_reg_state: DashMap<Uuid, (PasskeyRegistration, std::time::Instant)>,
    pub webauthn_auth_state: DashMap<String, (Uuid, PasskeyAuthentication, std::time::Instant)>,
}

impl AppState {
    pub fn new(
        db: PgPool,
        sfu_service: SfuService,
        http_client: reqwest::Client,
        file_store: Option<Arc<dyn ObjectStore>>,
        webauthn: Arc<Webauthn>,
    ) -> Self {
        let (global_tx, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
        Self {
            db,
            http_client,
            file_store,
            channel_broadcasts: DashMap::new(),
            global_broadcast: global_tx,
            online_users: DashMap::new(),
            voice_states: DashMap::new(),
            sfu_service: Arc::new(sfu_service),
            message_rate_limits: DashMap::new(),
            webauthn,
            webauthn_reg_state: DashMap::new(),
            webauthn_auth_state: DashMap::new(),
        }
    }

    /// Returns true if the user is allowed to send a message, false if rate-limited.
    pub fn check_message_rate_limit(&self, user_id: Uuid) -> bool {
        let now = std::time::Instant::now();
        let mut entry = self
            .message_rate_limits
            .entry(user_id)
            .or_insert_with(|| RateLimitState {
                tokens: MESSAGE_RATE_LIMIT,
                last_refill: now,
            });

        let elapsed = now.duration_since(entry.last_refill).as_secs_f64();
        entry.tokens =
            (entry.tokens + elapsed * MESSAGE_RATE_REFILL_PER_SEC).min(MESSAGE_RATE_LIMIT);
        entry.last_refill = now;

        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            true
        } else {
            false
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
        // Collect outer keys first, then iterate one at a time to avoid
        // holding nested DashMap shard locks simultaneously.
        let channel_ids: Vec<Uuid> = self.voice_states.iter().map(|e| *e.key()).collect();
        let mut result = Vec::new();
        for channel_id in channel_ids {
            if let Some(channel_users) = self.voice_states.get(&channel_id) {
                result.extend(channel_users.iter().map(|r| r.value().clone()));
            }
        }
        result
    }

    /// Remove a user from all voice channels, close their SFU connections,
    /// and broadcast departure events.
    pub async fn remove_user_from_voice(&self, user_id: Uuid) {
        let mut left_channels = Vec::new();
        for mut channel_entry in self.voice_states.iter_mut() {
            let channel_id = *channel_entry.key();
            if channel_entry.value_mut().remove(&user_id).is_some() {
                left_channels.push(channel_id);
            }
        }
        for channel_id in &left_channels {
            self.voice_states
                .remove_if(channel_id, |_, users| users.is_empty());
            self.sfu_service
                .close_user_connections(*channel_id, user_id)
                .await;
            self.broadcast_global(
                "voice_user_left",
                serde_json::json!({
                    "user_id": user_id,
                    "channel_id": channel_id,
                }),
            );
        }
    }
}
