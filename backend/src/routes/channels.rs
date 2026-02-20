use axum::{
    extract::{Path, State},
    response::Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, Channel, CreateChannelRequest, UpdateChannelRequest, UserPresence};
use crate::permissions::{self, Role};
use crate::shared::validation::validate_channel_name;
use crate::shared::{AppError, AppResult};

pub async fn get_channels(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
) -> AppResult<Json<Vec<Channel>>> {
    let channels = database::get_channels(&state.db).await?;
    Ok(Json(channels))
}

pub async fn create_channel(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<CreateChannelRequest>,
) -> AppResult<Json<Channel>> {
    let user_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, user_id).await?;
    permissions::require_role(&actor_role, Role::Admin)?;

    let name = validate_channel_name(&payload.name)?;

    let channel = Channel {
        id: Uuid::now_v7(),
        name,
        channel_type: payload.channel_type,
    };

    database::create_channel(&state.db, &channel, user_id).await?;

    state.broadcast_global("channel_created", serde_json::json!(channel));

    Ok(Json(channel))
}

pub async fn update_channel(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
    Json(payload): Json<UpdateChannelRequest>,
) -> AppResult<Json<Channel>> {
    let user_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, user_id).await?;
    permissions::require_role(&actor_role, Role::Admin)?;

    let name = validate_channel_name(&payload.name)?;

    database::update_channel(&state.db, channel_id, &name).await?;

    let channel = database::get_channel_by_id(&state.db, channel_id)
        .await?
        .ok_or_else(|| AppError::not_found("Channel not found"))?;

    state.broadcast_global("channel_updated", serde_json::json!(channel));

    Ok(Json(channel))
}

pub async fn delete_channel(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(channel_id): Path<Uuid>,
) -> AppResult<()> {
    let user_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, user_id).await?;
    permissions::require_role(&actor_role, Role::Admin)?;

    database::delete_channel(&state.db, channel_id).await?;

    // Clean up broadcast channel
    state.channel_broadcasts.remove(&channel_id);

    state.broadcast_global("channel_deleted", serde_json::json!({ "id": channel_id }));

    Ok(())
}

pub async fn get_online_users(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
) -> AppResult<Json<Vec<UserPresence>>> {
    let users: Vec<UserPresence> = state
        .online_users
        .iter()
        .map(|entry| entry.value().clone())
        .collect();
    Ok(Json(users))
}
