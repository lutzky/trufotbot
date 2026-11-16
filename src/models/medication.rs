// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::Serialize;
use sqlx::SqlitePool;

use crate::{
    api::medication::DoseLimits,
    errors::ServiceError,
    ids::{MedicationId, PatientId},
};

#[derive(Serialize, Debug)]
pub struct Medication {
    pub id: MedicationId,
    pub name: String,
    pub description: Option<String>,
    pub dose_limits: DoseLimits,
    pub inventory: Option<f64>,
    pub aliases: Vec<String>,
}

pub(super) struct MedicationRow {
    pub(super) id: MedicationId,
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) dose_limits: DoseLimits,
    pub(super) inventory: Option<f64>,
    pub(super) aliases: String,
}

fn aliases_from_db(s: &str) -> Vec<String> {
    s.lines().map(str::to_string).collect()
}

pub fn aliases_to_db(aliases: &[String]) -> String {
    aliases.join("\n")
}

impl From<MedicationRow> for Medication {
    fn from(value: MedicationRow) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            dose_limits: value.dose_limits,
            inventory: value.inventory,
            aliases: aliases_from_db(&value.aliases),
        }
    }
}

impl Medication {
    pub async fn get(db: &SqlitePool, medication_id: MedicationId) -> Result<Self, ServiceError> {
        let result = sqlx::query_as!(
            MedicationRow,
            r#"
                SELECT
                    id,
                    name,
                    description,
                    dose_limits AS "dose_limits!: DoseLimits",
                    inventory,
                    aliases
                FROM medications
                WHERE id = ?
            "#,
            medication_id
        )
        .fetch_one(db)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => ServiceError::not_found("Medication not found"),
            e => ServiceError::DatabaseError(e),
        })?;

        Ok(result.into())
    }

    pub async fn find_by_name(
        db: &SqlitePool,
        medication_name: &str,
    ) -> Result<Option<Self>, ServiceError> {
        let res = sqlx::query_as!(
            MedicationRow,
            r#"
                SELECT
                    id AS "id!",
                    name,
                    description,
                    dose_limits AS "dose_limits!: DoseLimits",
                    inventory,
                    aliases
                FROM medications
                WHERE name = ?
            "#,
            medication_name
        )
        .fetch_optional(db)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => ServiceError::not_found("medication not found"),
            e => ServiceError::DatabaseError(e),
        })?;

        Ok(res.map(Medication::from))
    }

    pub async fn get_natural_input_neighbors<'e, E>(
        db: E,
        medication_id: MedicationId,
    ) -> Result<Vec<Medication>, ServiceError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        let result = sqlx::query_as!(
            MedicationRow,
            r#"
                SELECT DISTINCT
                    m.id AS "id!", m.name, m.inventory, m.description,
                    m.dose_limits AS "dose_limits!: DoseLimits", m.aliases
                FROM patients_medications mine
                    JOIN patients_medications other
                        ON
                            other.patient_id = mine.patient_id
                            AND other.natural_input = true
                            AND other.medication_id <> mine.medication_id
                    JOIN medications m ON m.id = other.medication_id
                WHERE
                    mine.medication_id = ?
                    AND mine.natural_input = true
            "#,
            medication_id
        )
        .fetch_all(db)
        .await?;
        Ok(result.into_iter().map(Medication::from).collect())
    }

    pub async fn latest_dosage(
        db: &SqlitePool,
        medication_id: MedicationId,
        patient_id: PatientId,
    ) -> Result<Option<f64>, ServiceError> {
        let result = sqlx::query!(
            r"
                SELECT quantity
                FROM doses
                WHERE
                    medication_id = ? AND
                    patient_id = ? AND
                    quantity > 0
                ORDER BY taken_at DESC
                LIMIT 1
            ",
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
    use super::super::test_helpers::set_flag;
    use super::*;

    use crate::api::medication::DoseLimit;

    #[sqlx::test]
    async fn decode_medication_success(db: SqlitePool) {
        let result = sqlx::query!(
            r#"
                INSERT INTO medications (name, dose_limits)
                VALUES (?, ?)
            "#,
            "good_limits",
            "1:2,3:4"
        )
        .execute(&db)
        .await
        .unwrap()
        .last_insert_rowid();

        let m = Medication::get(&db, MedicationId(result)).await.unwrap();

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
                INSERT INTO medications (name, dose_limits)
                VALUES (?, ?)
            "#,
            "bad limits",
            "NONSENSE VALUE"
        )
        .execute(&db)
        .await
        .unwrap()
        .last_insert_rowid();

        let m = Medication::get(&db, MedicationId(result)).await;

        let err = m.unwrap_err();

        assert!(
            matches!(
                err,
                ServiceError::DatabaseError(sqlx::Error::ColumnDecode { .. })
            ),
            "expected DatabaseError(ColumnDecode), but got {err:?}"
        );
    }

    async fn neighbor_names(db: &SqlitePool, medication: MedicationId) -> Vec<String> {
        let mut names: Vec<String> = Medication::get_natural_input_neighbors(db, medication)
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.name)
            .collect();
        names.sort();
        names
    }

    // Medications: 1 Aspirin, 2 Ibuprofen, 3 Paracetamol, 4 Amoxicillin. Patients: 1 Alice, 2 Bob.
    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn neighbors_are_medications_enabled_for_a_shared_patient(db: SqlitePool) {
        set_flag(&db, PatientId(1), MedicationId(1), true).await; // Alice: Aspirin
        set_flag(&db, PatientId(1), MedicationId(2), true).await; // Alice: Ibuprofen
        set_flag(&db, PatientId(2), MedicationId(1), true).await; // Bob: Aspirin
        set_flag(&db, PatientId(2), MedicationId(3), true).await; // Bob: Paracetamol

        assert_eq!(
            neighbor_names(&db, MedicationId(1)).await,
            ["Ibuprofen", "Paracetamol"],
            "Aspirin shares Alice with Ibuprofen and Bob with Paracetamol"
        );
        assert_eq!(neighbor_names(&db, MedicationId(2)).await, ["Aspirin"]);
        assert_eq!(neighbor_names(&db, MedicationId(3)).await, ["Aspirin"]);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn neighbors_exclude_the_medication_itself_and_duplicates(db: SqlitePool) {
        set_flag(&db, PatientId(1), MedicationId(1), true).await;
        set_flag(&db, PatientId(1), MedicationId(2), true).await;
        set_flag(&db, PatientId(2), MedicationId(1), true).await;
        set_flag(&db, PatientId(2), MedicationId(2), true).await;

        assert_eq!(neighbor_names(&db, MedicationId(1)).await, ["Ibuprofen"]);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn neighbors_ignore_disabled_medications_and_patients(db: SqlitePool) {
        set_flag(&db, PatientId(1), MedicationId(1), true).await;
        set_flag(&db, PatientId(1), MedicationId(2), false).await; // Ibuprofen is disabled for Alice
        set_flag(&db, PatientId(2), MedicationId(2), true).await; // Ibuprofen is enabled, but not alongside Aspirin
        set_flag(&db, PatientId(2), MedicationId(3), true).await;
        set_flag(&db, PatientId(1), MedicationId(4), false).await;

        assert_eq!(
            neighbor_names(&db, MedicationId(1)).await,
            Vec::<String>::new()
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn neighbors_of_a_disabled_medication_are_empty(db: SqlitePool) {
        set_flag(&db, PatientId(1), MedicationId(1), false).await;
        set_flag(&db, PatientId(1), MedicationId(2), true).await;

        assert_eq!(
            neighbor_names(&db, MedicationId(1)).await,
            Vec::<String>::new()
        );
    }

    // cSpell: word אספירין
    #[test]
    fn aliases_db_round_trip() {
        assert_eq!(aliases_to_db(&[]), "");
        assert_eq!(aliases_from_db(""), Vec::<String>::new());

        let single = vec!["Asa".to_string()];
        let single_db = aliases_to_db(&single);
        assert_eq!(single_db, "Asa");
        assert_eq!(aliases_from_db(&single_db), single);

        let multiple = vec!["Asa".to_string(), "אספירין".to_string()];
        let multiple_db = aliases_to_db(&multiple);
        assert_eq!(multiple_db, "Asa\nאספירין");
        assert_eq!(aliases_from_db(&multiple_db), multiple);
    }
}
