use axum::http::StatusCode;
use std::{collections::HashMap, sync::Arc};
use teloxide::prelude::*;
use teloxide::types::{ChatId, Message};
use tokio::sync::Mutex;

use sqlx::SqlitePool;

use crate::models::Patient;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,                      // TODO: Shouldn't be public
    pub telegram_bot: Option<teloxide::Bot>, // TODO: Shouldn't be public

    #[cfg(test)]
    pub telegram_messages: Arc<Mutex<HashMap<i64, Vec<(i32, String)>>>>, // TODO: Shouldn't be public
}

impl AppState {
    pub fn new(db: SqlitePool, telegram_bot: Option<teloxide::Bot>) -> Self {
        AppState {
            db,
            telegram_bot,

            #[cfg(test)]
            telegram_messages: Arc::new(Mutex::new(HashMap::new())),
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
        let mut telegram_messages = self.telegram_messages.lock().await;

        let messages = telegram_messages
            .entry(telegram_group_id)
            .or_insert_with(Vec::new);

        let id = messages.len() as i32 + 1;

        messages.push((id, message.clone()));

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
