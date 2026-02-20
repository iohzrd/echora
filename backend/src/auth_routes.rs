use axum::{extract::State, response::Json};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{
    AuthResponse, AuthUser, LoginRequest, RegisterRequest, User, UserInfo, create_jwt,
};
use crate::database;
use crate::models::AppState;
use crate::permissions::{self, Role};
use crate::shared::password;
use crate::shared::validation;
use crate::shared::{AppError, AppResult};

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> AppResult<Json<AuthResponse>> {
    let username = validation::validate_username(&payload.username)?;
    let email = validation::validate_email(&payload.email)?;
    validation::validate_password(&payload.password)?;

    // Check registration mode
    let reg_mode = database::get_server_setting(&state.db, "registration_mode").await?;
    if reg_mode == "invite_only" {
        let code = payload
            .invite_code
            .as_deref()
            .ok_or_else(|| AppError::forbidden("Registration requires an invite code"))?;
        database::use_invite_code(&state.db, code).await?;
    }

    let password_hash = password::hash_password(&payload.password)?;

    // First user becomes owner, rest are members
    let user_count = database::get_user_count(&state.db).await?;
    let role = if user_count == 0 {
        Role::Owner
    } else {
        Role::Member
    };

    let user = User {
        id: Uuid::now_v7(),
        username,
        email,
        password_hash,
        role,
        created_at: Utc::now(),
    };

    // Relies on DB unique constraints -- create_user maps constraint violations
    // to specific conflict errors (username taken, email in use)
    database::create_user(&state.db, &user).await?;

    let token = create_jwt(user.id, &user.username, user.role)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            role: user.role,
        },
    }))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let username = validation::validate_username(&payload.username)
        .map_err(|_| AppError::authentication("Invalid credentials"))?;

    if payload.password.is_empty() {
        return Err(AppError::authentication("Invalid credentials"));
    }

    let user = database::get_user_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::authentication("Invalid credentials"))?;

    password::verify_password(&payload.password, &user.password_hash)?;

    permissions::check_not_banned(&state.db, user.id).await?;

    let token = create_jwt(user.id, &user.username, user.role)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            role: user.role,
        },
    }))
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> AppResult<Json<UserInfo>> {
    let user = database::get_user_by_id(&state.db, auth_user.user_id())
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(UserInfo {
        id: user.id,
        username: user.username,
        email: user.email,
        role: user.role,
    }))
}
