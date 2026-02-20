use axum::{extract::State, response::Json};
use std::sync::Arc;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, ServerSettingUpdate};
use crate::permissions::{self, Role};
use crate::shared::{AppError, AppResult};

pub async fn get_settings(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<std::collections::HashMap<String, String>>> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()).await?;
    permissions::require_role(&actor_role, Role::Admin)?;

    let settings = database::get_all_server_settings(&state.db).await?;
    Ok(Json(settings))
}

pub async fn update_setting(
    auth_user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ServerSettingUpdate>,
) -> AppResult<()> {
    let actor_role = database::get_user_role(&state.db, auth_user.user_id()).await?;
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
        "server_name" => {
            if payload.value.trim().is_empty() {
                return Err(AppError::bad_request("server_name cannot be empty"));
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
