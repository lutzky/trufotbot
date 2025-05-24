use std::sync::Arc;

use axum::{extract::FromRef, http::StatusCode};

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::{
    messenger::Messenger,
    models::{Medication, Patient},
    reminder_scheduler::ReminderScheduler,
};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,

    // TODO: Wrap this in a messaging-bot object, move the telegram handlers
    // there, and then make that pub; this will allow us to make it available to
    // handlers using FromRef, and clarify dependencies.
    pub messenger: Messenger,

    pub reminder_scheduler: Arc<Mutex<ReminderScheduler>>,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}
impl FromRef<AppState> for Arc<Mutex<ReminderScheduler>> {
    fn from_ref(state: &AppState) -> Self {
        state.reminder_scheduler.clone()
    }
}

impl AppState {
    pub async fn new(db: SqlitePool, telegram_bot: Option<teloxide::Bot>) -> anyhow::Result<Self> {
        Ok(AppState {
            db,
            messenger: Messenger::new(telegram_bot),
            reminder_scheduler: Arc::new(Mutex::new(ReminderScheduler::new().await?)),
        })
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
