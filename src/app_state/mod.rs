use axum::extract::{FromRef, Path, State};

use sqlx::SqlitePool;

use crate::{
    handlers::reminders::send_reminder,
    messenger::Messenger,
    reminder_scheduler::{MedicationId, PatientId, ReminderScheduler},
    storage::Storage,
};

#[derive(Clone)]
pub struct AppState {
    pub storage: Storage,
    pub messenger: Messenger,
    pub reminder_scheduler: ReminderScheduler,
}

impl FromRef<AppState> for Messenger {
    fn from_ref(state: &AppState) -> Self {
        state.messenger.clone()
    }
}

impl FromRef<AppState> for ReminderScheduler {
    fn from_ref(state: &AppState) -> Self {
        state.reminder_scheduler.clone()
    }
}

impl FromRef<AppState> for Storage {
    fn from_ref(state: &AppState) -> Self {
        state.storage.clone()
    }
}

impl AppState {
    pub async fn new(db: SqlitePool, messenger: Messenger) -> anyhow::Result<Self> {
        let storage = Storage { pool: db };
        let callback = Self::reminder_callback(messenger.clone(), storage.clone());
        let reminder_scheduler = ReminderScheduler::new(callback).await?;

        Ok(AppState {
            storage,
            messenger,
            reminder_scheduler,
        })
    }

    fn reminder_callback(
        messenger: Messenger,
        storage: Storage,
    ) -> impl Fn(PatientId, MedicationId) {
        move |patient_id: PatientId, medication_id: MedicationId| {
            let storage = storage.clone();
            let messenger = messenger.clone();
            tokio::spawn(async move {
                if let Err(e) = send_reminder(
                    State(storage),
                    State(messenger),
                    Path((patient_id.0, medication_id.0)),
                )
                .await
                {
                    log::error!("Failed to send reminder: {e:?}");
                }
            });
        }
    }
}
