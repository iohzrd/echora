use crate::shared::AppError;

pub const MAX_MESSAGE_LENGTH: usize = 4000;
pub const MAX_CHANNEL_NAME_LENGTH: usize = 50;
pub const MAX_USERNAME_LENGTH: usize = 32;
pub const MAX_EMAIL_LENGTH: usize = 254;
pub const MIN_PASSWORD_LENGTH: usize = 8;
pub const MAX_PASSWORD_LENGTH: usize = 128;
pub const MAX_EMOJI_LENGTH: usize = 32;
pub const REPLY_PREVIEW_LENGTH: usize = 200;
pub const MAX_IMAGE_PROXY_SIZE: usize = 10 * 1024 * 1024;
pub const BROADCAST_CHANNEL_CAPACITY: usize = 256;

pub fn validate_message_content(content: &str) -> Result<(), AppError> {
    if content.trim().is_empty() || content.len() > MAX_MESSAGE_LENGTH {
        return Err(AppError::bad_request(
            "Message must be between 1 and 4000 characters",
        ));
    }
    Ok(())
}

pub fn validate_username(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() || trimmed.len() > MAX_USERNAME_LENGTH {
        return Err(AppError::bad_request(
            "Username must be between 1 and 32 characters",
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::bad_request(
            "Username can only contain letters, numbers, underscores, and hyphens",
        ));
    }
    Ok(trimmed)
}

pub fn validate_email(email: &str) -> Result<String, AppError> {
    let email = email.trim().to_lowercase();
    if email.is_empty() || email.len() > MAX_EMAIL_LENGTH {
        return Err(AppError::bad_request("Invalid email address"));
    }
    if !email.contains('@') || !email.contains('.') {
        return Err(AppError::bad_request("Invalid email address"));
    }
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].len() < 3 {
        return Err(AppError::bad_request("Invalid email address"));
    }
    Ok(email)
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(AppError::bad_request(
            "Password must be at least 8 characters",
        ));
    }
    if password.len() > MAX_PASSWORD_LENGTH {
        return Err(AppError::bad_request(
            "Password must be at most 128 characters",
        ));
    }
    Ok(())
}

pub fn validate_channel_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() || trimmed.len() > MAX_CHANNEL_NAME_LENGTH {
        return Err(AppError::bad_request(
            "Channel name must be between 1 and 50 characters",
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ' ')
    {
        return Err(AppError::bad_request(
            "Channel name can only contain letters, numbers, hyphens, underscores, and spaces",
        ));
    }
    Ok(trimmed)
}
