use crate::{app_state::AppState, models::Patient};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use shared::api::patient_types;
use teloxide::utils::markdown;

pub mod doses;
pub mod remind;

pub async fn get_medication_menu(
    Path(patient_id): Path<i64>,
    State(app_state): State<AppState>,
) -> Result<Json<patient_types::MedicationMenu>, (StatusCode, String)> {
    // Fetch patient details
    let patient = app_state.get_patient(patient_id).await?;

    // Fetch medications and their last dose time for this patient
    // This query joins medications and doses, grouping by medication
    let medications = sqlx::query_as!(
        patient_types::MedicationMenuItem,
        r#"
        SELECT
            m.id AS "id!",
            m.name AS "name!",
            MAX(d.taken_at) AS last_taken_at
        FROM medications m
        LEFT JOIN doses d ON m.id = d.medication_id AND d.patient_id = $1
        GROUP BY m.id, m.name
        ORDER BY last_taken_at DESC NULLS LAST
        "#,
        patient_id
    )
    .fetch_all(&app_state.db)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch medication data".into(),
        )
    })?;

    let response = patient_types::MedicationMenu {
        patient_id: patient.id,
        patient_name: patient.name,
        medications,
    };

    Ok(Json(response))
}

pub async fn update(
    State(app_state): State<AppState>,
    Path(patient_id): Path<i64>,
    Json(payload): Json<patient_types::UpdateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        r#"
        UPDATE patients
        SET name = COALESCE(?, name),
            telegram_group_id = COALESCE(?, telegram_group_id)
        WHERE id = ?
        "#,
        payload.name,
        payload.telegram_group_id,
        patient_id
    )
    .execute(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update patient".to_string(),
        )
    })?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Patient not found".to_string()));
    }
    Ok(StatusCode::OK)
}

pub async fn list(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<Patient>>, (StatusCode, String)> {
    let patients = sqlx::query_as!(Patient, "SELECT id, name, telegram_group_id FROM patients")
        .fetch_all(&app_state.db)
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch users".to_string(),
            )
        })?;

    Ok(Json(patients))
}

pub async fn ping(
    State(app_state): State<AppState>,
    Path(patient_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let patient = app_state.get_patient(patient_id).await?;

    log::debug!("Pinging patient {:?}", patient);

    app_state
        .send_message(&patient, markdown::escape("Ping!"))
        .await?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use sqlx::SqlitePool;

    #[sqlx::test(fixtures("../../fixtures/patients.sql"))]
    async fn list_patients_correct(db: SqlitePool) {
        let app_state = AppState::new(db, None);

        let patients = list(State(app_state)).await.unwrap();
        assert_eq!(
            patients.0,
            vec![
                Patient {
                    id: 1,
                    telegram_group_id: Some(-123),
                    name: "Alice".to_string(),
                },
                Patient {
                    id: 2,
                    telegram_group_id: Some(-123),
                    name: "Bob".to_string(),
                },
                Patient {
                    id: 3,
                    telegram_group_id: Some(-123),
                    name: "Carol".to_string(),
                },
            ]
        );
    }
}
