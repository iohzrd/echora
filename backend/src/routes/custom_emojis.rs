use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Json},
};
use object_store::{ObjectStoreExt, PutPayload};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, CustomEmoji};
use crate::permissions::{self, Role};
use crate::shared::AppError;
use crate::shared::validation::{
    MAX_CUSTOM_EMOJI_SIZE, validate_emoji_content_type, validate_emoji_name,
};

fn require_storage(state: &AppState) -> Result<&Arc<dyn object_store::ObjectStore>, AppError> {
    state
        .file_store
        .as_ref()
        .ok_or_else(|| AppError::bad_request("File uploads are not enabled on this server"))
}

pub async fn list_custom_emojis(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
) -> Result<Json<Vec<CustomEmoji>>, AppError> {
    let emojis =
        sqlx::query_as::<_, CustomEmoji>("SELECT * FROM custom_emojis ORDER BY created_at ASC")
            .fetch_all(&state.db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to fetch custom emojis: {e}")))?;

    Ok(Json(emojis))
}

pub async fn upload_custom_emoji(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    mut multipart: Multipart,
) -> Result<Json<CustomEmoji>, AppError> {
    let store = require_storage(&state)?;
    let user_id = auth_user.user_id();
    permissions::check_not_muted(&state.db, user_id).await?;

    let mut name: Option<String> = None;
    let mut file_data: Option<(Vec<u8>, String)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("Invalid multipart data: {e}")))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        match field_name.as_str() {
            "name" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::bad_request(format!("Failed to read name: {e}")))?;
                name = Some(validate_emoji_name(&text)?);
            }
            "file" => {
                let content_type =
                    field
                        .content_type()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            let fname = field.file_name().unwrap_or("upload").to_string();
                            mime_guess::from_path(&fname)
                                .first_raw()
                                .unwrap_or("application/octet-stream")
                                .to_string()
                        });
                validate_emoji_content_type(&content_type)?;

                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::bad_request(format!("Failed to read file: {e}")))?;

                if data.is_empty() {
                    return Err(AppError::bad_request("File is empty"));
                }
                if data.len() > MAX_CUSTOM_EMOJI_SIZE {
                    return Err(AppError::bad_request(format!(
                        "Emoji image exceeds maximum size of {}KB",
                        MAX_CUSTOM_EMOJI_SIZE / 1024
                    )));
                }

                file_data = Some((data.to_vec(), content_type));
            }
            _ => {}
        }
    }

    let name = name.ok_or_else(|| AppError::bad_request("Missing 'name' field"))?;
    let (data, content_type) =
        file_data.ok_or_else(|| AppError::bad_request("Missing 'file' field"))?;

    let emoji_id = Uuid::now_v7();
    let ext = match content_type.as_str() {
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/jpeg" => "jpg",
        _ => "png",
    };
    let storage_path = format!("emojis/{emoji_id}.{ext}");

    let object_path = object_store::path::Path::from(storage_path.clone());
    let payload = PutPayload::from(data);
    store
        .put(&object_path, payload)
        .await
        .map_err(|e| AppError::internal(format!("Failed to store emoji image: {e}")))?;

    let emoji = sqlx::query_as::<_, CustomEmoji>(
        "INSERT INTO custom_emojis (id, name, uploaded_by, storage_path, content_type)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(emoji_id)
    .bind(&name)
    .bind(user_id)
    .bind(&storage_path)
    .bind(&content_type)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
            AppError::bad_request(format!("An emoji with the name '{name}' already exists"))
        } else {
            AppError::internal(format!("Failed to save custom emoji: {e}"))
        }
    })?;

    Ok(Json(emoji))
}

pub async fn delete_custom_emoji(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(emoji_id): Path<Uuid>,
) -> Result<(), AppError> {
    let user_id = auth_user.user_id();

    let emoji = sqlx::query_as::<_, CustomEmoji>("SELECT * FROM custom_emojis WHERE id = $1")
        .bind(emoji_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("Database error: {e}")))?
        .ok_or_else(|| AppError::not_found("Custom emoji not found"))?;

    // Only the uploader or mods/admins can delete
    if emoji.uploaded_by != user_id {
        let role = database::get_user_role(&state.db, user_id).await?;
        if !matches!(role, Role::Admin | Role::Owner | Role::Moderator) {
            return Err(AppError::forbidden(
                "Only the uploader or moderators can delete custom emojis",
            ));
        }
    }

    // Delete from storage
    if let Some(store) = &state.file_store {
        let object_path = object_store::path::Path::from(emoji.storage_path);
        let _ = store.delete(&object_path).await;
    }

    sqlx::query("DELETE FROM custom_emojis WHERE id = $1")
        .bind(emoji_id)
        .execute(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to delete custom emoji: {e}")))?;

    Ok(())
}

pub async fn get_custom_emoji_image(
    State(state): State<Arc<AppState>>,
    Path(emoji_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let store = require_storage(&state)?;

    let emoji = sqlx::query_as::<_, CustomEmoji>("SELECT * FROM custom_emojis WHERE id = $1")
        .bind(emoji_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("Database error: {e}")))?
        .ok_or_else(|| AppError::not_found("Custom emoji not found"))?;

    let object_path = object_store::path::Path::from(emoji.storage_path);
    let result = store
        .get(&object_path)
        .await
        .map_err(|e| AppError::not_found(format!("Emoji image not found in storage: {e}")))?;

    let stream = result.into_stream();
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    if let Ok(ct) = HeaderValue::from_str(&emoji.content_type) {
        headers.insert(header::CONTENT_TYPE, ct);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    Ok((StatusCode::OK, headers, body))
}
