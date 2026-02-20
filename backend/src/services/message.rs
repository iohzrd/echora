use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::database;
use crate::models::{AppState, Message, ReplyPreview};
use crate::shared::AppError;
use crate::shared::validation::validate_message_content;

pub struct CreateMessageParams {
    pub user_id: Uuid,
    pub username: String,
    pub channel_id: Uuid,
    pub content: String,
    pub reply_to_id: Option<Uuid>,
    /// If true, verifies the replied-to message is in the same channel (REST behavior).
    /// If false, skips this check (WS behavior).
    pub validate_reply_channel: bool,
}

pub struct CreateMessageResult {
    pub message: Message,
    pub channel_id: Uuid,
}

/// Validates content, resolves reply preview, persists the message, and spawns
/// link preview fetch. Does NOT broadcast -- the caller handles that because
/// REST and WS have different broadcast semantics.
pub async fn create_message(
    state: &Arc<AppState>,
    db: &PgPool,
    params: CreateMessageParams,
) -> Result<CreateMessageResult, AppError> {
    validate_message_content(&params.content)?;

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

    let new_message = Message::new(
        params.content,
        params.username,
        params.user_id,
        params.channel_id,
        params.reply_to_id,
        reply_to,
    );

    database::create_message(db, &new_message, params.user_id).await?;

    crate::link_preview::spawn_preview_fetch(
        state.clone(),
        new_message.id,
        params.channel_id,
        new_message.content.clone(),
    );

    Ok(CreateMessageResult {
        message: new_message,
        channel_id: params.channel_id,
    })
}
