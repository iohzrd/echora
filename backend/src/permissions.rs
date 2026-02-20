use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

use crate::database;
use crate::shared::AppError;

/// Role levels, ordered by power (higher number = more power).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "owner" => Self::Owner,
            "admin" => Self::Admin,
            "moderator" => Self::Moderator,
            _ => Self::Member,
        })
    }
}

/// Check that the user's role meets the minimum required level.
pub fn require_role(user_role: &str, minimum: Role) -> Result<Role, AppError> {
    let role: Role = user_role.parse().unwrap();
    if role >= minimum {
        Ok(role)
    } else {
        Err(AppError::forbidden(format!(
            "Requires {minimum} role or higher",
        )))
    }
}

/// Check that actor has a strictly higher role than target (for moderation actions).
pub fn require_higher_role(actor_role: &str, target_role: &str) -> Result<(), AppError> {
    let actor: Role = actor_role.parse().unwrap();
    let target: Role = target_role.parse().unwrap();
    if actor > target {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "Cannot moderate a user with equal or higher role",
        ))
    }
}

/// Check that actor can assign the given role (must be strictly above it, cannot assign owner).
pub fn can_assign_role(actor_role: &str, target_new_role: &str) -> Result<(), AppError> {
    let actor: Role = actor_role.parse().unwrap();
    let new_role: Role = target_new_role.parse().unwrap();

    if new_role == Role::Owner {
        return Err(AppError::forbidden("Cannot assign the owner role"));
    }
    if actor > new_role {
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
