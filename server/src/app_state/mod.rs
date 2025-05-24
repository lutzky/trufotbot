use anyhow::bail;
use axum::{extract::FromRef, http::StatusCode};

use sqlx::SqlitePool;

use crate::{
    messenger::{Messenger, SentMessageInfo},
    models::{Medication, Patient},
    reminder_scheduler::{MedicationId, PatientId, ReminderScheduler},
};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub messenger: Messenger,
    pub reminder_scheduler: ReminderScheduler,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}
impl FromRef<AppState> for ReminderScheduler {
    fn from_ref(state: &AppState) -> Self {
        state.reminder_scheduler.clone()
    }
}

async fn send_reminder_callback_hacky_version(
    db: SqlitePool,
    messenger: Messenger,
    patient_id: PatientId,
    medication_id: MedicationId,
) -> anyhow::Result<()> {
    // TODO: Non-hacky version
    let patient = match Patient::get(&db, patient_id.0).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            bail!("Patient not found {patient_id:?}");
        }
        Err(e) => {
            bail!("Database error: {}", e);
        }
    };
    let medication = match Medication::get(&db, medication_id.0).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            bail!("Medication not found {medication_id:?}");
        }
        Err(e) => {
            bail!("Database error: {}", e);
        }
    };

    let base_message =
        teloxide::utils::markdown::escape(&format!("Time to take {}.", medication.name));

    let message_id = messenger
        .send_message(&patient, base_message.clone())
        .await
        .unwrap()
        .unwrap();

    messenger
        .edit_message(
            &patient,
            message_id.id(),
            format!(
                "{base_message} {}",
                teloxide::utils::markdown::link("http://example.com", "Take")
            ),
        )
        .await
        .unwrap();

    Ok(())

    // Here you would call the actual reminder sending logic
    // crate::handlers::reminders::send_reminder(...)
}

impl AppState {
    pub async fn new(db: SqlitePool, telegram_bot: Option<teloxide::Bot>) -> anyhow::Result<Self> {
        let messenger = Messenger::new(telegram_bot);
        let callback = {
            let messenger = messenger.clone();
            let db = db.clone();
            move |patient_id: PatientId, medication_id: MedicationId| {
                let messenger = messenger.clone();
                let db = db.clone();
                tokio::spawn(async move {
                    if let Err(e) = send_reminder_callback_hacky_version(
                        db,
                        messenger,
                        patient_id,
                        medication_id,
                    )
                    .await
                    {
                        log::error!("Failed to send reminder: {}", e);
                    }
                });
                // let patient = db.get
                // OK, what we need to do is move get_patient and get_medication
                // into a Model module, so we can refer to it at this point. MAYBE! Actually, maybe Patient::get is good enough. But first, let's try doing this the ugly hacky way:
                // let patient = Patient::get(&db, patient_id.0)
                //     .await
                //     .map_err(|e| {
                //         log::error!("Database error: {}", e);
                //         (
                //             StatusCode::INTERNAL_SERVER_ERROR,
                //             "Failed to fetch patient".to_string(),
                //         )
                //     })?
                //     .ok_or((StatusCode::NOT_FOUND, "Patient not found".to_string()))
                //         log::warn!("Reminder callback for {patient_id:?}, {medication_id:?}");
                //     }
            }
        };

        Ok(AppState {
            db,
            messenger,
            reminder_scheduler: ReminderScheduler::new(callback).await?,
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
