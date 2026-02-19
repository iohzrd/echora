use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::{
    AppState, JoinVoiceRequest, LeaveVoiceRequest, UpdateScreenShareRequest, UpdateSpeakingRequest,
    UpdateVoiceStateRequest, VoiceSession, VoiceState,
};
use crate::shared::AppResult;

pub async fn join_voice_channel(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<JoinVoiceRequest>,
) -> AppResult<Json<VoiceState>> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let voice_state = VoiceState {
        user_id: auth_user
            .0
            .sub
            .parse()
            .map_err(|_| crate::shared::AppError::bad_request("Invalid user ID"))?,
        username: auth_user.0.username.clone(),
        channel_id: request.channel_id,
        session_id: session_id.clone(),
        is_muted: false,
        is_deafened: false,
        is_screen_sharing: false,
        joined_at: now,
    };

    let voice_session = VoiceSession {
        session_id: session_id.clone(),
        user_id: voice_state.user_id,
        channel_id: request.channel_id,
        peer_connection_id: None,
        created_at: now,
    };

    state
        .voice_states
        .entry(request.channel_id)
        .or_insert_with(DashMap::new)
        .insert(voice_state.user_id, voice_state.clone());

    state.voice_sessions.insert(session_id, voice_session);

    // Broadcast on global channel so all users see voice state changes
    let global_event = serde_json::json!({
        "type": "voice_user_joined",
        "data": voice_state,
    });
    let _ = state.global_broadcast.send(global_event.to_string());

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
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| crate::shared::AppError::bad_request("Invalid user ID"))?;

    // Remove from voice states
    if let Some(channel_users) = state.voice_states.get(&request.channel_id) {
        channel_users.remove(&user_id);
        if channel_users.is_empty() {
            drop(channel_users);
            state.voice_states.remove(&request.channel_id);
        }
    }

    // Remove voice session
    state.voice_sessions.retain(|_, session| {
        !(session.user_id == user_id && session.channel_id == request.channel_id)
    });

    // Close all SFU transports for this user in this channel
    state
        .sfu_service
        .close_user_connections(request.channel_id, user_id)
        .await;

    // Broadcast on global channel
    let global_event = serde_json::json!({
        "type": "voice_user_left",
        "data": {
            "user_id": user_id.to_string(),
            "channel_id": request.channel_id.to_string(),
        },
    });
    let _ = state.global_broadcast.send(global_event.to_string());

    tracing::info!("User {} left voice channel {}", user_id, request.channel_id);

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
    let all_states: Vec<VoiceState> = state
        .voice_states
        .iter()
        .flat_map(|channel| {
            channel
                .value()
                .iter()
                .map(|r| r.value().clone())
                .collect::<Vec<_>>()
        })
        .collect();

    Ok(Json(all_states))
}

pub async fn update_voice_state(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<Uuid>,
    auth_user: AuthUser,
    Json(request): Json<UpdateVoiceStateRequest>,
) -> AppResult<Json<VoiceState>> {
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| crate::shared::AppError::bad_request("Invalid user ID"))?;

    let updated_state = {
        let channel_users = state
            .voice_states
            .get(&channel_id)
            .ok_or_else(|| crate::shared::AppError::not_found("Not in voice channel"))?;

        let mut voice_state = channel_users
            .get_mut(&user_id)
            .ok_or_else(|| crate::shared::AppError::not_found("Not in voice channel"))?;

        if let Some(is_muted) = request.is_muted {
            voice_state.is_muted = is_muted;
        }
        if let Some(is_deafened) = request.is_deafened {
            voice_state.is_deafened = is_deafened;
        }
        voice_state.clone()
    };

    // Broadcast on global channel so all users see mute/deafen changes
    let global_event = serde_json::json!({
        "type": "voice_state_updated",
        "data": updated_state,
    });
    let _ = state.global_broadcast.send(global_event.to_string());

    tracing::info!(
        "User {} updated voice state in channel {}: muted={}, deafened={}",
        updated_state.username,
        channel_id,
        updated_state.is_muted,
        updated_state.is_deafened
    );

    Ok(Json(updated_state))
}

pub async fn update_speaking_status(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<Uuid>,
    auth_user: AuthUser,
    Json(request): Json<UpdateSpeakingRequest>,
) -> AppResult<()> {
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| crate::shared::AppError::bad_request("Invalid user ID"))?;

    // Broadcast speaking status on global channel
    let global_event = serde_json::json!({
        "type": "voice_speaking",
        "data": {
            "user_id": user_id.to_string(),
            "channel_id": channel_id.to_string(),
            "is_speaking": request.is_speaking,
        },
    });
    let _ = state.global_broadcast.send(global_event.to_string());

    Ok(())
}

pub async fn update_screen_share(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<Uuid>,
    auth_user: AuthUser,
    Json(request): Json<UpdateScreenShareRequest>,
) -> AppResult<Json<VoiceState>> {
    let user_id: Uuid = auth_user
        .0
        .sub
        .parse()
        .map_err(|_| crate::shared::AppError::bad_request("Invalid user ID"))?;

    let updated_state = {
        let channel_users = state
            .voice_states
            .get(&channel_id)
            .ok_or_else(|| crate::shared::AppError::not_found("Not in voice channel"))?;

        let mut voice_state = channel_users
            .get_mut(&user_id)
            .ok_or_else(|| crate::shared::AppError::not_found("Not in voice channel"))?;

        voice_state.is_screen_sharing = request.is_screen_sharing;
        voice_state.clone()
    };

    let global_event = serde_json::json!({
        "type": "screen_share_updated",
        "data": updated_state,
    });
    let _ = state.global_broadcast.send(global_event.to_string());

    tracing::info!(
        "User {} {} screen sharing in channel {}",
        updated_state.username,
        if request.is_screen_sharing {
            "started"
        } else {
            "stopped"
        },
        channel_id
    );

    Ok(Json(updated_state))
}
