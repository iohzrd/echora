use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Json},
};
use object_store::{ObjectStoreExt, PutPayload};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{
    AuthResponse, AuthUser, LoginRequest, PublicProfile, RegisterRequest, UpdateProfileRequest,
    UserInfo, create_jwt,
};
use crate::database;
use crate::models::{AppState, avatar_url_from_path};
use crate::permissions::{self, Role};
use crate::shared::password;
use crate::shared::validation;
use crate::shared::{AppError, AppResult};

fn user_info_from_db(user: &crate::auth::User) -> UserInfo {
    UserInfo {
        id: user.id,
        username: user.username.clone(),
        email: user.email.clone(),
        role: user.role,
        avatar_url: avatar_url_from_path(user.id, &user.avatar_path),
        display_name: user.display_name.clone(),
    }
}

fn require_storage(state: &AppState) -> Result<&Arc<dyn object_store::ObjectStore>, AppError> {
    state
        .file_store
        .as_ref()
        .ok_or_else(|| AppError::bad_request("File uploads are not enabled on this server"))
}

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

    let user = crate::auth::User {
        id: Uuid::now_v7(),
        username,
        email,
        password_hash,
        role,
        created_at: chrono::Utc::now(),
        avatar_path: None,
        display_name: None,
    };

    // Relies on DB unique constraints -- create_user maps constraint violations
    // to specific conflict errors (username taken, email in use)
    database::create_user(&state.db, &user).await?;

    let token = create_jwt(user.id, &user.username, user.role)?;

    Ok(Json(AuthResponse {
        token,
        user: user_info_from_db(&user),
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
        user: user_info_from_db(&user),
    }))
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> AppResult<Json<UserInfo>> {
    let user = database::get_user_by_id(&state.db, auth_user.user_id())
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(user_info_from_db(&user)))
}

pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<UpdateProfileRequest>,
) -> AppResult<Json<AuthResponse>> {
    let user = database::get_user_by_id(&state.db, auth_user.user_id())
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    let mut current_username = user.username.clone();
    let mut changed = false;

    // Handle username change
    if let Some(ref new_username) = payload.username {
        let new_username = validation::validate_username(new_username)?;
        if new_username != current_username {
            let old_username = current_username.clone();
            database::update_username(&state.db, user.id, &new_username).await?;
            current_username = new_username.clone();
            changed = true;

            // Update in-memory online presence
            if let Some(mut presence) = state.online_users.get_mut(&user.id) {
                presence.username = new_username.clone();
            }

            // Update in-memory voice states
            for channel_users in state.voice_states.iter() {
                if let Some(mut vs) = channel_users.get_mut(&user.id) {
                    vs.username = new_username.clone();
                }
            }

            // Broadcast rename to all connected clients
            state.broadcast_global(
                "user_renamed",
                serde_json::json!({
                    "user_id": user.id,
                    "old_username": old_username,
                    "new_username": new_username,
                }),
            );
        }
    }

    // Handle display_name change
    if let Some(ref new_display_name) = payload.display_name {
        let validated = match new_display_name {
            Some(name) => Some(validation::validate_display_name(name)?),
            None => None,
        };
        database::update_user_display_name(&state.db, user.id, validated.as_deref()).await?;
        changed = true;
    }

    if changed {
        // Broadcast profile update
        let updated_user = database::get_user_by_id(&state.db, user.id)
            .await?
            .ok_or_else(|| AppError::not_found("User not found"))?;
        let info = user_info_from_db(&updated_user);

        state.broadcast_global(
            "user_profile_updated",
            serde_json::json!({
                "user_id": user.id,
                "username": info.username,
                "display_name": info.display_name,
                "avatar_url": info.avatar_url,
            }),
        );

        let token = create_jwt(user.id, &current_username, user.role)?;
        Ok(Json(AuthResponse { token, user: info }))
    } else {
        let token = create_jwt(user.id, &current_username, user.role)?;
        Ok(Json(AuthResponse {
            token,
            user: user_info_from_db(&user),
        }))
    }
}

pub async fn upload_avatar(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<UserInfo>> {
    let store = require_storage(&state)?;
    let user_id = auth_user.user_id();

    let mut file_data: Option<(Vec<u8>, String)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("Invalid multipart data: {e}")))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "file" {
            let content_type = field
                .content_type()
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    let fname = field.file_name().unwrap_or("upload").to_string();
                    mime_guess::from_path(&fname)
                        .first_raw()
                        .unwrap_or("application/octet-stream")
                        .to_string()
                });
            validation::validate_avatar_content_type(&content_type)?;

            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::bad_request(format!("Failed to read file: {e}")))?;

            if data.is_empty() {
                return Err(AppError::bad_request("File is empty"));
            }
            if data.len() > validation::MAX_AVATAR_SIZE {
                return Err(AppError::bad_request(format!(
                    "Avatar image exceeds maximum size of {}MB",
                    validation::MAX_AVATAR_SIZE / (1024 * 1024)
                )));
            }

            file_data = Some((data.to_vec(), content_type));
        }
    }

    let (data, content_type) =
        file_data.ok_or_else(|| AppError::bad_request("Missing 'file' field"))?;

    let ext = match content_type.as_str() {
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/jpeg" => "jpg",
        _ => "png",
    };

    // Delete old avatar if one exists
    let user = database::get_user_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;
    if let Some(ref old_path) = user.avatar_path {
        let old_object_path = object_store::path::Path::from(old_path.clone());
        let _ = store.delete(&old_object_path).await;
    }

    let storage_path = format!("avatars/{user_id}.{ext}");
    let object_path = object_store::path::Path::from(storage_path.clone());
    let payload = PutPayload::from(data);
    store
        .put(&object_path, payload)
        .await
        .map_err(|e| AppError::internal(format!("Failed to store avatar image: {e}")))?;

    database::update_user_avatar(&state.db, user_id, Some(&storage_path)).await?;

    let avatar_url = avatar_url_from_path(user_id, &Some(storage_path));

    // Update in-memory presence
    if let Some(mut presence) = state.online_users.get_mut(&user_id) {
        presence.avatar_url = avatar_url.clone();
    }

    // Update in-memory voice states
    for channel_users in state.voice_states.iter() {
        if let Some(mut vs) = channel_users.get_mut(&user_id) {
            vs.avatar_url = avatar_url.clone();
        }
    }

    // Broadcast avatar update
    state.broadcast_global(
        "user_avatar_updated",
        serde_json::json!({
            "user_id": user_id,
            "avatar_url": avatar_url,
        }),
    );

    let updated_user = database::get_user_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(user_info_from_db(&updated_user)))
}

pub async fn delete_avatar(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> AppResult<Json<UserInfo>> {
    let user_id = auth_user.user_id();

    let user = database::get_user_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    if let Some(ref avatar_path) = user.avatar_path {
        if let Some(store) = &state.file_store {
            let object_path = object_store::path::Path::from(avatar_path.clone());
            let _ = store.delete(&object_path).await;
        }
    }

    database::update_user_avatar(&state.db, user_id, None).await?;

    // Update in-memory presence
    if let Some(mut presence) = state.online_users.get_mut(&user_id) {
        presence.avatar_url = None;
    }

    // Update in-memory voice states
    for channel_users in state.voice_states.iter() {
        if let Some(mut vs) = channel_users.get_mut(&user_id) {
            vs.avatar_url = None;
        }
    }

    // Broadcast avatar removal
    state.broadcast_global(
        "user_avatar_updated",
        serde_json::json!({
            "user_id": user_id,
            "avatar_url": null,
        }),
    );

    let updated_user = database::get_user_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(user_info_from_db(&updated_user)))
}

pub async fn get_user_profile(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<PublicProfile>> {
    let user = database::get_user_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(PublicProfile {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        avatar_url: avatar_url_from_path(user.id, &user.avatar_path),
        role: user.role,
        created_at: user.created_at,
    }))
}

pub async fn get_avatar(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let store = require_storage(&state)?;

    let user = database::get_user_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    let avatar_path = user
        .avatar_path
        .ok_or_else(|| AppError::not_found("User has no avatar"))?;

    let content_type = if avatar_path.ends_with(".png") {
        "image/png"
    } else if avatar_path.ends_with(".gif") {
        "image/gif"
    } else if avatar_path.ends_with(".webp") {
        "image/webp"
    } else if avatar_path.ends_with(".jpg") {
        "image/jpeg"
    } else {
        "application/octet-stream"
    };

    let object_path = object_store::path::Path::from(avatar_path);
    let result = store
        .get(&object_path)
        .await
        .map_err(|e| AppError::not_found(format!("Avatar image not found in storage: {e}")))?;

    let stream = result.into_stream();
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    if let Ok(ct) = HeaderValue::from_str(content_type) {
        headers.insert(header::CONTENT_TYPE, ct);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );

    Ok((StatusCode::OK, headers, body))
}
