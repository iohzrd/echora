use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::{Duration, Utc};
use rand::Rng;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{
    AppState, Ban, BanRequest, CreateInviteRequest, Invite, KickRequest, ModLogEntry, Mute,
    MuteRequest, RoleChangeRequest, ServerSettingUpdate, UserSummary,
};
use crate::permissions::{self, Role};
use crate::shared::{AppError, AppResult};

fn generate_invite_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
        .collect()
}

// --- User Management (Admin+) ---

pub async fn get_all_users(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<UserSummary>>> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()?).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let users = database::get_all_users(&state.db).await?;
    Ok(Json(users))
}

pub async fn change_user_role(
    auth_user: AuthUser,
    Path(target_user_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RoleChangeRequest>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id()?;
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(&actor_role, Role::Admin)?;

    let target_role = database::get_user_role(&state.db, target_user_id).await?;

    if target_role == "owner" {
        return Err(AppError::forbidden("Cannot change the owner's role"));
    }

    permissions::can_assign_role(&actor_role, &payload.role)?;
    permissions::require_higher_role(&actor_role, &target_role)?;

    database::set_user_role(&state.db, target_user_id, &payload.role).await?;

    database::create_mod_log_entry(
        &state.db,
        &ModLogEntry {
            id: Uuid::now_v7(),
            action: "role_change".to_string(),
            moderator_id: actor_id,
            target_user_id,
            reason: None,
            details: Some(
                serde_json::json!({
                    "from": target_role,
                    "to": payload.role,
                })
                .to_string(),
            ),
            created_at: Utc::now(),
        },
    )
    .await?;

    state.broadcast_global(
        "user_role_changed",
        serde_json::json!({
            "user_id": target_user_id,
            "new_role": payload.role,
        }),
    );

    Ok(())
}

// --- Moderation (Moderator+) ---

pub async fn kick_user(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<KickRequest>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id()?;
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
    let actor_id = auth_user.user_id()?;
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
    let actor_id = auth_user.user_id()?;
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
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()?).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let bans = database::get_all_bans(&state.db).await?;
    Ok(Json(bans))
}

pub async fn mute_user(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MuteRequest>,
) -> AppResult<()> {
    let actor_id = auth_user.user_id()?;
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
    let actor_id = auth_user.user_id()?;
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
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()?).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let mutes = database::get_all_mutes(&state.db).await?;
    Ok(Json(mutes))
}

// --- Invites (Moderator+) ---

pub async fn create_invite(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateInviteRequest>,
) -> AppResult<Json<Invite>> {
    let actor_id = auth_user.user_id()?;
    let actor_role = database::get_user_role(&state.db, actor_id).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let expires_at = payload
        .expires_in_hours
        .map(|h| Utc::now() + Duration::hours(h));

    let invite = Invite {
        id: Uuid::now_v7(),
        code: generate_invite_code(),
        created_by: actor_id,
        max_uses: payload.max_uses,
        uses: 0,
        expires_at,
        revoked: false,
        created_at: Utc::now(),
    };

    database::create_invite(&state.db, &invite).await?;

    Ok(Json(invite))
}

pub async fn list_invites(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Invite>>> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()?).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let invites = database::get_all_invites(&state.db).await?;
    Ok(Json(invites))
}

pub async fn revoke_invite(
    auth_user: AuthUser,
    Path(invite_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> AppResult<()> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()?).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    database::revoke_invite(&state.db, invite_id).await?;

    Ok(())
}

pub async fn validate_invite(
    Path(code): Path<String>,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    let invite = database::get_invite_by_code(&state.db, &code).await?;

    let valid = match invite {
        Some(inv) => {
            !inv.revoked
                && inv.expires_at.map_or(true, |e| e > Utc::now())
                && inv.max_uses.map_or(true, |max| inv.uses < max)
        }
        None => false,
    };

    Ok(Json(serde_json::json!({ "valid": valid })))
}

// --- Server Settings (Admin+) ---

pub async fn get_settings(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<std::collections::HashMap<String, String>>> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()?).await?;
    permissions::require_role(&actor_role, Role::Admin)?;

    let settings = database::get_all_server_settings(&state.db).await?;
    Ok(Json(settings))
}

pub async fn update_setting(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ServerSettingUpdate>,
) -> AppResult<()> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()?).await?;
    permissions::require_role(&actor_role, Role::Admin)?;

    // Validate known settings
    match payload.key.as_str() {
        "registration_mode" => {
            if payload.value != "open" && payload.value != "invite_only" {
                return Err(AppError::bad_request(
                    "registration_mode must be 'open' or 'invite_only'",
                ));
            }
        }
        _ => {
            return Err(AppError::bad_request(format!(
                "Unknown setting: {}",
                payload.key
            )));
        }
    }

    database::set_server_setting(&state.db, &payload.key, &payload.value).await?;

    state.broadcast_global(
        "settings_updated",
        serde_json::json!({
            "key": payload.key,
            "value": payload.value,
        }),
    );

    Ok(())
}

// --- Moderation Log (Moderator+) ---

#[derive(Debug, Deserialize)]
pub struct ModLogQuery {
    pub limit: Option<i64>,
}

pub async fn get_moderation_log(
    auth_user: AuthUser,
    Query(query): Query<ModLogQuery>,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<ModLogEntry>>> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()?).await?;
    permissions::require_role(&actor_role, Role::Moderator)?;

    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let log = database::get_mod_log(&state.db, limit).await?;
    Ok(Json(log))
}
