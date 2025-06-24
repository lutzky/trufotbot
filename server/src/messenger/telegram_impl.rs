use axum::http::StatusCode;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::models::Patient;

use super::{Messenger, SentMessageInfo, callbacks};

pub(super) type MessageId = i32;

impl Messenger {
    #[allow(dead_code)]
    pub(super) async fn edit_impl_telegram(
        &self,
        patient: &Patient,
        message_id: MessageId,
        new_message: String,
        new_keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<(), (StatusCode, String)> {
        let error_code = (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to edit message".to_string(),
        );

        let Some((chat_id, bot)) = self.prereqs(patient) else {
            return Ok(());
        };

        let keyboard = build_keyboard(new_keyboard).map_err(|e| {
            log::error!("Failed to build keyboard: {e}");
            error_code.clone()
        })?;

        log::debug!(
            "Editing message {message_id} in {chat_id:?} \
                    to {new_message:?} with keyboard {keyboard:?}"
        );

        bot.edit_message_text(
            chat_id,
            teloxide::types::MessageId(message_id.id()),
            new_message,
        )
        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
        .reply_markup(keyboard)
        .await
        .map_err(|e| {
            log::error!("Telegram error: {}", e);
            error_code.clone()
        })?;

        Ok(())
    }

    #[allow(dead_code)]
    pub(super) async fn send_impl_telegram(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<Message>, (StatusCode, String)> {
        let Some((chat_id, bot)) = self.prereqs(patient) else {
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

fn build_keyboard(
    new_keyboard: Vec<(String, callbacks::Action)>,
) -> Result<InlineKeyboardMarkup, Box<dyn std::error::Error>> {
    let buttons: Result<Vec<InlineKeyboardButton>, _> = new_keyboard
        .into_iter()
        .map(|(key, value)| {
            serde_json::to_string(&value)
                .map(|callback_json_data| InlineKeyboardButton::callback(key, callback_json_data))
        })
        .collect();

    let keyboard = InlineKeyboardMarkup::new(vec![buttons?]);

    Ok(keyboard)
}
