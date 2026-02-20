use crate::shared::AppError;

/// Role levels, ordered by power (higher number = more power).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Member = 0,
    Moderator = 1,
    Admin = 2,
    Owner = 3,
}

impl Role {
    pub fn from_str(s: &str) -> Self {
        match s {
            "owner" => Self::Owner,
            "admin" => Self::Admin,
            "moderator" => Self::Moderator,
            _ => Self::Member,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Moderator => "moderator",
            Self::Member => "member",
        }
    }
}

/// Check that the user's role meets the minimum required level.
pub fn require_role(user_role: &str, minimum: Role) -> Result<Role, AppError> {
    let role = Role::from_str(user_role);
    if role >= minimum {
        Ok(role)
    } else {
        Err(AppError::forbidden(format!(
            "Requires {} role or higher",
            minimum.as_str()
        )))
    }
}

/// Check that actor has a strictly higher role than target (for moderation actions).
pub fn require_higher_role(actor_role: &str, target_role: &str) -> Result<(), AppError> {
    let actor = Role::from_str(actor_role);
    let target = Role::from_str(target_role);
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
    let actor = Role::from_str(actor_role);
    let new_role = Role::from_str(target_new_role);

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
