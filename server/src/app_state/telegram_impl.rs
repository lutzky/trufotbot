use axum::http::StatusCode;
use teloxide::prelude::*;

use crate::models::Patient;

use super::{AppState, SentMessageInfo};

pub(super) type MessageId = i32;

impl AppState {
    pub(super) fn telegram_prereqs(&self, patient: &Patient) -> Option<(ChatId, &Bot)> {
        let Some(telegram_group_id) = patient.telegram_group_id else {
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            return None;
        };

        let Some(bot) = &self.telegram_bot else {
            log::warn!("Telegram bot is not configured, skipping message.");
            return None;
        };

        Some((ChatId(telegram_group_id), bot))
    }

    #[allow(dead_code)]
    pub(super) async fn edit_message_telegram(
        &self,
        patient: &Patient,
        message_id: MessageId,
        new_message: String,
    ) -> Result<(), (StatusCode, String)> {
        let Some((chat_id, bot)) = self.telegram_prereqs(patient) else {
            return Ok(());
        };

        log::debug!(
            "Editing message {message_id} in {chat_id:?} \
                    to {new_message:?}"
        );

        bot.edit_message_text(
            chat_id,
            teloxide::types::MessageId(message_id.id()),
            new_message,
        )
        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
        .await
        .map_err(|e| {
            log::error!("Telegram error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to edit message".to_string(),
            )
        })?;

        Ok(())
    }

    #[allow(dead_code)]
    pub(super) async fn send_message_telegram(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<Message>, (StatusCode, String)> {
        let Some((chat_id, bot)) = self.telegram_prereqs(patient) else {
            return Ok(None);
        };

        log::debug!("Sending message in {chat_id}: {message}");

        let message = bot
            .send_message(chat_id, message)
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .await
            .map_err(|e| {
                log::error!("Telegram error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to send message".to_string(),
                )
            })?;

        Ok(Some(message))
    }
}
