use axum::{extract::FromRequestParts, http::request::Parts};
use chrono::{TimeDelta, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::permissions::Role;
use crate::shared::AppError;

use std::sync::OnceLock;

static JWT_SECRET: OnceLock<String> = OnceLock::new();

pub fn jwt_secret_str() -> &'static str {
    JWT_SECRET.get_or_init(|| std::env::var("JWT_SECRET").expect("JWT_SECRET must be set"))
}

pub fn jwt_secret() -> &'static [u8] {
    jwt_secret_str().as_bytes()
}

static HMAC_SECRET: OnceLock<String> = OnceLock::new();

pub fn hmac_secret() -> &'static str {
    HMAC_SECRET.get_or_init(|| {
        std::env::var("HMAC_SECRET").unwrap_or_else(|_| {
            tracing::warn!(
                "HMAC_SECRET not set, falling back to JWT_SECRET. Set HMAC_SECRET for proper key separation."
            );
            jwt_secret_str().to_string()
        })
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub username: String,
    pub role: Role,
    pub exp: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: Role,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub invite_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: Role,
}

pub struct AuthUser(pub Claims);

impl AuthUser {
    pub fn user_id(&self) -> Uuid {
        self.0.sub
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::authentication("Missing authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::authentication("Invalid authorization header format"))?;

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(jwt_secret()),
            &Validation::default(),
        )
        .map_err(|_| AppError::authentication("Invalid token"))?;

        Ok(AuthUser(token_data.claims))
    }
}

pub fn create_jwt(user_id: Uuid, username: &str, role: Role) -> Result<String, AppError> {
    let expiration = Utc::now()
        .checked_add_signed(TimeDelta::days(7))
        .ok_or_else(|| AppError::internal("Failed to compute token expiration"))?
        .timestamp();

    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        role,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret()),
    )
    .map_err(|e| AppError::internal(format!("Failed to create JWT: {e}")))
}

pub fn decode_jwt(token: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret()),
        &Validation::default(),
    )
    .map_err(|_| AppError::authentication("Invalid token"))?;

    Ok(token_data.claims)
}
