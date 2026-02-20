use axum::{
    extract::{Query, State},
    response::Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, ModLogEntry};
use crate::permissions::{self, Role};
use crate::shared::AppResult;

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
