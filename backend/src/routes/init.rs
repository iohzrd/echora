use axum::{extract::State, response::Json};
use serde::Serialize;
use std::sync::Arc;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, Channel, MemberInfo, UserPresence, UserSummary, VoiceState};
use crate::permissions::Role;
use crate::shared::AppResult;

#[derive(Serialize)]
pub struct InitResponse {
    pub server_name: String,
    pub version: String,
    pub channels: Vec<Channel>,
    pub online_users: Vec<UserPresence>,
    pub voice_states: Vec<VoiceState>,
    pub members: Vec<MemberInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<UserSummary>>,
}

pub async fn get_init(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> AppResult<Json<InitResponse>> {
    let user_id = auth_user.user_id();

    // Run independent DB queries concurrently
    let (server_name, channels, actor_role, members) = tokio::join!(
        database::get_server_setting(&state.db, "server_name"),
        database::get_channels(&state.db),
        database::get_user_role(&state.db, user_id),
        database::get_all_members(&state.db),
    );
    let server_name = server_name?;
    let channels = channels?;
    let actor_role = actor_role?;
    let members = members?;

    let online_users: Vec<UserPresence> = state
        .online_users
        .iter()
        .map(|entry| entry.value().clone())
        .collect();

    let voice_states: Vec<VoiceState> = state.all_voice_states();

    // Include user list for moderators+ (needed for admin panel)
    let users = if actor_role >= Role::Moderator {
        Some(database::get_all_users(&state.db).await?)
    } else {
        None
    };

    Ok(Json(InitResponse {
        server_name,
        version: env!("CARGO_PKG_VERSION").to_string(),
        channels,
        online_users,
        voice_states,
        members,
        users,
    }))
}
