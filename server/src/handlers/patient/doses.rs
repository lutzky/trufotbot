use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use shared::{
    api::{
        dose::{self},
        requests::CreateDoseQueryParams,
        responses,
    },
    time,
};
use teloxide::utils::markdown;

use crate::{app_state::AppState, models};

#[axum::debug_handler]
pub async fn record(
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    Query(CreateDoseQueryParams {
        reminder_message_id,
    }): Query<CreateDoseQueryParams>,
    State(app_state): State<AppState>,
    Json(payload): Json<dose::CreateDose>,
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
        maybe_name => format!(
            "{} gave {} {}",
            maybe_name.unwrap_or("Someone".to_owned()),
            patient.name,
            medication.name
        ),
    });

    let base_msg = format!(
        "{who_gave_whom} ({}) {} ({})",
        payload.quantity,
        time::time_ago(&payload.taken_at),
        time::local_display(&payload.taken_at),
    );

    if let Some(reminder_message_id) = reminder_message_id {
        app_state
            .edit_message(
                &patient,
                reminder_message_id,
                markdown::escape(&format!("✅ {base_msg}")),
            )
            .await?;
    } else {
        app_state
            .send_message(&patient, markdown::escape(&base_msg))
            .await?;
    }

    Ok(StatusCode::CREATED)
}

pub async fn get(
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    State(app_state): State<AppState>,
) -> Result<Json<responses::PatientGetDosesResponse>, (StatusCode, String)> {
    let patient = app_state.get_patient(patient_id).await?;
    let medication = app_state.get_medication(medication_id).await?;

    let doses = sqlx::query!(
        r#"
        SELECT
            d.id,
            d.taken_at,
            d.quantity,
            d.noted_by_user,
            m.name AS medication_name
        FROM doses d
        JOIN medications m ON d.medication_id = m.id
        WHERE d.patient_id = ? AND d.medication_id = ?
        "#,
        patient_id,
        medication_id
    )
    .map(|row| {
        let taken_at: chrono::NaiveDateTime = row.taken_at;
        let quantity: f64 = row.quantity;
        let noted_by_user: Option<String> = row.noted_by_user;

        dose::Dose {
            id: row.id,
            data: dose::CreateDose {
                quantity,
                taken_at: taken_at.and_utc(),
                noted_by_user,
            },
        }
    })
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch doses".to_string(),
        )
    })?;

    Ok(Json(responses::PatientGetDosesResponse {
        patient_name: patient.name,
        medication_name: medication.name,
        doses,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDateTime, Utc};
    use pretty_assertions::assert_eq;
    use shared::api::dose;
    use sqlx::SqlitePool;

    #[sqlx::test(fixtures("../../fixtures/patients.sql"))]
    async fn record_dose_fails_with_nonexistent_medication(db: SqlitePool) {
        let app_state = AppState::new(db, None);

        let result = record(
            Path((1, 999)),
            Query(Default::default()),
            State(app_state),
            Json(dose::CreateDose {
                quantity: 2.0,
                taken_at: Utc::now(),
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

        let taken_at = NaiveDateTime::parse_from_str("2023-04-05 06:07:08", "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc();

        record(
            Path((1, 1)),
            Query(Default::default()),
            State(app_state.clone()),
            Json(dose::CreateDose {
                quantity: 2.0,
                taken_at,
                noted_by_user: Some("Alice".to_string()),
            }),
        )
        .await
        .unwrap();

        let result = get(Path((1, 1)), State(app_state.clone())).await.unwrap().0;

        assert_eq!(
            result,
            responses::PatientGetDosesResponse {
                patient_name: "Alice".to_string(),
                medication_name: "Aspirin".to_string(),
                doses: vec![dose::Dose {
                    id: 1,
                    data: dose::CreateDose {
                        quantity: 2.0,
                        taken_at,
                        noted_by_user: Some("Alice".to_string()),
                    },
                }],
            }
        );

        assert_eq!(
            app_state
                .telegram_messages
                .get_messages(-123)
                .await
                .unwrap(),
            vec![(
                1,
                // FIXME: We always emit UTC here, but we want telegram to show the local time
                "Alice took Aspirin \\(2\\) at 2023\\-04\\-05 06:07:08 UTC".to_string()
            )]
        );
    }
}
