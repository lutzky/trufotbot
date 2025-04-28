use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use shared::api::patient_types;
use teloxide::utils::markdown;

use crate::{app_state::AppState, models};

pub async fn record(
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    State(app_state): State<AppState>,
    Json(payload): Json<patient_types::CreateDose>,
) -> Result<StatusCode, (StatusCode, String)> {
    let patient = app_state.get_patient(patient_id).await?;

    // TODO: Test what happens if the medication_id is not found

    let medication = sqlx::query_as!(
        models::Medication,
        "SELECT id, name, description FROM medications WHERE id = ?",
        medication_id
    )
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch medication".to_string(),
        )
    })?;

    let Some(medication) = medication else {
        return Err((StatusCode::NOT_FOUND, "Medication not found".to_string()));
    };

    sqlx::query!(
        r#"
        INSERT INTO  doses (patient_id, medication_id, quantity, taken_at, noted_by_user)
        VALUES (?, ?, ?, ?, ?)
        "#,
        patient_id,
        medication_id,
        payload.quantity,
        payload.taken_at,
        payload.noted_by_user,
    )
    .execute(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to record intake".to_string(),
        )
    })?;

    let who_gave_whom = markdown::escape(&match payload.noted_by_user {
        Some(name) if name == patient.name => format!("{} took {}", name, medication.name),
        Some(name) => format!("{} gave {} {}", name, patient.name, medication.name),
        None => format!(
            "{} was given {} (by unknown)",
            patient.name, medication.name
        ),
    });

    app_state
        .send_message(
            &patient,
            markdown::escape(&format!(
                "{who_gave_whom} ({}) at ({})",
                payload.quantity, payload.taken_at
            )),
        )
        .await?;

    // TODO: Humanize taken_at

    // TODO: Support editing previous messages instead if this is a result of a reminder

    Ok(StatusCode::CREATED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDateTime, Utc};
    use sqlx::SqlitePool;

    #[sqlx::test(fixtures("../../fixtures/patients.sql"))]
    async fn record_dose_fails_with_nonexistent_medication(db: SqlitePool) {
        let app_state = AppState::new(db, None);

        let result = record(
            Path((1, 999)),
            State(app_state),
            Json(patient_types::CreateDose {
                quantity: 2.0,
                taken_at: Utc::now().naive_utc(),
                noted_by_user: Some("Alice".to_string()),
            }),
        )
        .await;

        assert_eq!(
            result,
            Err((StatusCode::NOT_FOUND, "Medication not found".to_string()))
        );
    }

    #[sqlx::test(fixtures("../../fixtures/patients.sql", "../../fixtures/medications.sql"))]
    async fn record_dose_succeeds(db: SqlitePool) {
        let app_state = AppState::new(db, None);

        let taken_at =
            NaiveDateTime::parse_from_str("2023-04-05 06:07:08", "%Y-%m-%d %H:%M:%S").unwrap();

        record(
            Path((1, 1)),
            State(app_state.clone()),
            Json(patient_types::CreateDose {
                quantity: 2.0,
                taken_at,
                noted_by_user: Some("Alice".to_string()),
            }),
        )
        .await
        .unwrap();

        let result = sqlx::query!(
            r#"SELECT taken_at FROM doses 
              WHERE
                patient_id = 1 AND 
                medication_id = 1 AND
                quantity = 2.0 AND
                noted_by_user = "Alice""#,
        )
        .fetch_one(&app_state.db)
        .await
        .unwrap();

        assert_eq!(result.taken_at, taken_at);

        assert_eq!(
            app_state
                .telegram_messages
                .get_messages(-123)
                .await
                .unwrap(),
            vec![(
                1,
                "Alice took Aspirin \\(2\\) at \\(2023\\-04\\-05 06:07:08\\)".to_string()
            )]
        );
    }
}
