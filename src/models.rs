// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::api::medication::DoseLimits;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::errors::ServiceError; // Added SqlitePool

#[derive(Serialize, Debug, PartialEq, Eq)]
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

#[derive(Serialize, Debug)]
pub struct Medication {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub dose_limits: DoseLimits,
    pub inventory: Option<f64>,
}

impl Medication {
    pub async fn get(db: &SqlitePool, medication_id: i64) -> Result<Self, ServiceError> {
        let result = sqlx::query_as!(
            Medication,
            r#"
            SELECT
                id,
                name,
                description,
                dose_limits as "dose_limits!: DoseLimits",
                inventory
            FROM medications
            WHERE id = ?"#,
            medication_id
        )
        .fetch_one(db)
        .await;

        match result {
            Err(sqlx::Error::RowNotFound) => Err(ServiceError::not_found("Medication not found")),
            _ => Ok(result?),
        }
    }

    pub async fn find_by_name(
        db: &SqlitePool,
        medication_name: &str,
    ) -> Result<Option<Self>, ServiceError> {
        let res = sqlx::query_as!(
            Medication,
            r#"
            SELECT
              id as "id!",
              name,
              description,
              dose_limits as "dose_limits!: DoseLimits",
              inventory
            FROM medications
            WHERE name = ?
            "#,
            medication_name
        )
        .fetch_optional(db)
        .await;

        match res {
            Err(sqlx::Error::RowNotFound) => Err(ServiceError::not_found("Medication not found")),
            _ => Ok(res?),
        }
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

        Ok(result.map(|result| result.quantity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::api::medication::DoseLimit;

    #[sqlx::test]
    async fn decode_medication_success(db: SqlitePool) {
        let result = sqlx::query!(
            r#"
            INSERT INTO medications(name, dose_limits)
            VALUES (?, ?)
            "#,
            "good_limits",
            "1:2,3:4"
        )
        .execute(&db)
        .await
        .unwrap()
        .last_insert_rowid();

        let m = Medication::get(&db, result).await.unwrap();

        assert_eq!(
            &*m.dose_limits,
            [
                DoseLimit {
                    hours: 1,
                    amount: 2.0
                },
                DoseLimit {
                    hours: 3,
                    amount: 4.0
                },
            ]
        );
    }

    #[sqlx::test]
    async fn decode_medication_failure(db: SqlitePool) {
        let result = sqlx::query!(
            r#"
            INSERT INTO medications(name, dose_limits)
            VALUES (?, ?)
            "#,
            "bad limits",
            "NONSENSE VALUE"
        )
        .execute(&db)
        .await
        .unwrap()
        .last_insert_rowid();

        let m = Medication::get(&db, result).await;

        let err = m.unwrap_err();

        assert!(
            matches!(
                err,
                ServiceError::DatabaseError(sqlx::Error::ColumnDecode { .. })
            ),
            "expected DatabaseError(ColumnDecode), but got {err:?}"
        );
    }
}
