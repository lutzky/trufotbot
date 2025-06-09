use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use shared::api::{
    medication::DoseLimit,
    requests::{PatientMedicationCreateRequest, PatientMedicationUpdateRequest},
    responses::MedicationCreateResponse,
};

use crate::{reminder_scheduler::ReminderScheduler, storage::Storage};

use super::reminders;

pub async fn delete(
    State(storage): State<Storage>,
    State(mut reminder_scheduler): State<ReminderScheduler>,
    Path(medication_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let internal_server_error = (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to delete patient",
    );

    let mut tx = storage.pool.begin().await.map_err(|e| {
        log::error!("Failed to create transaction: {e}");
        internal_server_error
    })?;

    sqlx::query!(
        r#"
        DELETE FROM doses
        WHERE medication_id = ?
        "#,
        medication_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to delete doses for medication: {e}");
        internal_server_error
    })?;

    sqlx::query!(
        r#"
        DELETE FROM reminders
        WHERE medication_id = ?
        "#,
        medication_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to delete reminders for medication: {e}");
        internal_server_error
    })?;

    let result = sqlx::query!(
        r#"
        DELETE FROM medications
        WHERE id = ?
        "#,
        medication_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to delete medication: {e}");
        internal_server_error
    })?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Medication not found"));
    }

    tx.commit().await.map_err(|e| {
        log::error!("Failed to commit transaction: {e}");
        internal_server_error
    })?;

    reminder_scheduler
        .remove_medication(medication_id)
        .await
        .map_err(|e| {
            log::error!("Failed to remove medication from scheduler: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to remove medication from scheduler",
            )
        })?;

    Ok(StatusCode::OK)
}

pub async fn update(
    State(storage): State<Storage>,
    State(reminder_scheduler): State<ReminderScheduler>,
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    Json(payload): Json<PatientMedicationUpdateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let dose_limits_string = DoseLimit::string_from_vec(&payload.medication.dose_limits);
    let result = sqlx::query!(
        r#"
        UPDATE medications
        SET name = ?,
            description = ?,
            inventory = ?,
            dose_limits = ?
        WHERE id = ?
        "#,
        payload.medication.name,
        payload.medication.description,
        payload.medication.inventory,
        dose_limits_string,
        medication_id
    )
    .execute(&storage.pool)
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
    reminders::set(
        State(storage),
        State(reminder_scheduler),
        Path((patient_id, medication_id)),
        Json(payload.reminders),
    )
    .await?;
    Ok(StatusCode::OK)
}

pub async fn create(
    State(storage): State<Storage>,
    Json(payload): Json<PatientMedicationCreateRequest>,
) -> Result<(StatusCode, Json<MedicationCreateResponse>), (StatusCode, String)> {
    let result = sqlx::query!(
        r#"
        INSERT INTO medications(name, description) VALUES (?, ?)
        "#,
        payload.name,
        payload.description,
    )
    .execute(&storage.pool)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update medication".to_string(),
        )
    })?
    .last_insert_rowid();
    Ok((
        StatusCode::CREATED,
        Json(MedicationCreateResponse { id: result }),
    ))
}
