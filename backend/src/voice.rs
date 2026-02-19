use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::{AppState, JoinVoiceRequest, LeaveVoiceRequest, VoiceSession, VoiceState};
use crate::shared::AppResult;

pub async fn join_voice_channel(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<JoinVoiceRequest>,
) -> AppResult<Json<VoiceState>> {
    let user_id = auth_user.user_id()?;
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let voice_state = VoiceState {
        user_id,
        username: auth_user.0.username,
        channel_id: request.channel_id,
        session_id: session_id.clone(),
        is_muted: false,
        is_deafened: false,
        is_screen_sharing: false,
        joined_at: now,
    };

    let voice_session = VoiceSession {
        session_id: session_id.clone(),
        user_id,
        channel_id: request.channel_id,
        peer_connection_id: None,
        created_at: now,
    };

    state
        .voice_states
        .entry(request.channel_id)
        .or_insert_with(DashMap::new)
        .insert(user_id, voice_state.clone());

    state.voice_sessions.insert(session_id, voice_session);

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
    let user_id = auth_user.user_id()?;

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

    state.broadcast_global(
        "voice_user_left",
        serde_json::json!({
            "user_id": user_id,
            "channel_id": request.channel_id,
        }),
    );

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
        .flat_map(|entry| {
            entry
                .value()
                .iter()
                .map(|r| r.value().clone())
                .collect::<Vec<_>>()
        })
        .collect();

    Ok(Json(all_states))
}
