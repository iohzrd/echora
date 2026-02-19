use axum::{extract::State, response::Json};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{AuthResponse, LoginRequest, RegisterRequest, User, UserInfo, create_jwt};
use crate::database;
use crate::models::AppState;
use crate::shared::password;
use crate::shared::{AppError, AppResult};

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> AppResult<Json<AuthResponse>> {
    let username = payload.username.trim().to_string();
    let email = payload.email.trim().to_lowercase();
    let password = &payload.password;

    // Username validation
    if username.is_empty() || username.len() > 32 {
        return Err(AppError::bad_request(
            "Username must be between 1 and 32 characters",
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::bad_request(
            "Username can only contain letters, numbers, underscores, and hyphens",
        ));
    }

    // Email validation
    if email.is_empty() || email.len() > 254 {
        return Err(AppError::bad_request("Invalid email address"));
    }
    if !email.contains('@') || !email.contains('.') {
        return Err(AppError::bad_request("Invalid email address"));
    }
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].len() < 3 {
        return Err(AppError::bad_request("Invalid email address"));
    }

    // Password validation
    if password.len() < 8 {
        return Err(AppError::bad_request(
            "Password must be at least 8 characters",
        ));
    }
    if password.len() > 128 {
        return Err(AppError::bad_request(
            "Password must be at most 128 characters",
        ));
    }

    if database::get_user_by_username(&state.db, &username)
        .await?
        .is_some()
    {
        return Err(AppError::conflict("Username already taken"));
    }

    if database::get_user_by_email(&state.db, &email)
        .await?
        .is_some()
    {
        return Err(AppError::conflict("Email already in use"));
    }

    let password_hash = password::hash_password(password)?;

    let user = User {
        id: Uuid::now_v7(),
        username,
        email,
        password_hash,
        created_at: Utc::now(),
    };

    database::create_user(&state.db, &user).await?;

    let token = create_jwt(user.id, &user.username)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
        },
    }))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    if payload.username.trim().is_empty() || payload.password.is_empty() {
        return Err(AppError::authentication("Invalid credentials"));
    }

    let user = database::get_user_by_username(&state.db, payload.username.trim())
        .await?
        .ok_or_else(|| AppError::authentication("Invalid credentials"))?;

    password::verify_password(&payload.password, &user.password_hash)?;

    let token = create_jwt(user.id, &user.username)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
        },
    }))
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    auth_user: crate::auth::AuthUser,
) -> AppResult<Json<UserInfo>> {
    let user = database::get_user_by_username(&state.db, &auth_user.0.username)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(UserInfo {
        id: user.id,
        username: user.username,
        email: user.email,
    }))
}
