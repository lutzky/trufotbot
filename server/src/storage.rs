use axum::http::StatusCode;
use sqlx::SqlitePool;

use crate::models::{Medication, Patient};

#[derive(Clone)]
pub struct Storage {
    pub pool: SqlitePool,
}

impl Storage {
    // TODO: get_patient and get_medication are effectively duplicates and only serve to map
    // errors; eliminate them and replace them with an explicit "log an map errors" thing.

    pub async fn get_patient(&self, patient_id: i64) -> Result<Patient, (StatusCode, String)> {
        Patient::get(&self.pool, patient_id)
            .await
            .map_err(|e| {
                log::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to fetch patient".to_string(),
                )
            })?
            .ok_or((StatusCode::NOT_FOUND, "Patient not found".to_string()))
    }

    pub async fn get_medication(
        &self,
        medication_id: i64,
    ) -> Result<Medication, (StatusCode, String)> {
        Medication::get(&self.pool, medication_id)
            .await
            .map_err(|e| {
                log::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to fetch medication".to_string(),
                )
            })?
            .ok_or((StatusCode::NOT_FOUND, "Medication not found".to_string()))
    }
}
