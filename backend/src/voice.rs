use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, ChannelType, JoinVoiceRequest, LeaveVoiceRequest, VoiceState};
use crate::shared::{AppError, AppResult};

pub async fn join_voice_channel(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<JoinVoiceRequest>,
) -> AppResult<Json<VoiceState>> {
    let user_id = auth_user.user_id();

    // Verify the channel exists and is a voice channel
    let channel_type = database::get_channel_type(&state.db, request.channel_id).await?;
    if channel_type != ChannelType::Voice {
        return Err(AppError::bad_request("Cannot join a non-voice channel"));
    }

    // Leave all voice channels the user is currently in (including the target channel for re-joins)
    state.remove_user_from_voice(user_id).await;

    let session_id = Uuid::new_v4();
    let now = Utc::now();

    let voice_state = VoiceState {
        user_id,
        username: auth_user.0.username,
        channel_id: request.channel_id,
        session_id,
        is_muted: false,
        is_deafened: false,
        is_screen_sharing: false,
        is_camera_sharing: false,
        joined_at: now,
    };

    state
        .voice_states
        .entry(request.channel_id)
        .or_default()
        .insert(user_id, voice_state.clone());

    state.broadcast_global("voice_user_joined", serde_json::json!(voice_state));

    tracing::info!(
        "User {} joined voice channel {}",
        voice_state.username,
        request.channel_id
    );

    Ok(Json(voice_state))
}

pub async fn leave_voice_channel(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<LeaveVoiceRequest>,
) -> AppResult<()> {
    let user_id = auth_user.user_id();

    // Remove from voice states
    if let Some(channel_users) = state.voice_states.get(&request.channel_id) {
        channel_users.remove(&user_id);
        drop(channel_users);
        state
            .voice_states
            .remove_if(&request.channel_id, |_, users| users.is_empty());
    }

    // Close all SFU transports for this user in this channel
    state
        .sfu_service
        .close_user_connections(request.channel_id, user_id)
        .await;

    state.broadcast_global(
        "voice_user_left",
        serde_json::json!({
            "user_id": user_id,
            "channel_id": request.channel_id,
        }),
    );

    tracing::info!("User {user_id} left voice channel {}", request.channel_id);

    Ok(())
}

pub async fn get_voice_states(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<Uuid>,
    _auth_user: AuthUser,
) -> AppResult<Json<Vec<VoiceState>>> {
    let users_in_voice = state
        .voice_states
        .get(&channel_id)
        .map(|channel_users| channel_users.iter().map(|r| r.value().clone()).collect())
        .unwrap_or_default();

    Ok(Json(users_in_voice))
}

pub async fn get_all_voice_states(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
) -> AppResult<Json<Vec<VoiceState>>> {
    Ok(Json(state.all_voice_states()))
}
