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
use crate::models::{AppState, Attachment};
use crate::shared::validation::{
    MAX_ATTACHMENT_SIZE, validate_attachment_content_type, validate_filename,
};
use crate::shared::{AppError, AppResult};

fn require_storage(state: &AppState) -> Result<&Arc<dyn object_store::ObjectStore>, AppError> {
    state
        .file_store
        .as_ref()
        .ok_or_else(|| AppError::bad_request("File uploads are not enabled on this server"))
}

pub async fn upload_attachment(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    mut multipart: Multipart,
) -> AppResult<Json<Attachment>> {
    let store = require_storage(&state)?;
    let user_id = auth_user.user_id();
    crate::permissions::check_not_muted(&state.db, user_id).await?;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("Invalid multipart data: {e}")))?
        .ok_or_else(|| AppError::bad_request("No file provided"))?;

    let original_filename = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "upload".to_string());
    let filename = validate_filename(&original_filename)?;

    let content_type = field
        .content_type()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            mime_guess::from_path(&filename)
                .first_raw()
                .unwrap_or("application/octet-stream")
                .to_string()
        });
    validate_attachment_content_type(&content_type)?;

    let data = field
        .bytes()
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read file data: {e}")))?;

    if data.is_empty() {
        return Err(AppError::bad_request("File is empty"));
    }
    if data.len() > MAX_ATTACHMENT_SIZE {
        return Err(AppError::bad_request(format!(
            "File exceeds maximum size of {}MB",
            MAX_ATTACHMENT_SIZE / (1024 * 1024)
        )));
    }

    let attachment_id = Uuid::now_v7();
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let storage_path = if ext.is_empty() {
        format!("attachments/{attachment_id}")
    } else {
        format!("attachments/{attachment_id}.{ext}")
    };

    let object_path = object_store::path::Path::from(storage_path.clone());
    let payload = PutPayload::from(data.clone());
    store
        .put(&object_path, payload)
        .await
        .map_err(|e| AppError::internal(format!("Failed to store file: {e}")))?;

    let size = data.len() as i64;

    let attachment = sqlx::query_as::<_, Attachment>(
        "INSERT INTO attachments (id, filename, content_type, size, storage_path, uploader_id)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(attachment_id)
    .bind(&filename)
    .bind(&content_type)
    .bind(size)
    .bind(&storage_path)
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to save attachment metadata: {e}")))?;

    Ok(Json(attachment))
}

pub async fn download_attachment(
    State(state): State<Arc<AppState>>,
    Path((attachment_id, _filename)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let store = require_storage(&state)?;

    let attachment = sqlx::query_as::<_, Attachment>("SELECT * FROM attachments WHERE id = $1")
        .bind(attachment_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("Database error: {e}")))?
        .ok_or_else(|| AppError::not_found("Attachment not found"))?;

    let object_path = object_store::path::Path::from(attachment.storage_path);
    let result = store
        .get(&object_path)
        .await
        .map_err(|e| AppError::not_found(format!("File not found in storage: {e}")))?;

    let stream = result.into_stream();
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    if let Ok(ct) = HeaderValue::from_str(&attachment.content_type) {
        headers.insert(header::CONTENT_TYPE, ct);
    }
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "inline; filename=\"{}\"",
            attachment.filename.replace('"', "\\\"")
        ))
        .unwrap_or_else(|_| HeaderValue::from_static("inline")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    Ok((StatusCode::OK, headers, body))
}
