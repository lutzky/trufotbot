use axum::http::StatusCode;
use teloxide::prelude::*;
use teloxide::types::{ChatId, Message};

use sqlx::SqlitePool;

use crate::models::{Medication, Patient};

mod fake_telegram;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool, // TODO: Shouldn't be public
    telegram_bot: Option<teloxide::Bot>,

    #[cfg(test)]
    pub telegram_messages: fake_telegram::MessageHistory,
}

impl AppState {
    pub fn new(db: SqlitePool, telegram_bot: Option<teloxide::Bot>) -> Self {
        AppState {
            db,
            telegram_bot,

            #[cfg(test)]
            telegram_messages: fake_telegram::MessageHistory::new(),
        }
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

    #[allow(dead_code)]
    async fn send_message_telegram(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<Message>, (StatusCode, String)> {
        let Some(telegram_group_id) = patient.telegram_group_id else {
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            return Ok(None);
        };

        let Some(bot) = &self.telegram_bot else {
            log::warn!("Telegram bot is not configured, skipping message.");
            return Ok(None);
        };

        log::debug!("Sending message in {telegram_group_id}: {message}");

        let message = bot
            .send_message(ChatId(telegram_group_id), message)
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

    #[cfg(test)]
    async fn send_message_mock(
        &self,
        patient: &Patient,
        message: String,
    ) -> Result<Option<impl SentMessageInfo>, (StatusCode, String)> {
        let Some(telegram_group_id) = patient.telegram_group_id else {
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            return Ok(None);
        };

        let id = self
            .telegram_messages
            .add_message(telegram_group_id, message.clone())
            .await;

        Ok(Some(id))
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

    #[cfg(test)]
    pub async fn edit_message_mock(
        &self,
        patient: &Patient,
        message_id: MessageId,
        new_message: String,
    ) -> Result<(), (StatusCode, String)> {
        let Some(telegram_group_id) = patient.telegram_group_id else {
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            return Ok(());
        };

        self.telegram_messages
            .replace_message(telegram_group_id, message_id, new_message)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to replace message".to_string(),
                )
            })?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn edit_message_telegram(
        &self,
        patient: &Patient,
        message_id: MessageId,
        new_message: String,
    ) -> Result<(), (StatusCode, String)> {
        let Some(telegram_group_id) = patient.telegram_group_id else {
            // TODO can we deduplicate these?
            log::warn!(
                "Patient {} has no telegram group ID, skipping message.",
                patient.name
            );
            return Ok(());
        };

        let Some(bot) = &self.telegram_bot else {
            log::warn!("Telegram bot is not configured, skipping message.");
            return Ok(());
        };

        log::debug!(
            "Editing message {message_id} in {telegram_group_id} \
                    to {new_message:?}"
        );

        bot.edit_message_text(
            ChatId(telegram_group_id),
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

    pub async fn get_patient(&self, patient_id: i64) -> Result<Patient, (StatusCode, String)> {
        Patient::get(&self.db, patient_id)
            .await
            .map_err(|e| {
                log::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to fetch patient".to_string(),
                )
            })?
            .ok_or((StatusCode::NOT_FOUND, "Patient not found".to_string()))
    }

    pub async fn get_medication(
        &self,
        medication_id: i64,
    ) -> Result<Medication, (StatusCode, String)> {
        Medication::get(&self.db, medication_id)
            .await
            .map_err(|e| {
                log::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to fetch patient".to_string(),
                )
            })?
            .ok_or((StatusCode::NOT_FOUND, "Medication not found".to_string()))
    }
}

type MessageId = i32;

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
