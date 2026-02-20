use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::database;
use crate::models::{AppState, Message, ReplyPreview};
use crate::shared::AppError;
use crate::shared::validation::{MAX_ATTACHMENTS_PER_MESSAGE, validate_message_content_optional};

pub struct CreateMessageParams {
    pub user_id: Uuid,
    pub username: String,
    pub channel_id: Uuid,
    pub content: Option<String>,
    pub reply_to_id: Option<Uuid>,
    pub attachment_ids: Vec<Uuid>,
    /// If true, verifies the replied-to message is in the same channel (REST behavior).
    /// If false, skips this check (WS behavior).
    pub validate_reply_channel: bool,
}

pub struct CreateMessageResult {
    pub message: Message,
    pub channel_id: Uuid,
}

/// Validates content, resolves reply preview, persists the message, links
/// attachments, and spawns link preview fetch. Does NOT broadcast -- the
/// caller handles that because REST and WS have different broadcast semantics.
pub async fn create_message(
    state: &Arc<AppState>,
    db: &PgPool,
    params: CreateMessageParams,
) -> Result<CreateMessageResult, AppError> {
    let has_attachments = !params.attachment_ids.is_empty();
    validate_message_content_optional(&params.content, has_attachments)?;

    if params.attachment_ids.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(AppError::bad_request(format!(
            "Maximum {MAX_ATTACHMENTS_PER_MESSAGE} attachments per message"
        )));
    }

    let reply_to: Option<ReplyPreview> = if let Some(reply_id) = params.reply_to_id {
        if params.validate_reply_channel {
            let replied_msg = database::get_message_by_id(db, reply_id)
                .await?
                .ok_or_else(|| AppError::not_found("Replied-to message not found"))?;
            if replied_msg.channel_id != params.channel_id {
                return Err(AppError::bad_request(
                    "Cannot reply to a message in a different channel",
                ));
            }
        }
        database::get_reply_preview(db, reply_id).await?
    } else {
        None
    };

    let content = params.content.unwrap_or_default();

    let mut new_message = Message::new(
        content.clone(),
        params.username,
        params.user_id,
        params.channel_id,
        params.reply_to_id,
        reply_to,
    );

    database::create_message(db, &new_message, params.user_id).await?;

    if has_attachments {
        let attachments = database::link_attachments_to_message(
            db,
            &params.attachment_ids,
            new_message.id,
            params.user_id,
        )
        .await?;
        if !attachments.is_empty() {
            new_message.attachments = Some(attachments);
        }
    }

    if !content.is_empty() {
        crate::link_preview::spawn_preview_fetch(
            state.clone(),
            new_message.id,
            params.channel_id,
            content,
        );
    }

    Ok(CreateMessageResult {
        message: new_message,
        channel_id: params.channel_id,
    })
}
