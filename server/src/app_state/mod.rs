use axum::http::StatusCode;
use teloxide::prelude::*;
use teloxide::types::{ChatId, Message};

use sqlx::SqlitePool;

use crate::models::Patient;

mod fake_telegram;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,                      // TODO: Shouldn't be public
    pub telegram_bot: Option<teloxide::Bot>, // TODO: Shouldn't be public

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
    ) -> Result<Option<impl MessageWithId>, (StatusCode, String)> {
        #[cfg(test)]
        {
            self.send_message_mock(patient, message).await
        }
        #[cfg(not(test))]
        {
            self.send_message_telegram(patient, message).await
        }
    }

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
    ) -> Result<Option<impl MessageWithId>, (StatusCode, String)> {
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
}

// TODO: Probably rename this to something clearer
pub trait MessageWithId {
    fn id(&self) -> i32;
}

impl MessageWithId for teloxide::types::Message {
    fn id(&self) -> i32 {
        self.id.0
    }
}

impl MessageWithId for i32 {
    fn id(&self) -> i32 {
        *self
    }
}
