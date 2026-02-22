use crate::shared::AppError;

pub const MAX_MESSAGE_LENGTH: usize = 4000;
pub const MAX_CHANNEL_NAME_LENGTH: usize = 50;
pub const MIN_USERNAME_LENGTH: usize = 2;
pub const MAX_USERNAME_LENGTH: usize = 32;
pub const MAX_EMAIL_LENGTH: usize = 254;
pub const MIN_PASSWORD_LENGTH: usize = 8;
pub const MAX_PASSWORD_LENGTH: usize = 128;
pub const MAX_EMOJI_LENGTH: usize = 32;
pub const REPLY_PREVIEW_LENGTH: usize = 200;
pub const MAX_REASON_LENGTH: usize = 500;
pub const MAX_SERVER_NAME_LENGTH: usize = 100;
pub const MAX_IMAGE_PROXY_SIZE: usize = 10 * 1024 * 1024;
pub const BROADCAST_CHANNEL_CAPACITY: usize = 256;
pub const MAX_ATTACHMENT_SIZE: usize = 250 * 1024 * 1024; // 250MB
pub const MAX_CUSTOM_EMOJI_SIZE: usize = 256 * 1024; // 256KB
pub const MAX_CUSTOM_EMOJI_NAME_LENGTH: usize = 32;
pub const MAX_ATTACHMENTS_PER_MESSAGE: usize = 5;
pub const MAX_FILENAME_LENGTH: usize = 255;
pub const MESSAGE_RATE_LIMIT: f64 = 5.0;
pub const MESSAGE_RATE_REFILL_PER_SEC: f64 = 1.0;

pub const ALLOWED_CONTENT_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/svg+xml",
    "video/mp4",
    "video/webm",
    "audio/mpeg",
    "audio/ogg",
    "audio/wav",
    "audio/webm",
    "application/pdf",
    "text/plain",
    "application/zip",
    "application/gzip",
    "application/x-tar",
];

pub const ALLOWED_EMOJI_CONTENT_TYPES: &[&str] =
    &["image/png", "image/gif", "image/webp", "image/jpeg"];

pub const MAX_AVATAR_SIZE: usize = 2 * 1024 * 1024; // 2MB
pub const MAX_DISPLAY_NAME_LENGTH: usize = 64;
pub const ALLOWED_AVATAR_CONTENT_TYPES: &[&str] =
    &["image/png", "image/gif", "image/webp", "image/jpeg"];

pub fn validate_emoji_content_type(content_type: &str) -> Result<(), AppError> {
    if !ALLOWED_EMOJI_CONTENT_TYPES.contains(&content_type) {
        return Err(AppError::bad_request(format!(
            "Emoji image type '{}' is not allowed. Use PNG, GIF, WebP, or JPEG.",
            content_type
        )));
    }
    Ok(())
}

pub fn validate_emoji_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() || trimmed.len() > MAX_CUSTOM_EMOJI_NAME_LENGTH {
        return Err(AppError::bad_request(
            "Emoji name must be between 1 and 32 characters",
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::bad_request(
            "Emoji name can only contain ASCII letters, numbers, underscores, and hyphens",
        ));
    }
    Ok(trimmed)
}

pub fn validate_message_content(content: &str) -> Result<(), AppError> {
    if content.trim().is_empty() || content.len() > MAX_MESSAGE_LENGTH {
        return Err(AppError::bad_request(
            "Message must be between 1 and 4000 characters",
        ));
    }
    Ok(())
}

pub fn validate_message_content_optional(
    content: &Option<String>,
    has_attachments: bool,
) -> Result<(), AppError> {
    match content {
        Some(c) if !c.trim().is_empty() => validate_message_content(c),
        _ if has_attachments => Ok(()),
        _ => Err(AppError::bad_request(
            "Message must have content or attachments",
        )),
    }
}

pub fn validate_attachment_content_type(content_type: &str) -> Result<(), AppError> {
    if !ALLOWED_CONTENT_TYPES.contains(&content_type) {
        return Err(AppError::bad_request(format!(
            "File type '{}' is not allowed",
            content_type
        )));
    }
    Ok(())
}

pub fn validate_filename(name: &str) -> Result<String, AppError> {
    let sanitized: String = name
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        })
        .collect();
    let sanitized = sanitized.trim().to_string();
    if sanitized.is_empty() || sanitized.len() > MAX_FILENAME_LENGTH {
        return Err(AppError::bad_request("Invalid filename"));
    }
    Ok(sanitized)
}

pub fn validate_username(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim().to_string();
    if trimmed.len() < MIN_USERNAME_LENGTH || trimmed.len() > MAX_USERNAME_LENGTH {
        return Err(AppError::bad_request(
            "Username must be between 2 and 32 characters",
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::bad_request(
            "Username can only contain ASCII letters, numbers, underscores, and hyphens",
        ));
    }
    Ok(trimmed)
}

pub fn validate_email(email: &str) -> Result<String, AppError> {
    let email = email.trim().to_lowercase();
    if email.is_empty() || email.len() > MAX_EMAIL_LENGTH {
        return Err(AppError::bad_request("Invalid email address"));
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err(AppError::bad_request("Invalid email address"));
    };
    if local.is_empty()
        || local.contains(' ')
        || domain.len() < 3
        || domain.contains(' ')
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
        || domain.contains("..")
    {
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

pub fn validate_reason(reason: &Option<String>) -> Result<(), AppError> {
    if let Some(r) = reason
        && r.len() > MAX_REASON_LENGTH
    {
        return Err(AppError::bad_request(format!(
            "Reason must be at most {} characters",
            MAX_REASON_LENGTH
        )));
    }
    Ok(())
}

pub fn validate_positive_duration(hours: Option<i64>, field_name: &str) -> Result<(), AppError> {
    if let Some(h) = hours
        && h <= 0
    {
        return Err(AppError::bad_request(format!(
            "{field_name} must be a positive number"
        )));
    }
    Ok(())
}

pub fn validate_avatar_content_type(content_type: &str) -> Result<(), AppError> {
    if !ALLOWED_AVATAR_CONTENT_TYPES.contains(&content_type) {
        return Err(AppError::bad_request(format!(
            "Avatar image type '{}' is not allowed. Use PNG, GIF, WebP, or JPEG.",
            content_type
        )));
    }
    Ok(())
}

pub fn validate_display_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() || trimmed.len() > MAX_DISPLAY_NAME_LENGTH {
        return Err(AppError::bad_request(
            "Display name must be between 1 and 64 characters",
        ));
    }
    Ok(trimmed)
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
