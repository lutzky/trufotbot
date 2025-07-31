use std::{pin::Pin, sync::Arc};

use crate::{errors::ServiceError, models::Patient};
use anyhow::Result;
use async_trait::async_trait;
use teloxide::types::ChatId;

pub mod callbacks;
pub mod fake_sender;
pub mod nil_sender;
pub mod telegram_sender;

pub trait SentMessageInfo {
    fn id(&self) -> MessageId;
}

pub type MessageId = i32;

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
        override_chat_id: Option<ChatId>,
        message_id: MessageId,
        new_message: String,
        new_keyboard: Vec<(String, callbacks::Action)>,
    ) -> Result<(), ServiceError> {
        let patient_chat_id = Self::get_chat_id_or_warn(patient);

        let chat_id = match (patient_chat_id, override_chat_id) {
            (None, None) => return Ok(()),
            (Some(patient_chat_id), None) => patient_chat_id,
            (Some(patient_chat_id), Some(override_chat_id))
                if patient_chat_id == override_chat_id =>
            {
                patient_chat_id
            }
            (maybe_patient_chat_id, Some(override_chat_id)) => {
                log::warn!(
                    "Patient now has chat ID {maybe_patient_chat_id:?}, but we're editing message \
                    {message_id} in overridden chat ID {override_chat_id}."
                );
                override_chat_id
            }
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
