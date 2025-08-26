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

use crate::{errors::ServiceError, reminder_scheduler::ReminderScheduler, storage::Storage};

use super::reminders;

pub const UTOIPA_TAG: &str = "medication";

#[utoipa::path(
    delete,
    path = "/api/medications/{id}",
    operation_id = "medication_delete",
    summary = "Delete a medication",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Medication deleted successfully"),
        (status = 404, description = "Medication not found"),
    ),
    params(
        ("id" = i64, Path, description = "Medication ID"),
    )
)]
pub async fn delete(
    State(storage): State<Storage>,
    State(mut reminder_scheduler): State<ReminderScheduler>,
    Path(medication_id): Path<i64>,
) -> Result<(), ServiceError> {
    let mut tx = storage.pool.begin().await?;

    sqlx::query!(
        r#"
        DELETE FROM doses
        WHERE medication_id = ?
        "#,
        medication_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM reminders
        WHERE medication_id = ?
        "#,
        medication_id
    )
    .execute(&mut *tx)
    .await?;

    let result = sqlx::query!(
        r#"
        DELETE FROM medications
        WHERE id = ?
        "#,
        medication_id
    )
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ServiceError::not_found("Medication not found"));
    }

    tx.commit().await?;

    reminder_scheduler.remove_medication(medication_id).await?;

    Ok(())
}

#[utoipa::path(
    put,
    path = "/api/patients/{patient_id}/medications/{medication_id}",
    summary = "Update a medication",
    operation_id = "medication_update",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Medication updated successfully"),
        (status = 404, description = "Medication not found"),
    ),
    request_body = PatientMedicationUpdateRequest,
    params(
        ("patient_id" = i64, Path, description = "Patient ID"),
        ("medication_id" = i64, Path, description = "Medication ID"),
    )
)]
pub async fn update(
    State(storage): State<Storage>,
    State(reminder_scheduler): State<ReminderScheduler>,
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    Json(payload): Json<PatientMedicationUpdateRequest>,
) -> Result<(), ServiceError> {
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
    .await?;

    if result.rows_affected() == 0 {
        return Err(ServiceError::not_found("Medication not found"));
    }
    reminders::set(
        State(storage),
        State(reminder_scheduler),
        Path((patient_id, medication_id)),
        Json(payload.reminders),
    )
    .await?;
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/medications",
    operation_id = "medication_create",
    summary = "Create a new medication",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Medication created successfully", body = MedicationCreateResponse),
    ),
    request_body = PatientMedicationCreateRequest,
)]
pub async fn create(
    State(storage): State<Storage>,
    Json(payload): Json<PatientMedicationCreateRequest>,
) -> Result<(StatusCode, Json<MedicationCreateResponse>), ServiceError> {
    let result = sqlx::query!(
        r#"
        INSERT INTO medications(name, description) VALUES (?, ?)
        "#,
        payload.name,
        payload.description,
    )
    .execute(&storage.pool)
    .await?
    .last_insert_rowid();
    Ok((
        StatusCode::CREATED,
        Json(MedicationCreateResponse { id: result }),
    ))
}
