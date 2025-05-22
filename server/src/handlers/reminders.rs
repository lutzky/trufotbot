use crate::app_state::SentMessageInfo;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use teloxide::utils::markdown;

use crate::app_state::AppState;

pub async fn send_reminder(
    State(app_state): State<AppState>,
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
        let app_state = AppState::new(db, None);

        send_reminder(State(app_state.clone()), Path((1, 1)))
            .await
            .unwrap();

        assert_eq!(
            app_state
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
