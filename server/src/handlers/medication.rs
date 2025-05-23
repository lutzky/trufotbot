use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use shared::api::{requests::PatientMedicationCreateRequest, responses::MedicationCreateResponse};
use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::reminder_scheduler::ReminderScheduler;

pub async fn delete(
    State(db): State<SqlitePool>,
    State(reminder_scheduler): State<Option<Arc<Mutex<ReminderScheduler>>>>,
    Path(medication_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let internal_server_error = (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to delete patient",
    );

    let mut tx = db.begin().await.map_err(|e| {
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

    if let Some(reminder_scheduler) = reminder_scheduler {
        let mut scheduler = reminder_scheduler.lock().await;
        scheduler
            .remove_medication(medication_id)
            .await
            .map_err(|e| {
                log::error!("Failed to remove medication from scheduler: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to remove medication from scheduler",
                )
            })?;
    }

    Ok(StatusCode::OK)
}

// update receives both a patient_id and a medication_id, as some
// medication settings will be per-patient (reminders, in particular). It also
// saves us, at this point, the need for creating a "medications browser without
// the context of a user".
pub async fn update(
    State(db): State<SqlitePool>,
    Path((_patient_id, medication_id)): Path<(i64, i64)>,
    Json(payload): Json<PatientMedicationCreateRequest>,
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
    .execute(&db)
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

pub async fn create(
    State(db): State<SqlitePool>,
    Json(payload): Json<PatientMedicationCreateRequest>,
) -> Result<(StatusCode, Json<MedicationCreateResponse>), (StatusCode, String)> {
    let result = sqlx::query!(
        r#"
        INSERT INTO medications(name, description) VALUES (?, ?)
        "#,
        payload.name,
        payload.description,
    )
    .execute(&db)
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
