use axum::extract::{FromRef, Path, State};

use sqlx::SqlitePool;

use crate::{
    handlers::reminders::send_reminder,
    messenger::Messenger,
    reminder_scheduler::{MedicationId, PatientId, ReminderScheduler},
    storage::Storage,
    telegram_bot::create_telegram_bot,
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
    pub async fn new(db: SqlitePool, telegram_bot: Option<teloxide::Bot>) -> anyhow::Result<Self> {
        let storage = Storage { pool: db };

        if let Some(bot) = &telegram_bot {
            let bot = bot.clone();
            let storage = storage.clone();
            tokio::spawn(async move { create_telegram_bot(bot, storage).await });
        }

        let messenger = Messenger::new(telegram_bot);

        let callback = {
            let messenger = messenger.clone();
            let storage = storage.clone();
            move |patient_id: PatientId, medication_id: MedicationId| {
                let messenger = messenger.clone();
                let storage = storage.clone();
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
        };

        Ok(AppState {
            storage,
            messenger,
            reminder_scheduler: ReminderScheduler::new(callback).await?,
        })
    }
}
