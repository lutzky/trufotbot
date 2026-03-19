use std::sync::Arc;

use anyhow::Context;
use axum::extract::{FromRef, Path, State};

use serde::{Deserialize, Deserializer};
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
    pub database_url: String,

    #[serde(default = "Config::default_frontend_url")]
    pub frontend_url: url::Url,

    #[serde(default)]
    #[serde(deserialize_with = "comma_separated_vec")]
    pub trufotbot_allowed_users: Vec<String>,

    // TODO: Migrate the environment variables to all have a TRUFOTBOT_ prefix
    #[serde(default = "Config::default_reminder_completion_delete_and_resend")]
    pub trufotbot_reminder_completion_delete_and_resend: bool,

    #[serde(default)]
    pub trufotbot_show_dose_absolute_time: bool,
}

fn comma_separated_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    let vec = s.split(',').map(|item| item.trim().to_string()).collect();
    Ok(vec)
}

impl Config {
    fn default_reminder_completion_delete_and_resend() -> bool {
        true
    }

    fn default_frontend_url() -> url::Url {
        #[allow(clippy::expect_used)]
        url::Url::parse("http://0.0.0.0:8080").expect("Default frontend URL should parse")
    }

    fn validate_frontend_url_or_warn(&self) {
        let host = self.frontend_url.host_str().unwrap_or_default();

        if host.contains(".") {
            return;
        }

        let raw_url = self.frontend_url.as_str();
        log::warn!(
            "FRONTEND_URL {raw_url:?} has a host with no dots ({host:?}), links might fail to render. See e.g. https://github.com/telegramdesktop/tdesktop/issues/7827

Hint: Try localhost.localdomain, 127.0.0.1, 0.0.0.0, the target's IP address");
    }

    pub fn check_user(&self, user_id: Option<&str>) -> anyhow::Result<()> {
        let allowed_users: &Vec<String> = &self.trufotbot_allowed_users;

        let Some(user_id) = user_id else {
            anyhow::bail!(
                "Couldn't check if user is allowed to send messages, \
            as user was None"
            );
        };

        if allowed_users.iter().any(|u| u == user_id) {
            return Ok(());
        }

        anyhow::bail!("Forbidden user {user_id:?}; allowed users are {allowed_users:?}");
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

        config.validate_frontend_url_or_warn();

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
        let callback = Self::reminder_callback(messenger.clone(), storage.clone(), config.clone());
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
        config: Arc<Config>,
    ) -> impl Fn(PatientId, MedicationId) {
        move |patient_id: PatientId, medication_id: MedicationId| {
            let storage = storage.clone();
            let messenger = messenger.clone();
            let config = config.clone();
            tokio::spawn(async move {
                if let Err(e) = send_reminder(
                    State(storage),
                    State(messenger),
                    State(config),
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
