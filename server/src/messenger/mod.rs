use crate::models::Patient;
use axum::http::StatusCode;
use telegram_impl::MessageId;
use teloxide::{Bot, types::ChatId};

mod fake_telegram;
mod telegram_impl;

pub trait SentMessageInfo {
    fn id(&self) -> MessageId;
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
    telegram_bot: Option<teloxide::Bot>,

    #[cfg(test)]
    pub telegram_messages: fake_telegram::MessageHistory,
}

impl Messenger {
    pub fn new(telegram_bot: Option<teloxide::Bot>) -> Self {
        Messenger {
            telegram_bot,
            #[cfg(test)]
            telegram_messages: fake_telegram::MessageHistory::default(),
        }
    }

    // TODO: Rename to messenger_prereqs
    fn telegram_prereqs(&self, patient: &Patient) -> Option<(ChatId, &Bot)> {
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

    pub async fn send_message(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<impl SentMessageInfo>, (StatusCode, String)> {
        #[cfg(test)]
        {
            self.send_message_mock(patient, message).await
        }
        #[cfg(not(test))]
        {
            self.send_message_telegram(patient, message).await
        }
    }

    pub async fn edit_message(
        &self,
        patient: &Patient,
        message_id: MessageId,
        new_message: String,
    ) -> Result<(), (StatusCode, String)> {
        #[cfg(test)]
        {
            self.edit_message_mock(patient, message_id, new_message)
                .await
        }
        #[cfg(not(test))]
        {
            self.edit_message_telegram(patient, message_id, new_message)
                .await
        }
    }
}
