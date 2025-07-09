use chrono::NaiveDateTime;
use serde::Serialize;
use shared::api::medication::DoseLimit;
use sqlx::{FromRow, SqlitePool};

use crate::errors::ServiceError; // Added SqlitePool

#[derive(FromRow, Serialize, Debug, PartialEq)]
pub struct Patient {
    pub id: i64,
    pub telegram_group_id: Option<i64>,
    pub name: String,
}

impl Patient {
    /// Fetches a patient by their ID from the database.
    pub async fn get(db: &SqlitePool, patient_id: i64) -> Result<Patient, ServiceError> {
        let res = sqlx::query_as!(
            Patient,
            r"SELECT id, name, telegram_group_id FROM patients WHERE id = ?",
            patient_id
        )
        .fetch_one(db)
        .await;

        match res {
            Err(sqlx::Error::RowNotFound) => Err(ServiceError::not_found("Patient not found")),
            _ => Ok(res?),
        }
    }

    /// Fetches a patient by their ID from the database.
    pub async fn find_by_name(
        db: &SqlitePool,
        patient_name: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Patient,
            r#"SELECT id AS "id!", name, telegram_group_id FROM patients WHERE name = ?"#,
            patient_name
        )
        .fetch_optional(db)
        .await
    }
}

#[derive(FromRow, Serialize, Debug)]
pub struct Medication {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub dose_limits: Vec<DoseLimit>,
    pub inventory: Option<f64>,
}

impl Medication {
    pub async fn get(db: &SqlitePool, medication_id: i64) -> Result<Self, ServiceError> {
        let result = sqlx::query!(
            r"SELECT id, name, description, dose_limits, inventory FROM medications WHERE id = ?",
            medication_id
        )
        .fetch_one(db)
        .await;

        let row = match result {
            Err(sqlx::Error::RowNotFound) => Err(ServiceError::not_found("Medication not found")),
            _ => Ok(result?),
        }?;

        Ok(Medication {
            id: row.id,
            name: row.name,
            inventory: row.inventory,
            description: row.description,
            dose_limits: DoseLimit::vec_from_string(&row.dose_limits.unwrap_or_default())?,
        })
    }

    pub async fn find_by_name(
        db: &SqlitePool,
        medication_name: &str,
    ) -> Result<Option<Self>, ServiceError> {
        let result = sqlx::query!(
            r#"SELECT id as "id!", name, description, dose_limits, inventory FROM medications WHERE name = ?"#,
            medication_name
        )
        .fetch_optional(db)
        .await?;

        let res = result.map(|result| {
            Ok(Medication {
                id: result.id,
                name: result.name,
                inventory: result.inventory,
                description: result.description,
                dose_limits: DoseLimit::vec_from_string(&result.dose_limits.unwrap_or_default())?,
            })
        });

        res.transpose()
    }

    pub async fn latest_dosage(
        db: &SqlitePool,
        medication_id: i64,
        patient_id: i64,
    ) -> Result<Option<f64>, ServiceError> {
        let result = sqlx::query!(
            r"SELECT quantity
              FROM doses
              WHERE
                medication_id = ? AND
                patient_id = ? AND
                quantity > 0
            ORDER BY taken_at DESC
            LIMIT 1",
            medication_id,
            patient_id
        )
        .fetch_optional(db)
        .await?;

        Ok(result.map(|result| (result.quantity)))
    }
}

#[derive(FromRow, Serialize)]
pub struct Reminder {
    pub patient_id: i64,
    pub medication_id: i64,
    pub hour: Option<u8>,
}

#[derive(FromRow, Serialize)]
pub struct Dose {
    pub id: i64,
    pub patient_id: i64,
    pub medication_id: i64,
    pub quantity: f64,
    pub taken_at: NaiveDateTime,
    pub noted_by_user: Option<String>,
}
