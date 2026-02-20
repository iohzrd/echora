use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

use crate::database;
use crate::shared::AppError;

/// Role levels, ordered by power (higher number = more power).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Member = 0,
    Moderator = 1,
    Admin = 2,
    Owner = 3,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner => f.write_str("owner"),
            Self::Admin => f.write_str("admin"),
            Self::Moderator => f.write_str("moderator"),
            Self::Member => f.write_str("member"),
        }
    }
}

impl std::str::FromStr for Role {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "moderator" => Ok(Self::Moderator),
            "member" => Ok(Self::Member),
            _ => Err(AppError::bad_request(format!("Invalid role: {s}"))),
        }
    }
}

/// Check that the user's role meets the minimum required level.
pub fn require_role(user_role: Role, minimum: Role) -> Result<Role, AppError> {
    if user_role >= minimum {
        Ok(user_role)
    } else {
        Err(AppError::forbidden(format!(
            "Requires {minimum} role or higher",
        )))
    }
}

/// Check that actor has a strictly higher role than target (for moderation actions).
pub fn require_higher_role(actor_role: Role, target_role: Role) -> Result<(), AppError> {
    if actor_role > target_role {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "Cannot moderate a user with equal or higher role",
        ))
    }
}

/// Check that actor can assign the given role (must be strictly above it, cannot assign owner).
pub fn can_assign_role(actor_role: Role, new_role: Role) -> Result<(), AppError> {
    if new_role == Role::Owner {
        return Err(AppError::forbidden("Cannot assign the owner role"));
    }
    if actor_role > new_role {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "Cannot assign a role equal to or above your own",
        ))
    }
}

/// Returns Err(Forbidden) if user is banned. For REST/auth routes.
pub async fn check_not_banned(db: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    if database::get_active_ban(db, user_id).await?.is_some() {
        return Err(AppError::forbidden("You are banned from this server"));
    }
    Ok(())
}

/// Returns Err(Forbidden) if user is muted. For REST routes.
pub async fn check_not_muted(db: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    if database::get_active_mute(db, user_id).await?.is_some() {
        return Err(AppError::forbidden("You are muted"));
    }
    Ok(())
}

/// Returns true if user is muted. Swallows DB errors. For WebSocket code.
pub async fn is_muted(db: &PgPool, user_id: Uuid) -> bool {
    database::get_active_mute(db, user_id)
        .await
        .ok()
        .flatten()
        .is_some()
}
