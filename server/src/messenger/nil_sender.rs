use anyhow::Result;
use async_trait::async_trait;
use std::{pin::Pin, sync::Arc};
use teloxide::types::ChatId;

use super::{MessageId, Sender, SentMessageInfo, callbacks};

/// Fake [`Sender`] that does nothing; for cases where a telegram bot is not configured
pub struct NilSender {}

impl NilSender {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Sender for NilSender {
    async fn send(
        &self,
        chat_id: ChatId,
        message: String,
    ) -> Result<Option<Pin<Box<dyn SentMessageInfo + Send>>>> {
        log::warn!("DoNothingSenderEditer::send({chat_id:?}, {message:?})");
        Ok(None)
    }

    async fn edit(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        new_message: String,
        new_keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<()> {
        log::warn!(
            "DoNothingSenderEditer::edit({chat_id:?}, {message_id:?}, {new_message:?}, {new_keyboard:?})"
        );
        Ok(())
    }
}

impl From<NilSender> for super::Messenger {
    fn from(value: NilSender) -> Self {
        Self::new_from_sender(Arc::new(value))
    }
}
