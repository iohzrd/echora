use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::AppError;

use std::sync::OnceLock;

static JWT_SECRET: OnceLock<String> = OnceLock::new();

pub fn jwt_secret_str() -> &'static str {
    JWT_SECRET.get_or_init(|| std::env::var("JWT_SECRET").expect("JWT_SECRET must be set"))
}

pub fn jwt_secret() -> &'static [u8] {
    jwt_secret_str().as_bytes()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub username: String,
    pub role: String,
    pub exp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
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
    pub role: String,
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

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| AppError::authentication("Missing or invalid authorization header"))?;

        let token_data = decode::<Claims>(
            bearer.token(),
            &DecodingKey::from_secret(jwt_secret()),
            &Validation::default(),
        )
        .map_err(|_| AppError::authentication("Invalid token"))?;

        Ok(AuthUser(token_data.claims))
    }
}

pub fn create_jwt(user_id: Uuid, username: &str, role: &str) -> Result<String, AppError> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .ok_or_else(|| AppError::internal("Failed to compute token expiration"))?
        .timestamp();

    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        role: role.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret()),
    )
    .map_err(|e| AppError::internal(format!("Failed to create JWT: {}", e)))
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
