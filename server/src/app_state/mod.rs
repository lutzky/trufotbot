use std::sync::Arc;

use axum::{extract::FromRef, http::StatusCode};

use sqlx::SqlitePool;
pub use telegram::SentMessageInfo;
use tokio::sync::Mutex;

use crate::{
    models::{Medication, Patient},
    reminder_scheduler::ReminderScheduler,
};

mod fake_telegram;
pub mod telegram;
mod telegram_impl;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,

    // TODO: Wrap this in a messaging-bot object, move the telegram handlers
    // there, and then make that pub; this will allow us to make it available to
    // handlers using FromRef, and clarify dependencies.
    telegram_bot: Option<teloxide::Bot>,

    // TODO: This should not be pub if we're going to manage it internally;
    // possibly make a pub ScheduleManager or something so you can FromRef that specifically.
    pub scheduler: Option<Arc<Mutex<ReminderScheduler>>>,

    #[cfg(test)]
    pub telegram_messages: fake_telegram::MessageHistory,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}
impl FromRef<AppState> for Option<Arc<Mutex<ReminderScheduler>>> {
    fn from_ref(state: &AppState) -> Self {
        state.scheduler.clone()
    }
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        telegram_bot: Option<teloxide::Bot>,
        scheduler: Option<ReminderScheduler>,
    ) -> Self {
        AppState {
            db,
            telegram_bot,
            scheduler: scheduler.map(|s| Arc::new(Mutex::new(s))),

            #[cfg(test)]
            telegram_messages: Default::default(),
        }
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
