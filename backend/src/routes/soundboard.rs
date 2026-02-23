use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Json},
};
use dashmap::DashMap;
use object_store::{ObjectStoreExt, PutPayload};
use std::sync::{Arc, LazyLock};
use std::time::Instant;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::database;
use crate::models::{AppState, PlaySoundRequest, SoundboardSound, UpdateSoundRequest};
use crate::permissions::Role;
use crate::shared::AppError;
use crate::shared::validation::{
    MAX_SOUNDBOARD_SOUND_DURATION_MS, MAX_SOUNDBOARD_SOUND_SIZE, MAX_SOUNDBOARD_SOUNDS,
    validate_soundboard_content_type, validate_soundboard_sound_name, validate_soundboard_volume,
};

/// Per-sound cooldown tracking: sound_id -> last_played Instant
static SOUND_COOLDOWNS: LazyLock<DashMap<Uuid, Instant>> = LazyLock::new(DashMap::new);
const SOUND_COOLDOWN_SECS: u64 = 5;

fn require_storage(state: &AppState) -> Result<&Arc<dyn object_store::ObjectStore>, AppError> {
    state
        .file_store
        .as_ref()
        .ok_or_else(|| AppError::bad_request("File uploads are not enabled on this server"))
}

/// Measure audio duration in milliseconds using symphonia.
fn measure_audio_duration_ms(data: &[u8], content_type: &str) -> Result<i32, AppError> {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let cursor = std::io::Cursor::new(data.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    match content_type {
        "audio/mpeg" => hint.with_extension("mp3"),
        "audio/ogg" => hint.with_extension("ogg"),
        "audio/wav" => hint.with_extension("wav"),
        _ => &mut hint,
    };

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| AppError::bad_request(format!("Failed to read audio file: {e}")))?;

    let reader = probed.format;

    let track = reader
        .default_track()
        .ok_or_else(|| AppError::bad_request("No audio track found in file"))?;

    let time_base = track.codec_params.time_base;
    let n_frames = track.codec_params.n_frames;

    match (time_base, n_frames) {
        (Some(tb), Some(frames)) => {
            let time = tb.calc_time(frames);
            let ms = (time.seconds as f64 + time.frac) * 1000.0;
            Ok(ms.round() as i32)
        }
        _ => {
            // Fallback: estimate from sample rate and frame count if available
            if let (Some(sample_rate), Some(frames)) =
                (track.codec_params.sample_rate, track.codec_params.n_frames)
            {
                let duration_s = frames as f64 / sample_rate as f64;
                Ok((duration_s * 1000.0).round() as i32)
            } else {
                Err(AppError::bad_request("Unable to determine audio duration"))
            }
        }
    }
}

pub async fn list_sounds(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
) -> Result<Json<Vec<SoundboardSound>>, AppError> {
    let sounds = sqlx::query_as::<_, SoundboardSound>(
        "SELECT * FROM soundboard_sounds ORDER BY created_at ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch sounds: {e}")))?;

    Ok(Json(sounds))
}

pub async fn get_sound(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(sound_id): Path<Uuid>,
) -> Result<Json<SoundboardSound>, AppError> {
    let sound =
        sqlx::query_as::<_, SoundboardSound>("SELECT * FROM soundboard_sounds WHERE id = $1")
            .bind(sound_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| AppError::internal(format!("Database error: {e}")))?
            .ok_or_else(|| AppError::not_found("Sound not found"))?;

    Ok(Json(sound))
}

pub async fn upload_sound(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    mut multipart: Multipart,
) -> Result<Json<SoundboardSound>, AppError> {
    let store = require_storage(&state)?;
    let user_id = auth_user.user_id();
    crate::permissions::check_not_muted(&state.db, user_id).await?;

    // Check sound count limit
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM soundboard_sounds")
        .fetch_one(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("Database error: {e}")))?;
    if count.0 as usize >= MAX_SOUNDBOARD_SOUNDS {
        return Err(AppError::bad_request(format!(
            "Maximum of {MAX_SOUNDBOARD_SOUNDS} sounds reached"
        )));
    }

    let mut name: Option<String> = None;
    let mut volume: f64 = 1.0;
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
                name = Some(validate_soundboard_sound_name(&text)?);
            }
            "volume" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::bad_request(format!("Failed to read volume: {e}")))?;
                volume = text
                    .parse::<f64>()
                    .map_err(|_| AppError::bad_request("Invalid volume value"))?;
                validate_soundboard_volume(volume)?;
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
                validate_soundboard_content_type(&content_type)?;

                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::bad_request(format!("Failed to read file: {e}")))?;

                if data.is_empty() {
                    return Err(AppError::bad_request("File is empty"));
                }
                if data.len() > MAX_SOUNDBOARD_SOUND_SIZE {
                    return Err(AppError::bad_request(format!(
                        "Sound file exceeds maximum size of {}KB",
                        MAX_SOUNDBOARD_SOUND_SIZE / 1024
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

    // Measure duration
    let duration_ms = measure_audio_duration_ms(&data, &content_type)?;
    if duration_ms > MAX_SOUNDBOARD_SOUND_DURATION_MS {
        return Err(AppError::bad_request(format!(
            "Sound duration ({:.1}s) exceeds maximum of {:.1}s",
            duration_ms as f64 / 1000.0,
            MAX_SOUNDBOARD_SOUND_DURATION_MS as f64 / 1000.0
        )));
    }
    if duration_ms <= 0 {
        return Err(AppError::bad_request("Sound file has no audio content"));
    }

    let sound_id = Uuid::now_v7();
    let ext = match content_type.as_str() {
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/wav" => "wav",
        _ => "mp3",
    };
    let storage_path = format!("soundboard/{sound_id}.{ext}");

    let object_path = object_store::path::Path::from(storage_path.clone());
    let payload = PutPayload::from(data.clone());
    store
        .put(&object_path, payload)
        .await
        .map_err(|e| AppError::internal(format!("Failed to store sound file: {e}")))?;

    let sound = sqlx::query_as::<_, SoundboardSound>(
        "INSERT INTO soundboard_sounds (id, name, volume, file_size, duration_ms, content_type, storage_path, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING *",
    )
    .bind(sound_id)
    .bind(&name)
    .bind(volume)
    .bind(data.len() as i32)
    .bind(duration_ms)
    .bind(&content_type)
    .bind(&storage_path)
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to save sound: {e}")))?;

    // Broadcast creation event
    state.broadcast_global("soundboard_sound_created", serde_json::json!(sound));

    Ok(Json(sound))
}

pub async fn update_sound(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(sound_id): Path<Uuid>,
    Json(req): Json<UpdateSoundRequest>,
) -> Result<Json<SoundboardSound>, AppError> {
    let user_id = auth_user.user_id();

    let existing =
        sqlx::query_as::<_, SoundboardSound>("SELECT * FROM soundboard_sounds WHERE id = $1")
            .bind(sound_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| AppError::internal(format!("Database error: {e}")))?
            .ok_or_else(|| AppError::not_found("Sound not found"))?;

    // Only the uploader or mods/admins can update
    if existing.created_by != user_id {
        let role = database::get_user_role(&state.db, user_id).await?;
        if !matches!(role, Role::Admin | Role::Owner | Role::Moderator) {
            return Err(AppError::forbidden(
                "Only the uploader or moderators can edit sounds",
            ));
        }
    }

    let new_name = match &req.name {
        Some(n) => validate_soundboard_sound_name(n)?,
        None => existing.name.clone(),
    };
    let new_volume = match req.volume {
        Some(v) => {
            validate_soundboard_volume(v)?;
            v
        }
        None => existing.volume,
    };

    let sound = sqlx::query_as::<_, SoundboardSound>(
        "UPDATE soundboard_sounds SET name = $1, volume = $2, updated_at = NOW() WHERE id = $3 RETURNING *",
    )
    .bind(&new_name)
    .bind(new_volume)
    .bind(sound_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::internal(format!("Failed to update sound: {e}")))?;

    state.broadcast_global("soundboard_sound_updated", serde_json::json!(sound));

    Ok(Json(sound))
}

pub async fn delete_sound(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(sound_id): Path<Uuid>,
) -> Result<(), AppError> {
    let user_id = auth_user.user_id();

    let sound =
        sqlx::query_as::<_, SoundboardSound>("SELECT * FROM soundboard_sounds WHERE id = $1")
            .bind(sound_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| AppError::internal(format!("Database error: {e}")))?
            .ok_or_else(|| AppError::not_found("Sound not found"))?;

    // Only the uploader or mods/admins can delete
    if sound.created_by != user_id {
        let role = database::get_user_role(&state.db, user_id).await?;
        if !matches!(role, Role::Admin | Role::Owner | Role::Moderator) {
            return Err(AppError::forbidden(
                "Only the uploader or moderators can delete sounds",
            ));
        }
    }

    // Delete from storage
    if let Some(store) = &state.file_store {
        let object_path = object_store::path::Path::from(sound.storage_path);
        let _ = store.delete(&object_path).await;
    }

    sqlx::query("DELETE FROM soundboard_sounds WHERE id = $1")
        .bind(sound_id)
        .execute(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("Failed to delete sound: {e}")))?;

    state.broadcast_global(
        "soundboard_sound_deleted",
        serde_json::json!({ "sound_id": sound_id }),
    );

    Ok(())
}

pub async fn get_sound_audio(
    State(state): State<Arc<AppState>>,
    Path(sound_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let store = require_storage(&state)?;

    let sound =
        sqlx::query_as::<_, SoundboardSound>("SELECT * FROM soundboard_sounds WHERE id = $1")
            .bind(sound_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| AppError::internal(format!("Database error: {e}")))?
            .ok_or_else(|| AppError::not_found("Sound not found"))?;

    let object_path = object_store::path::Path::from(sound.storage_path);
    let result = store
        .get(&object_path)
        .await
        .map_err(|e| AppError::not_found(format!("Sound file not found in storage: {e}")))?;

    let stream = result.into_stream();
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    if let Ok(ct) = HeaderValue::from_str(&sound.content_type) {
        headers.insert(header::CONTENT_TYPE, ct);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    Ok((StatusCode::OK, headers, body))
}

pub async fn play_sound(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(sound_id): Path<Uuid>,
    Json(req): Json<PlaySoundRequest>,
) -> Result<StatusCode, AppError> {
    let user_id = auth_user.user_id();

    // Verify user is in the voice channel
    let channel_users = state
        .voice_states
        .get(&req.channel_id)
        .ok_or_else(|| AppError::bad_request("You are not in a voice channel"))?;
    let voice_state = channel_users
        .get(&user_id)
        .ok_or_else(|| AppError::bad_request("You are not in this voice channel"))?;
    if voice_state.is_deafened {
        return Err(AppError::bad_request("Cannot play sounds while deafened"));
    }
    drop(voice_state);
    drop(channel_users);

    // Check cooldown
    if let Some(last_played) = SOUND_COOLDOWNS.get(&sound_id)
        && last_played.elapsed().as_secs() < SOUND_COOLDOWN_SECS
    {
        return Err(AppError::bad_request("This sound is on cooldown"));
    }

    // Verify sound exists
    let sound =
        sqlx::query_as::<_, SoundboardSound>("SELECT * FROM soundboard_sounds WHERE id = $1")
            .bind(sound_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| AppError::internal(format!("Database error: {e}")))?
            .ok_or_else(|| AppError::not_found("Sound not found"))?;

    // Update cooldown
    SOUND_COOLDOWNS.insert(sound_id, Instant::now());

    // Broadcast play event to all connected clients
    state.broadcast_global(
        "soundboard_play",
        serde_json::json!({
            "channel_id": req.channel_id,
            "user_id": user_id,
            "sound_id": sound_id,
            "sound_volume": sound.volume,
        }),
    );

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_favorites(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<Uuid>>, AppError> {
    let user_id = auth_user.user_id();
    let favorites: Vec<(Uuid,)> =
        sqlx::query_as("SELECT sound_id FROM soundboard_favorites WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&state.db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to fetch favorites: {e}")))?;

    Ok(Json(favorites.into_iter().map(|(id,)| id).collect()))
}

pub async fn toggle_favorite(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(sound_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = auth_user.user_id();

    // Verify sound exists
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM soundboard_sounds WHERE id = $1")
        .bind(sound_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("Database error: {e}")))?;
    if exists.is_none() {
        return Err(AppError::not_found("Sound not found"));
    }

    // Check if already favorited
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM soundboard_favorites WHERE user_id = $1 AND sound_id = $2",
    )
    .bind(user_id)
    .bind(sound_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::internal(format!("Database error: {e}")))?;

    if existing.is_some() {
        sqlx::query("DELETE FROM soundboard_favorites WHERE user_id = $1 AND sound_id = $2")
            .bind(user_id)
            .bind(sound_id)
            .execute(&state.db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to remove favorite: {e}")))?;
        Ok(Json(serde_json::json!({ "favorited": false })))
    } else {
        sqlx::query("INSERT INTO soundboard_favorites (user_id, sound_id) VALUES ($1, $2)")
            .bind(user_id)
            .bind(sound_id)
            .execute(&state.db)
            .await
            .map_err(|e| AppError::internal(format!("Failed to add favorite: {e}")))?;
        Ok(Json(serde_json::json!({ "favorited": true })))
    }
}
