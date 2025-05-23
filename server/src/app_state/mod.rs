use std::sync::Arc;

use axum::http::StatusCode;

use sqlx::SqlitePool;
pub use telegram::SentMessageInfo;
use tokio::sync::Mutex;
use tokio_cron_scheduler::JobScheduler;

use crate::models::{Medication, Patient};

mod fake_telegram;
pub mod telegram;
mod telegram_impl;

// TODO: Use
// https://docs.rs/axum/latest/axum/extract/struct.State.html#substates so that
// functions that only require the db can specify it that way

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    telegram_bot: Option<teloxide::Bot>,

    pub scheduler: Option<Arc<Mutex<JobScheduler>>>,

    #[cfg(test)]
    pub telegram_messages: fake_telegram::MessageHistory,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        telegram_bot: Option<teloxide::Bot>,
        scheduler: Option<JobScheduler>,
    ) -> Self {
        AppState {
            db,
            telegram_bot,
            scheduler: scheduler.map(|s| Arc::new(Mutex::new(s))),

            #[cfg(test)]
            telegram_messages: fake_telegram::MessageHistory::new(),
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
