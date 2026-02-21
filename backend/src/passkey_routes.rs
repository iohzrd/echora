use axum::{extract::Path, extract::State, response::Json};
use base64::engine::{Engine, general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::auth::{AuthResponse, AuthUser, UserInfo, create_jwt};
use crate::database;
use crate::models::AppState;
use crate::permissions;
use crate::shared::{AppError, AppResult};

// --- Registration (authenticated) ---

pub async fn start_passkey_register(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let user_id = auth_user.user_id();
    let username = &auth_user.0.username;

    let existing = database::get_user_passkeys(&state.db, user_id).await?;
    let exclude_creds: Vec<CredentialID> = existing
        .iter()
        .map(|(_, _, pk, _, _)| pk.cred_id().clone())
        .collect();
    let exclude = if exclude_creds.is_empty() {
        None
    } else {
        Some(exclude_creds)
    };

    let (ccr, reg_state) = state
        .webauthn
        .start_passkey_registration(user_id, username, username, exclude)
        .map_err(|e| AppError::internal(format!("WebAuthn registration start failed: {e}")))?;

    state
        .webauthn_reg_state
        .insert(user_id, (reg_state, std::time::Instant::now()));

    serde_json::to_value(&ccr)
        .map(Json)
        .map_err(|e| AppError::internal(format!("Failed to serialize challenge: {e}")))
}

#[derive(Debug, Deserialize)]
pub struct FinishRegisterRequest {
    pub credential: serde_json::Value,
    pub name: Option<String>,
}

pub async fn finish_passkey_register(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<FinishRegisterRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let user_id = auth_user.user_id();

    let (_, (reg_state, _)) = state
        .webauthn_reg_state
        .remove(&user_id)
        .ok_or_else(|| AppError::bad_request("No pending passkey registration found"))?;

    let reg: RegisterPublicKeyCredential = serde_json::from_value(payload.credential)
        .map_err(|e| AppError::bad_request(format!("Invalid registration credential: {e}")))?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&reg, &reg_state)
        .map_err(|e| AppError::bad_request(format!("Passkey registration failed: {e}")))?;

    let credential_name = payload
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Passkey".to_string());

    let passkey_id = Uuid::now_v7();
    let cred_id_b64 = URL_SAFE_NO_PAD.encode(passkey.cred_id().as_ref());

    database::create_user_passkey(
        &state.db,
        passkey_id,
        user_id,
        &credential_name,
        &cred_id_b64,
        &passkey,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "id": passkey_id,
        "name": credential_name,
        "created_at": Utc::now(),
    })))
}

// --- Authentication (unauthenticated) ---

#[derive(Debug, Deserialize)]
pub struct StartAuthRequest {
    pub username: Option<String>,
}

pub async fn start_passkey_auth(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StartAuthRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let (user_id, passkeys) = if let Some(ref username) = payload.username {
        let user = database::get_user_by_username(&state.db, username)
            .await?
            .ok_or_else(|| AppError::authentication("Invalid credentials"))?;
        let pks = database::get_user_passkeys(&state.db, user.id).await?;
        if pks.is_empty() {
            return Err(AppError::authentication(
                "No passkeys registered for this user",
            ));
        }
        let passkey_list: Vec<Passkey> = pks.into_iter().map(|(_, _, pk, _, _)| pk).collect();
        (user.id, passkey_list)
    } else {
        (Uuid::nil(), vec![])
    };

    let (rcr, auth_state) = state
        .webauthn
        .start_passkey_authentication(passkeys.as_slice())
        .map_err(|e| AppError::internal(format!("WebAuthn auth start failed: {e}")))?;

    let challenge_key = Uuid::now_v7().to_string();
    state.webauthn_auth_state.insert(
        challenge_key.clone(),
        (user_id, auth_state, std::time::Instant::now()),
    );

    let mut response = serde_json::to_value(&rcr)
        .map_err(|e| AppError::internal(format!("Failed to serialize challenge: {e}")))?;

    if let Some(obj) = response.as_object_mut() {
        obj.insert(
            "challenge_key".to_string(),
            serde_json::json!(challenge_key),
        );
    }

    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
pub struct FinishAuthRequest {
    pub credential: serde_json::Value,
    pub challenge_key: String,
}

pub async fn finish_passkey_auth(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FinishAuthRequest>,
) -> AppResult<Json<AuthResponse>> {
    let (_, (user_id, auth_state, _)) = state
        .webauthn_auth_state
        .remove(&payload.challenge_key)
        .ok_or_else(|| AppError::bad_request("No pending passkey authentication found"))?;

    let cred: PublicKeyCredential = serde_json::from_value(payload.credential)
        .map_err(|e| AppError::bad_request(format!("Invalid authentication credential: {e}")))?;

    let auth_result = state
        .webauthn
        .finish_passkey_authentication(&cred, &auth_state)
        .map_err(|_| AppError::authentication("Passkey authentication failed"))?;

    // For discoverable credentials, resolve user_id from userHandle
    let actual_user_id = if user_id.is_nil() {
        let user_handle = cred
            .get_user_unique_id()
            .ok_or_else(|| AppError::authentication("Missing user handle in credential"))?;
        Uuid::from_slice(user_handle)
            .map_err(|_| AppError::authentication("Invalid user handle"))?
    } else {
        user_id
    };

    permissions::check_not_banned(&state.db, actual_user_id).await?;

    let user = database::get_user_by_id(&state.db, actual_user_id)
        .await?
        .ok_or_else(|| AppError::authentication("User not found"))?;

    // Update passkey counter
    let passkeys = database::get_user_passkeys(&state.db, actual_user_id).await?;
    for (_, _, mut pk, _, _) in passkeys {
        if pk.update_credential(&auth_result).is_some() {
            let cred_id_b64 = URL_SAFE_NO_PAD.encode(pk.cred_id().as_ref());
            database::update_user_passkey(&state.db, actual_user_id, &cred_id_b64, &pk).await?;
            break;
        }
    }

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

// --- Management (authenticated) ---

#[derive(Debug, Serialize)]
pub struct PasskeyInfo {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

pub async fn list_passkeys(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> AppResult<Json<Vec<PasskeyInfo>>> {
    let passkeys = database::get_user_passkeys(&state.db, auth_user.user_id()).await?;
    let infos: Vec<PasskeyInfo> = passkeys
        .into_iter()
        .map(|(id, name, _, created, last_used)| PasskeyInfo {
            id,
            name,
            created_at: created,
            last_used_at: last_used,
        })
        .collect();
    Ok(Json(infos))
}

pub async fn delete_passkey(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(passkey_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    database::delete_user_passkey(&state.db, passkey_id, auth_user.user_id()).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
