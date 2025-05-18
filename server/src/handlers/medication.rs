use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::app_state::AppState;

pub async fn delete(
    State(app_state): State<AppState>,
    Path(medication_id): Path<i64>,
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
    Ok(StatusCode::OK)
}
