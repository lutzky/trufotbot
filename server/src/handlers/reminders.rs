use std::env;

use crate::messenger::{Messenger, SentMessageInfo as _, callbacks};
use crate::models::Medication;
use crate::reminder_scheduler::ReminderScheduler;
use crate::storage::Storage;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use shared::api::patient::Reminders;
use teloxide::utils::markdown;

pub async fn get(
    State(storage): State<Storage>,
    Path((patient_id, medication_id)): Path<(i64, i64)>,
) -> Result<Json<Reminders>, (StatusCode, String)> {
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
    .await
    .map_err(|e| {
        log::error!("Failed to fetch reminder data from DB: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch reminder data".into(),
        )
    })?;

    let cron_schedules = match schedules {
        Some(s) => s.cron_schedule_lines.lines().map(String::from).collect(),
        None => vec![],
    };

    Ok(Json(Reminders { cron_schedules }))
}

pub async fn set(
    State(storage): State<Storage>,
    State(mut reminder_scheduler): State<ReminderScheduler>,
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    Json(Reminders { cron_schedules }): Json<Reminders>,
) -> Result<(), (StatusCode, String)> {
    for schedule in &cron_schedules {
        tokio_cron_scheduler::Job::new(schedule, |_, _| unreachable!()).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid cron schedule '{}'", schedule),
            )
        })?;
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
    .await
    .map_err(|e| {
        log::error!("Failed to set reminders in DB: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to set reminder".into(),
        )
    })?;

    let cron_schedules_ref: Vec<&str> = cron_schedules.iter().map(|s| s.as_str()).collect();

    reminder_scheduler
        .set_reminders(patient_id, medication_id, &cron_schedules_ref)
        .await
        .map_err(|e| {
            log::error!("Failed to set reminders: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to set reminders".into(),
            )
        })?;

    Ok(())
}

pub async fn send_reminder(
    State(storage): State<Storage>,
    State(messenger): State<Messenger>,
    Path((patient_id, medication_id)): Path<(i64, i64)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let patient = storage.get_patient(patient_id).await?;
    let medication = storage.get_medication(medication_id).await?;
    let latest_dosage = Medication::latest_dosage(&storage.pool, medication_id, patient_id)
        .await
        .map_err(|e| {
            log::error!("Failed to fetch latest dosage from DB: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch latest dosage data".into(),
            )
        })?;

    if patient.telegram_group_id.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Patient has no telegram group ID".to_string(),
        ));
    }

    let default_dosage = latest_dosage.unwrap_or(1.0);

    let base_message = markdown::escape(&format!("Time to take {}.", medication.name));

    let message_id = messenger
        .send(&patient, base_message.clone())
        .await?
        .ok_or_else(|| {
            log::error!(
                "Sending message to patient {patient_id} returned None, \
                 though we checked that they have a telegram group ID"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send message".to_string(),
            )
        })?;

    messenger
        .edit(
            &patient,
            message_id.id(),
            format!(
                "{base_message} {}",
                &deep_link(patient_id, medication_id, message_id.id(), "Take"),
            ),
            vec![
                (
                    format!("Take {default_dosage}"),
                    callbacks::Action::Take {
                        patient_id,
                        medication_id,
                        quantity: default_dosage,
                    },
                ),
                (
                    "Skip".to_string(),
                    callbacks::Action::Take {
                        patient_id,
                        medication_id,
                        quantity: 0.0,
                    },
                ),
            ],
        )
        .await?;

    Ok(StatusCode::OK)
}

fn deep_link(patient_id: i64, medication_id: i64, message_id: i32, text: &str) -> String {
    let base_url = env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let mut url = url::Url::parse(&base_url).unwrap();

    url.path_segments_mut()
        .unwrap()
        .push("patients")
        .push(&patient_id.to_string())
        .push("medications")
        .push(&medication_id.to_string());

    url.query_pairs_mut()
        .append_pair("message_id", &message_id.to_string())
        .finish();

    if base_url.contains("localhost") || base_url.contains("127.0.0.1") {
        markdown::escape(&format!(
            "{} (link would've probably been blocked by telegram)",
            url.as_str()
        ))
    } else {
        markdown::link(url.as_str(), text)
    }
}

#[cfg(test)]
mod tests {
    use axum::{Json, extract::Query};
    use chrono::TimeDelta;
    use pretty_assertions::assert_eq;
    use shared::{
        api::{dose, requests::CreateDoseQueryParams},
        time::{self, now},
    };
    use sqlx::SqlitePool;

    use super::*;

    use crate::{app_state::AppState, messenger::fake_telegram::messages_from_slice};

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn remind_dose_succeeds(db: SqlitePool) {
        unsafe {
            time::use_fake_time();
        }
        let app_state = AppState::new(db, None).await.unwrap();

        send_reminder(
            State(app_state.storage.clone()),
            State(app_state.messenger.clone()),
            Path((1, 1)),
        )
        .await
        .unwrap();

        assert_eq!(
            app_state
                .messenger
                .telegram_messages
                .get_messages(-123)
                .await
                .unwrap(),
            messages_from_slice(&[(
                "Time to take Aspirin\\. \
                http://localhost:8080/patients/1/medications/1?message\\_id\\=1 \
                \\(link would've probably been blocked by telegram\\)",
                &[
                    (
                        "Take 1",
                        callbacks::Action::Take {
                            patient_id: 1,
                            medication_id: 1,
                            quantity: 1.0
                        }
                    ),
                    (
                        "Skip",
                        callbacks::Action::Take {
                            patient_id: 1,
                            medication_id: 1,
                            quantity: 0.0
                        }
                    )
                ]
            )])
        );

        let taken_at = now() - TimeDelta::hours(1);

        crate::handlers::doses::record(
            Path((1, 1)),
            Query(CreateDoseQueryParams {
                reminder_message_id: Some(1),
            }),
            State(app_state.storage.clone()),
            State(app_state.messenger.clone()),
            Json(dose::CreateDose {
                quantity: 2.0,
                taken_at,
                noted_by_user: Some("Albert".to_string()),
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            app_state
                .messenger
                .telegram_messages
                .get_messages(-123)
                .await
                .unwrap(),
            messages_from_slice(&[(
                r#"✅ Albert gave Alice Aspirin \(2\) an hour ago \(2025\-01\-01 \(Wed\) 23:00\)"#,
                &[]
            )])
        );
    }
}
