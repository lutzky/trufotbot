use std::sync::Arc;

use anyhow::Context;
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
    pub config: std::sync::Arc<Config>,
    pub storage: Storage,
    pub messenger: Messenger,
    pub reminder_scheduler: ReminderScheduler,
}

#[derive(Debug, serde::Deserialize)]
pub struct Config {
    // TODO: Find usages of std::env::var around the codebase, move them here
    pub database_url: String,

    pub telegram_group_id: Option<i64>,

    // TODO: Migrate the environment variables to all have a TRUFOTBOT_ prefix
    #[serde(default = "Config::default_reminder_completion_delete_and_resend")]
    pub trufotbot_reminder_completion_delete_and_resend: bool,
}

impl Config {
    fn default_reminder_completion_delete_and_resend() -> bool {
        // TODO: Change this to true
        false
    }

    pub fn load() -> anyhow::Result<Self> {
        #[cfg(test)]
        {
            unsafe {
                std::env::set_var("DATABASE_URL", "PHONY_DATABASE_URL_FOR_TESTING");
            }
        }

        let config = envy::from_env::<Self>()
            .context("Failed to load config from environment (use UPPER_SNAKE_CASE)")?;
        log::info!("Loaded config: {config:#?}");
        Ok(config)
    }
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

impl FromRef<AppState> for Arc<Config> {
    fn from_ref(state: &AppState) -> Self {
        state.config.clone()
    }
}

impl AppState {
    pub async fn new(
        db: SqlitePool,
        messenger: Messenger,
        config: Arc<Config>,
    ) -> anyhow::Result<Self> {
        let storage = Storage { pool: db };
        let callback = Self::reminder_callback(messenger.clone(), storage.clone());
        let reminder_scheduler = ReminderScheduler::new(callback).await?;

        Ok(AppState {
            storage,
            messenger,
            reminder_scheduler,
            config,
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
