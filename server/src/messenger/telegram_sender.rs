use anyhow::Result;
use async_trait::async_trait;
use std::{pin::Pin, sync::Arc};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

use super::{Sender, SentMessageInfo, callbacks};

pub(super) type MessageId = i32;

fn build_keyboard(new_keyboard: Vec<(String, callbacks::Action)>) -> Result<InlineKeyboardMarkup> {
    let buttons: Result<Vec<InlineKeyboardButton>, _> = new_keyboard
        .into_iter()
        .map(|(key, value)| {
            serde_json::to_string(&value)
                .map(|callback_json_data| InlineKeyboardButton::callback(key, callback_json_data))
        })
        .collect();

    let keyboard = InlineKeyboardMarkup::new(vec![buttons?]);

    log::debug!("Built inline keyboard markup: {keyboard:?}");

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
    ) -> Result<Option<Pin<Box<dyn SentMessageInfo + Send>>>> {
        let message = self
            .bot
            .send_message(chat_id, message)
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
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

        self.bot
            .edit_message_text(
                chat_id,
                teloxide::types::MessageId(message_id.id()),
                new_message,
            )
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .reply_markup(keyboard)
            .await?;

        Ok(())
    }
}

impl From<TelegramSender> for super::Messenger {
    fn from(value: TelegramSender) -> Self {
        Self::new_from_sender(Arc::new(value))
    }
}
