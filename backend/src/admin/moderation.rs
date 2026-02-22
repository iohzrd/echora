use axum::{
    extract::{Path, State},
    response::Json,
};
use chrono::{TimeDelta, Utc};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{
    AppState, Ban, BanRequest, KickRequest, ModAction, ModLogEntry, Mute, MuteRequest,
};
use crate::permissions::{self, Role};
use crate::shared::AppResult;
use crate::shared::validation;

pub async fn kick_user(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<KickRequest>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(actor_role, Role::Moderator)?;

    validation::validate_reason(&payload.reason)?;

    let target_role = database::get_user_role(&state.db, payload.user_id).await?;
    permissions::require_higher_role(actor_role, target_role)?;

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry::new(
            ModAction::Kick,
            actor_id,
            payload.user_id,
            payload.reason.clone(),
            None,
        ),
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
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<BanRequest>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(actor_role, Role::Moderator)?;

    validation::validate_reason(&payload.reason)?;
    validation::validate_positive_duration(payload.duration_hours, "duration_hours")?;

    let target_role = database::get_user_role(&state.db, payload.user_id).await?;
    permissions::require_higher_role(actor_role, target_role)?;

    let expires_at = payload
        .duration_hours
        .and_then(TimeDelta::try_hours)
        .map(|d| Utc::now() + d);

    let ban = Ban {
        id: Uuid::now_v7(),
        user_id: payload.user_id,
        banned_by: actor_id,
        reason: payload.reason.clone(),
        expires_at,
        created_at: Utc::now(),
    };

    database::create_ban(&state.db, &ban).await?;

    // Keep in-memory cache in sync so WS path avoids per-message DB queries
    state.cache_ban(payload.user_id);

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry::new(
            ModAction::Ban,
            actor_id,
            payload.user_id,
            payload.reason.clone(),
            payload
                .duration_hours
                .map(|h| format!("duration: {} hours", h)),
        ),
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
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(target_user_id): Path<Uuid>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(actor_role, Role::Moderator)?;

    database::remove_ban(&state.db, target_user_id).await?;

    state.uncache_ban(target_user_id);

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry::new(ModAction::Unban, actor_id, target_user_id, None, None),
    )
    .await?;

    state.broadcast_global(
        "user_unbanned",
        serde_json::json!({ "user_id": target_user_id }),
    );

    Ok(())
}

pub async fn list_bans(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> AppResult<Json<Vec<Ban>>> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()).await?;
    permissions::require_role(actor_role, Role::Moderator)?;

    let bans = database::get_all_bans(&state.db).await?;
    Ok(Json(bans))
}

pub async fn mute_user(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<MuteRequest>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(actor_role, Role::Moderator)?;

    validation::validate_reason(&payload.reason)?;
    validation::validate_positive_duration(payload.duration_hours, "duration_hours")?;

    let target_role = database::get_user_role(&state.db, payload.user_id).await?;
    permissions::require_higher_role(actor_role, target_role)?;

    let expires_at = payload
        .duration_hours
        .and_then(TimeDelta::try_hours)
        .map(|d| Utc::now() + d);

    let mute = Mute {
        id: Uuid::now_v7(),
        user_id: payload.user_id,
        muted_by: actor_id,
        reason: payload.reason.clone(),
        expires_at,
        created_at: Utc::now(),
    };

    database::create_mute(&state.db, &mute).await?;

    state.cache_mute(payload.user_id);

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry::new(
            ModAction::Mute,
            actor_id,
            payload.user_id,
            payload.reason.clone(),
            payload
                .duration_hours
                .map(|h| format!("duration: {} hours", h)),
        ),
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
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(target_user_id): Path<Uuid>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id();
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(actor_role, Role::Moderator)?;

    database::remove_mute(&state.db, target_user_id).await?;

    state.uncache_mute(target_user_id);

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry::new(ModAction::Unmute, actor_id, target_user_id, None, None),
    )
    .await?;

    state.broadcast_global(
        "user_unmuted",
        serde_json::json!({ "user_id": target_user_id }),
    );

    Ok(())
}

pub async fn list_mutes(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> AppResult<Json<Vec<Mute>>> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()).await?;
    permissions::require_role(actor_role, Role::Moderator)?;

    let mutes = database::get_all_mutes(&state.db).await?;
    Ok(Json(mutes))
}
