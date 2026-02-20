use axum::{
    extract::{Path, State},
    response::Json,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, ModLogEntry, RoleChangeRequest, UserSummary};
use crate::permissions::{self, Role};
use crate::shared::{AppError, AppResult};

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
