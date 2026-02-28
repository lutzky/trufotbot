use anyhow::Result;
use async_trait::async_trait;
use std::{pin::Pin, sync::Arc};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use super::{MessageId, Sender, SentMessageInfo, callbacks};

fn maybe_warn_about_long_callback(json_data: &str) {
    log::trace!(
        "Generating callback data from this JSON with length {}: {json_data}",
        json_data.len()
    );
    if json_data.len() > 64 {
        log::error!(
            "Generating callback data from this JSON with length {} > 64, which will fail: {json_data}",
            json_data.len()
        );
    }
}

fn build_keyboard(new_keyboard: Vec<(String, callbacks::Action)>) -> Result<InlineKeyboardMarkup> {
    let buttons: Result<Vec<InlineKeyboardButton>, _> = new_keyboard
        .into_iter()
        .map(|(key, action)| match action {
            callbacks::Action::Link { url } => Ok(InlineKeyboardButton::url(key, url)),
            any_other_callback => {
                serde_json::to_string(&any_other_callback).map(|callback_json_data| {
                    maybe_warn_about_long_callback(&callback_json_data);
                    InlineKeyboardButton::callback(key, callback_json_data)
                })
            }
        })
        .collect();

    let keyboard = InlineKeyboardMarkup::new(vec![buttons?]);

    log::debug!("Built inline keyboard markup: {keyboard:#?}");

    Ok(keyboard)
}

/// Real [`Sender`] that uses telegram
pub struct TelegramSender {
    pub bot: Bot,
}

impl TelegramSender {
    pub fn new(bot: Bot) -> Self {
        Self { bot }
    }
}

#[async_trait]
impl Sender for TelegramSender {
    async fn send(
        &self,
        chat_id: ChatId,
        message: String,
        keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<Option<Pin<Box<dyn SentMessageInfo + Send>>>> {
        let keyboard = build_keyboard(keyboard)?;

        let message = self
            .bot
            .send_message(chat_id, message)
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .reply_markup(keyboard)
            .await?;

        Ok(Some(Box::pin(message)))
    }

    async fn edit(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        new_message: String,
        new_keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<()> {
        let keyboard = build_keyboard(new_keyboard)?;
        log::debug!("Editing message {message_id} so it has this keyboard: {keyboard:#?}");

        self.bot
            .edit_message_text(
                chat_id,
                teloxide::types::MessageId(message_id.id()),
                new_message,
            )
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .reply_markup(keyboard)
            .await?;

        log::debug!("Message {message_id} edited successfully");

        Ok(())
    }

    async fn delete(&self, chat_id: ChatId, message_id: MessageId) -> Result<()> {
        self.bot
            .delete_message(chat_id, teloxide::types::MessageId(message_id.id()))
            .await?;

        Ok(())
    }
}

impl From<TelegramSender> for super::Messenger {
    fn from(value: TelegramSender) -> Self {
        Self::new_from_sender(Arc::new(value))
    }
}
