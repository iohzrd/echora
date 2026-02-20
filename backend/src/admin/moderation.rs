use axum::{
    extract::{Path, State},
    response::Json,
};
use chrono::{Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, Ban, BanRequest, KickRequest, ModLogEntry, Mute, MuteRequest};
use crate::permissions::{self, Role};
use crate::shared::AppResult;

pub async fn kick_user(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<KickRequest>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let target_role = database::get_user_role(&state.db, payload.user_id).await?;
    permissions::require_higher_role(&actor_role, &target_role)?;

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry {
            id: Uuid::now_v7(),
            action: "kick".to_string(),
            moderator_id: actor_id,
            target_user_id: payload.user_id,
            reason: payload.reason.clone(),
            details: None,
            created_at: Utc::now(),
        },
    )
    .await?;

    state.broadcast_global(
        "user_kicked",
        serde_json::json!({
            "user_id": payload.user_id,
            "reason": payload.reason,
        }),
    );

    Ok(())
}

pub async fn ban_user(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BanRequest>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let target_role = database::get_user_role(&state.db, payload.user_id).await?;
    permissions::require_higher_role(&actor_role, &target_role)?;

    let expires_at = payload
        .duration_hours
        .map(|h| Utc::now() + Duration::hours(h));

    let ban = Ban {
        id: Uuid::now_v7(),
        user_id: payload.user_id,
        banned_by: actor_id,
        reason: payload.reason.clone(),
        expires_at,
        created_at: Utc::now(),
    };

    database::create_ban(&state.db, &ban).await?;

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry {
            id: Uuid::now_v7(),
            action: "ban".to_string(),
            moderator_id: actor_id,
            target_user_id: payload.user_id,
            reason: payload.reason.clone(),
            details: payload
                .duration_hours
                .map(|h| format!("duration: {} hours", h)),
            created_at: Utc::now(),
        },
    )
    .await?;

    state.broadcast_global(
        "user_banned",
        serde_json::json!({
            "user_id": payload.user_id,
            "reason": payload.reason,
        }),
    );

    Ok(())
}

pub async fn unban_user(
    auth_user: AuthUser,
    Path(target_user_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    database::remove_ban(&state.db, target_user_id).await?;

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry {
            id: Uuid::now_v7(),
            action: "unban".to_string(),
            moderator_id: actor_id,
            target_user_id,
            reason: None,
            details: None,
            created_at: Utc::now(),
        },
    )
    .await?;

    state.broadcast_global(
        "user_unbanned",
        serde_json::json!({ "user_id": target_user_id }),
    );

    Ok(())
}

pub async fn list_bans(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Ban>>> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let bans = database::get_all_bans(&state.db).await?;
    Ok(Json(bans))
}

pub async fn mute_user(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MuteRequest>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let target_role = database::get_user_role(&state.db, payload.user_id).await?;
    permissions::require_higher_role(&actor_role, &target_role)?;

    let expires_at = payload
        .duration_hours
        .map(|h| Utc::now() + Duration::hours(h));

    let mute = Mute {
        id: Uuid::now_v7(),
        user_id: payload.user_id,
        muted_by: actor_id,
        reason: payload.reason.clone(),
        expires_at,
        created_at: Utc::now(),
    };

    database::create_mute(&state.db, &mute).await?;

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry {
            id: Uuid::now_v7(),
            action: "mute".to_string(),
            moderator_id: actor_id,
            target_user_id: payload.user_id,
            reason: payload.reason.clone(),
            details: payload
                .duration_hours
                .map(|h| format!("duration: {} hours", h)),
            created_at: Utc::now(),
        },
    )
    .await?;

    state.broadcast_global(
        "user_muted",
        serde_json::json!({
            "user_id": payload.user_id,
            "reason": payload.reason,
            "expires_at": expires_at,
        }),
    );

    Ok(())
}

pub async fn unmute_user(
    auth_user: AuthUser,
    Path(target_user_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    database::remove_mute(&state.db, target_user_id).await?;

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry {
            id: Uuid::now_v7(),
            action: "unmute".to_string(),
            moderator_id: actor_id,
            target_user_id,
            reason: None,
            details: None,
            created_at: Utc::now(),
        },
    )
    .await?;

    state.broadcast_global(
        "user_unmuted",
        serde_json::json!({ "user_id": target_user_id }),
    );

    Ok(())
}

pub async fn list_mutes(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Mute>>> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let mutes = database::get_all_mutes(&state.db).await?;
    Ok(Json(mutes))
}
