use std::{pin::Pin, sync::Arc};

use crate::{errors::ServiceError, models::Patient};
use anyhow::Result;
use async_trait::async_trait;
use telegram_sender::MessageId;
use teloxide::types::ChatId;

pub mod callbacks;
pub mod fake_sender;
pub mod nil_sender;
pub mod telegram_sender;

pub trait SentMessageInfo {
    fn id(&self) -> MessageId;
}

#[async_trait]
pub trait Sender: Send + Sync {
    async fn send(
        &self,
        chat_id: ChatId,
        message: String,
    ) -> Result<Option<Pin<Box<dyn SentMessageInfo + Send>>>>;

    async fn edit(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        new_message: String,
        new_keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<()>;
}

impl SentMessageInfo for teloxide::types::Message {
    fn id(&self) -> MessageId {
        self.id.0
    }
}

impl SentMessageInfo for MessageId {
    fn id(&self) -> MessageId {
        *self
    }
}

#[derive(Clone)]
pub struct Messenger {
    sender: Arc<dyn Sender>,
}

/// Initialize a Messenger by creating an implementation of [`Sender`] and using `.into()`.
impl Messenger {
    fn new_from_sender(sender: Arc<dyn Sender>) -> Self {
        Messenger { sender }
    }

    fn get_chat_id_or_warn(patient: &Patient) -> Option<ChatId> {
        if let Some(chat_id) = patient.telegram_group_id {
            Some(ChatId(chat_id))
        } else {
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            None
        }
    }

    pub async fn send(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<Pin<Box<dyn SentMessageInfo + Send>>>, ServiceError> {
        let Some(chat_id) = Self::get_chat_id_or_warn(patient) else {
            return Ok(None);
        };
        log::debug!("Sending message in {chat_id}: {message}");
        self.sender
            .send(chat_id, message)
            .await
            .map_err(|e| ServiceError::InternalError(e.context("Telegram error sending message")))
    }

    pub async fn edit(
        &self,
        patient: &Patient,
        message_id: MessageId,
        new_message: String,
        new_keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<(), ServiceError> {
        let Some(chat_id) = Self::get_chat_id_or_warn(patient) else {
            return Ok(());
        };

        log::debug!(
            "Editing message {message_id} in {chat_id:?} \
                    to {new_message:?} with keyboard {new_keyboard:?}"
        );

        self.sender
            .edit(chat_id, message_id, new_message, new_keyboard)
            .await?;

        Ok(())
    }
}
