use std::sync::Arc;

use crate::app_state::AppState;
use crate::messenger::SentMessageInfo as _;
use crate::reminder_scheduler::ReminderScheduler;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use shared::api::patient::Reminders;
use sqlx::SqlitePool;
use teloxide::utils::markdown;
use tokio::sync::Mutex;

pub async fn get(
    State(db): State<SqlitePool>,
    Path((patient_id, medication_id)): Path<(i64, i64)>,
) -> Result<Json<Reminders>, (StatusCode, String)> {
    struct ReminderRow {
        cron_schedule: String,
    }

    let schedules = sqlx::query_as!(
        ReminderRow,
        r#"
        SELECT
            r.cron_schedule AS "cron_schedule!"
        FROM reminders r
        WHERE r.patient_id = ?
          AND r.medication_id = ?
        "#,
        patient_id,
        medication_id,
    )
    .fetch_one(&db)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch reminder data".into(),
        )
    })?;

    Ok(Json(Reminders {
        cron_schedules: schedules.cron_schedule.lines().map(String::from).collect(),
    }))
}

pub async fn set(
    State(db): State<SqlitePool>,
    State(reminder_scheduler): State<Arc<Mutex<ReminderScheduler>>>,
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
    .execute(&db)
    .await
    .map_err(|e| {
        log::error!("Failed to set reminders in DB: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to set reminder".into(),
        )
    })?;

    reminder_scheduler
        .lock()
        .await
        .set_reminders(patient_id, medication_id, &cron_schedules)
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
    State(app_state): State<AppState>, // TODO: Get Messenger directly
    Path((patient_id, medication_id)): Path<(i64, i64)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let patient = app_state.get_patient(patient_id).await?;
    let medication = app_state.get_medication(medication_id).await?;

    if patient.telegram_group_id.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Patient has no telegram group ID".to_string(),
        ));
    }

    let base_message = markdown::escape(&format!("Time to take {}.", medication.name));

    let message_id = app_state
        .messenger
        .send_message(&patient, base_message.clone())
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

    app_state
        .messenger
        .edit_message(
            &patient,
            message_id.id(),
            format!(
                "{base_message} {}",
                markdown::link(
                    &deep_link_url(patient_id, medication_id, message_id.id()),
                    "Take",
                )
            ),
        )
        .await?;

    Ok(StatusCode::OK)
}

fn deep_link_url(patient_id: i64, medication_id: i64, message_id: i32) -> String {
    let mut url = url::Url::parse("https://postman-echo.com/get").unwrap();

    url.query_pairs_mut()
        .append_pair(
            "comment",
            "This is a placeholder for a deep link to the app",
        )
        .append_pair("patient_id", &patient_id.to_string())
        .append_pair("medication_id", &medication_id.to_string())
        .append_pair("message_id", &message_id.to_string())
        .finish();

    url.as_str().to_owned()
}

#[cfg(test)]
mod tests {
    use axum::{Json, extract::Query};
    use chrono::NaiveDateTime;
    use pretty_assertions::assert_eq;
    use shared::{
        api::{dose, requests::CreateDoseQueryParams},
        time,
    };
    use sqlx::SqlitePool;

    use super::*;

    use crate::app_state::AppState;

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn remind_dose_succeeds(db: SqlitePool) {
        unsafe {
            time::use_fake_time();
        }
        let app_state = AppState::new(db, None).await.unwrap();

        send_reminder(State(app_state.clone()), Path((1, 1)))
            .await
            .unwrap();

        assert_eq!(
            app_state
                .messenger
                .telegram_messages
                .get_messages(-123)
                .await
                .unwrap(),
            vec![(
                1,
                "Time to take Aspirin\\. [Take](https://postman-echo.com/get?\
                comment=This+is+a+placeholder+for+a+deep+link+to+the+app\
                &patient_id=1&medication_id=1&message_id=1)"
                    .to_string()
            )]
        );

        let taken_at = NaiveDateTime::parse_from_str("2023-04-05 06:07:08", "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc();

        crate::handlers::doses::record(
            Path((1, 1)),
            Query(CreateDoseQueryParams {
                reminder_message_id: Some(1),
            }),
            State(app_state.clone()),
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
            vec![(
                1,
                r#"✅ Albert gave Alice Aspirin \(2\) an hour ago \(2023\-04\-05 \(Wed\) 07:07\)"#
                    .to_string()
            )]
        );
    }
}
