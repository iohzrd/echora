use axum::{
    extract::{Path, State},
    response::Json,
};
use chrono::{Duration, Utc};
use rand::Rng;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, CreateInviteRequest, Invite};
use crate::permissions::{self, Role};
use crate::shared::AppResult;

fn generate_invite_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
        .collect()
}

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
                && inv.expires_at.is_none_or(|e| e > Utc::now())
                && inv.max_uses.is_none_or(|max| inv.uses < max)
        }
        None => false,
    };

    Ok(Json(serde_json::json!({ "valid": valid })))
}
