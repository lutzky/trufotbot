use crate::app_state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use shared::api::{medication, patient, requests, responses};
use teloxide::utils::markdown;

pub mod doses;
pub mod remind;

pub async fn get(
    Path(patient_id): Path<i64>,
    State(app_state): State<AppState>,
) -> Result<Json<responses::PatientGetResponse>, (StatusCode, String)> {
    // Fetch patient details
    let patient = app_state.get_patient(patient_id).await?;

    // Fetch medications and their last dose time for this patient
    // This query joins medications and doses, grouping by medication
    let medications = sqlx::query!(
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

    // Can't use query_as! here because taken_at is interpreted as a
    // NaiveDateTime rather than DateTime<Utc>; see https://github.com/launchbadge/sqlx/issues/2288
    let medications: Vec<medication::MedicationSummary> = medications
        .into_iter()
        .map(|med| medication::MedicationSummary {
            id: med.id,
            name: med.name,
            last_taken_at: med.last_taken_at.map(|ndt| ndt.and_utc()),
        })
        .collect();

    let response = responses::PatientGetResponse {
        name: patient.name,
        telegram_group_id: patient.telegram_group_id,
        medications,
    };

    Ok(Json(response))
}

pub async fn delete(
    State(app_state): State<AppState>,
    Path(patient_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let internal_server_error = (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to delete patient",
    );

    let mut tx = app_state.db.begin().await.map_err(|e| {
        log::error!("Failed to create transaction: {e}");
        internal_server_error
    })?;

    sqlx::query!(
        r#"
        DELETE FROM doses
        WHERE patient_id = ?
        "#,
        patient_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to delete patient's doses: {e}");
        internal_server_error
    })?;

    sqlx::query!(
        r#"
        DELETE FROM reminders
        WHERE patient_id = ?
        "#,
        patient_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to delete patient's reminders : {e}");
        internal_server_error
    })?;

    let result = sqlx::query!(
        r#"
        DELETE FROM patients
        WHERE id = ?
        "#,
        patient_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to delete patient: {e}");
        internal_server_error
    })?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Patient not found"));
    }

    tx.commit().await.map_err(|e| {
        log::error!("Failed to commit transaction: {e}");
        internal_server_error
    })?;
    Ok(StatusCode::OK)
}

pub async fn update(
    State(app_state): State<AppState>,
    Path(patient_id): Path<i64>,
    Json(payload): Json<requests::PatientCreateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        r#"
        UPDATE patients
        SET name = ?,
            telegram_group_id = ?
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

// update_medication receives both a patient_id and a medication_id, as some
// medication settings will be per-patient (reminders, in particular). It also
// saves us, at this point, the need for creating a "medications browser without
// the context of a user".
pub async fn update_medication(
    State(app_state): State<AppState>,
    Path((_patient_id, medication_id)): Path<(i64, i64)>,
    Json(payload): Json<requests::PatientMedicationUpdateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        r#"
        UPDATE medications
        SET name = ?,
            description = ?
        WHERE id = ?
        "#,
        payload.name,
        payload.description,
        medication_id
    )
    .execute(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update medication".to_string(),
        )
    })?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Medication not found".to_string()));
    }
    Ok(StatusCode::OK)
}

// TODO: pre-commit check to make sure we don't have axum debug handlers lying around

pub async fn create(
    State(app_state): State<AppState>,
    Json(payload): Json<requests::PatientCreateRequest>,
) -> Result<(StatusCode, Json<responses::PatientCreateResponse>), (StatusCode, &'static str)> {
    let result = sqlx::query!(
        r#"
        INSERT INTO patients(name,telegram_group_id) VALUES
        (?, ?)
        "#,
        payload.name,
        payload.telegram_group_id,
    )
    .execute(&app_state.db)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create patient",
        )
    })?;
    Ok((
        StatusCode::OK,
        Json(responses::PatientCreateResponse {
            id: result.last_insert_rowid(),
        }),
    ))
}

pub async fn list(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<patient::Patient>>, (StatusCode, String)> {
    let patients = sqlx::query_as!(
        patient::Patient,
        r#"SELECT id as "id!", name FROM patients"#
    )
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
    use shared::api::patient::Patient;
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
                    name: "Alice".to_string(),
                },
                Patient {
                    id: 2,
                    name: "Bob".to_string(),
                },
                Patient {
                    id: 3,
                    name: "Carol".to_string(),
                },
            ]
        );
    }
}
