// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use color_eyre::eyre::{self, eyre};
use teloxide::utils::markdown;

use crate::{
    api::patient::Reminders,
    app_state::Config,
    errors::ServiceError,
    messenger::{Messenger, callbacks},
    models,
    models::Medication,
    reminder_scheduler::ReminderScheduler,
    storage::Storage,
    time::now,
};

pub const UTOIPA_TAG: &str = "reminders";

fn validate_cron_schedule(schedule: &str) -> Result<(), ServiceError> {
    // Although it would be nice to use [tokio_cron_scheduler::Job::schedule_to_cron] to validate
    // this, validation currently (2025-11-02) only works with feature `english` enabled, and
    // otherwise does nothing.
    #[allow(clippy::unreachable)] // This is never scheduled
    tokio_cron_scheduler::Job::new(schedule, |_, _| unreachable!())
        .map_err(|_| ServiceError::BadRequest(format!("Invalid cron schedule '{schedule}'")))?;
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/patients/{patient_id}/medications/{medication_id}/reminders",
    summary = "Get reminders for patient's medication",
    operation_id = "reminders_get",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Reminders fetched successfully", body=Reminders),
        (status = 404, description = "Medication not found"),
    ),
    params(
        ("patient_id" = i32, Path, description = "Patient ID"),
        ("medication_id" = i32, Path, description = "Medication ID"),
    )
)]
pub async fn get(
    State(storage): State<Storage>,
    Path((patient_id, medication_id)): Path<(i64, i64)>,
) -> Result<Json<Reminders>, ServiceError> {
    struct ReminderRow {
        cron_schedule_lines: String,
    }

    let schedules = sqlx::query_as!(
        ReminderRow,
        r#"
        SELECT
            r.cron_schedule AS "cron_schedule_lines!"
        FROM reminders r
        WHERE r.patient_id = ?
          AND r.medication_id = ?
        "#,
        patient_id,
        medication_id,
    )
    .fetch_optional(&storage.pool)
    .await?;

    let cron_schedules = match schedules {
        Some(s) => s.cron_schedule_lines.lines().map(String::from).collect(),
        None => vec![],
    };

    Ok(Json(Reminders { cron_schedules }))
}

#[utoipa::path(
    put,
    path = "/api/patients/{patient_id}/medications/{medication_id}/reminders",
    summary = "Set reminders for patient's medication",
    operation_id = "reminders_set",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Reminders set successfully"),
        (status = 404, description = "Medication not found"),
    ),
    request_body = Reminders,
    params(
        ("patient_id" = i32, Path, description = "Patient ID"),
        ("medication_id" = i32, Path, description = "Medication ID"),
    )
)]
pub async fn set(
    State(storage): State<Storage>,
    State(mut reminder_scheduler): State<ReminderScheduler>,
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    Json(Reminders { cron_schedules }): Json<Reminders>,
) -> Result<(), ServiceError> {
    for schedule in &cron_schedules {
        validate_cron_schedule(schedule)?;
    }

    let joined_cron_schedule = cron_schedules.join("\n");

    sqlx::query_as!(
        ReminderRow,
        r#"
        INSERT INTO reminders (patient_id, medication_id, cron_schedule)
        VALUES (?, ?, ?)
        ON CONFLICT (patient_id, medication_id) DO UPDATE
        SET cron_schedule = EXCLUDED.cron_schedule
        "#,
        patient_id,
        medication_id,
        joined_cron_schedule,
    )
    .execute(&storage.pool)
    .await?;

    let cron_schedules_ref: Vec<&str> = cron_schedules.iter().map(|s| s.as_str()).collect();

    reminder_scheduler
        .set_reminders(patient_id, medication_id, &cron_schedules_ref)
        .await?;

    Ok(())
}

#[utoipa::path(
    put,
    path = "/api/patients/{patient_id}/medications/{medication_id}/remind",
    summary = "Send a reminder",
    operation_id = "reminders_send",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Reminder sent successfully"),
        (status = 404, description = "Medication not found"),
    ),
    params(
        ("patient_id" = i32, Path, description = "Patient ID"),
        ("medication_id" = i32, Path, description = "Medication ID"),
    )
)]
pub async fn send_reminder(
    State(storage): State<Storage>,
    State(messenger): State<Messenger>,
    State(config): State<Arc<Config>>,
    Path((patient_id, medication_id)): Path<(i64, i64)>,
) -> Result<(), ServiceError> {
    let patient = models::Patient::get(&storage.pool, patient_id).await?;
    let medication = models::Medication::get(&storage.pool, medication_id).await?;
    let latest_dosage = Medication::latest_dosage(&storage.pool, medication_id, patient_id).await?;

    if patient.telegram_group_id.is_none() {
        return Err(ServiceError::BadRequest(
            "Patient has no telegram group ID".to_string(),
        ));
    }

    let default_dosage = latest_dosage.unwrap_or(1.0);

    let base_message = markdown::escape(&format!(
        "Time for {} to take {}.",
        patient.name, medication.name
    ));

    let message_id = messenger
        .send(&patient, base_message.clone(), vec![])
        .await?
        .ok_or_else(|| {
            ServiceError::InternalError(eyre!(
                "Sending message to patient {patient_id} returned None, \
                 though we checked that they have a telegram group ID"
            ))
        })?;

    let url = deep_link(patient_id, medication_id, message_id.id(), now(), &config);

    let keyboard = vec![
        Some((
            format!("Take {default_dosage} 💊"),
            callbacks::Action::TakeFromReminder {
                patient_id,
                medication_id,
                quantity: default_dosage,
            },
        )),
        Some((
            "Skip ⏭️".to_string(),
            callbacks::Action::TakeFromReminder {
                patient_id,
                medication_id,
                quantity: 0.0,
            },
        )),
        match url {
            Ok(url) => Some(("Take... 📝".to_string(), callbacks::Action::Link { url })),
            Err(err) => {
                log::error!("Couldn't build URL to display \"Take...\" button: {err}");
                None
            }
        },
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    messenger
        .edit(&patient, None, message_id.id(), base_message, keyboard)
        .await?;

    Ok(())
}

fn deep_link(
    patient_id: i64,
    medication_id: i64,
    message_id: i32,
    reminder_sent_time: DateTime<Utc>,
    config: &Config,
) -> eyre::Result<url::Url> {
    let mut url = config.frontend_url.clone();

    url.path_segments_mut()
        .map_err(|_| {
            // path_segments_mut() returns a Result<_, ()>, so there's nothing to wrap
            eyre!(
                "frontend_url {:?} can't be used as a base",
                config.frontend_url
            )
        })?
        .push("patients")
        .push(&patient_id.to_string())
        .push("medications")
        .push(&medication_id.to_string());

    url.query_pairs_mut()
        .append_pair("message_id", &message_id.to_string())
        .append_pair("message_time", &reminder_sent_time.timestamp().to_string())
        .finish();

    Ok(url)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        api::{dose, requests::CreateDoseQueryParams},
        app_state::Config,
        time::FAKE_TIME,
    };
    use axum::{Json, extract::Query};

    use pretty_assertions::assert_eq;
    use sqlx::SqlitePool;

    use super::*;

    use crate::{
        app_state::AppState,
        messenger::fake_sender::{FakeSender, messages_from_slice},
    };

    async fn test_remind_dose(db: SqlitePool, delete_and_resend: bool) {
        let fake_telegram = Arc::new(FakeSender::new());
        let messenger = fake_telegram.clone().into();
        let config = Config {
            trufotbot_reminder_completion_delete_and_resend: delete_and_resend,
            trufotbot_show_dose_absolute_time: true,
            ..Config::load().unwrap()
        };
        let app_state = AppState::new(db, messenger, config.into()).await.unwrap();

        FAKE_TIME
            .scope("2025-01-02T00:00:00Z", async {
                send_reminder(
                    State(app_state.storage.clone()),
                    State(app_state.messenger.clone()),
                    State(app_state.config.clone()),
                    Path((1, 1)),
                )
                .await
                .unwrap();
            })
            .await;

        assert_eq!(
            fake_telegram.messages.get_messages(-123).await.unwrap(),
            messages_from_slice(
                &[(
                    r"Time for Alice to take Aspirin\.",
                    &[
                        (
                            "Take 1 💊",
                            callbacks::Action::TakeFromReminder {
                                patient_id: 1,
                                medication_id: 1,
                                quantity: 1.0
                            }
                        ),
                        (
                            "Skip ⏭️",
                            callbacks::Action::TakeFromReminder {
                                patient_id: 1,
                                medication_id: 1,
                                quantity: 0.0
                            }
                        ),
                        (
                            "Take... 📝",
                            callbacks::Action::Link {
                                url: url::Url::parse(
                                    "http://0.0.0.0:8080/patients/1/medications/1?message_id=1&message_time=1735776000"
                                )
                                .unwrap()
                            }
                        )
                    ]
                )],
                1
            )
        );

        let taken_at = DateTime::parse_from_rfc3339("2025-01-01T23:00:00Z")
            .unwrap()
            .to_utc();
        let reminded_at = DateTime::parse_from_rfc3339("2025-01-01T22:00:00Z")
            .unwrap()
            .to_utc();

        FAKE_TIME
            .scope("2025-01-02T00:00:00Z", async {
                crate::handlers::doses::record(
                    Path((1, 1)),
                    Query(CreateDoseQueryParams {
                        reminder_message_id: Some(1),
                        reminder_sent_time: Some(reminded_at),
                    }),
                    State(app_state.storage.clone()),
                    State(app_state.messenger.clone()),
                    State(app_state.config.clone()),
                    Json(dose::CreateDose {
                        quantity: 2.0,
                        taken_at,
                        noted_by_user: Some("Albert".to_string()),
                    }),
                )
                .await
                .unwrap();
            })
            .await;

        let (expected_text, expected_id) = if delete_and_resend {
            (
                r"✅ Albert gave Alice Aspirin \(2\) an hour earlier \(2025\-01\-01 \(Wed\) 23:00\)",
                2,
            )
        } else {
            (
                r"✅ Albert gave Alice Aspirin \(2\) an hour later \(2025\-01\-01 \(Wed\) 23:00\)",
                1,
            )
        };

        assert_eq!(
            fake_telegram.messages.get_messages(-123).await.unwrap(),
            messages_from_slice(
                &[(
                    expected_text,
                    &[
                        (
                            "Edit... ✏️",
                            callbacks::Action::Link {
                                url: url::Url::parse(
                                    "http://0.0.0.0:8080/patients/1/medications/1/doses/1"
                                )
                                .unwrap()
                            }
                        ),
                        (
                            "Repeat 🔁",
                            callbacks::Action::TakeNew {
                                patient_id: 1,
                                medication_id: 1,
                                quantity: 2.0,
                            },
                        )
                    ]
                )],
                expected_id
            )
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn remind_dose_succeeds_with_edit(db: SqlitePool) {
        test_remind_dose(db, false).await;
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn remind_dose_succeeds_with_delete_and_resend(db: SqlitePool) {
        test_remind_dose(db, true).await;
    }
}
