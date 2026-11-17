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
    pub inventory: Option<f64>,
    pub aliases: Vec<String>,
}

pub(super) struct MedicationRow {
    pub(super) id: MedicationId,
    pub(super) name: String,
    pub(super) description: Option<String>,
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

    pub async fn get_dose_limits(
        db: &SqlitePool,
        patient_id: PatientId,
        medication_id: MedicationId,
    ) -> Result<DoseLimits, ServiceError> {
        let result = sqlx::query_scalar!(
            r#"
                SELECT dose_limits AS "dose_limits!: DoseLimits"
                FROM patients_medications
                WHERE patient_id = ? AND medication_id = ?
            "#,
            patient_id,
            medication_id
        )
        .fetch_optional(db)
        .await?;

        Ok(result.unwrap_or_default())
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
                    m.id AS "id!", m.name, m.inventory, m.description, m.aliases
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

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn decode_dose_limits_success(db: SqlitePool) {
        sqlx::query!(
            r#"
                INSERT INTO patients_medications (
                    patient_id, medication_id, reminder_cron_schedule, dose_limits
                )
                VALUES (1, 1, '', ?)
            "#,
            "1:2,3:4"
        )
        .execute(&db)
        .await
        .unwrap();

        let dose_limits = Medication::get_dose_limits(&db, PatientId(1), MedicationId(1))
            .await
            .unwrap();

        assert_eq!(
            &*dose_limits,
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

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn decode_dose_limits_failure(db: SqlitePool) {
        sqlx::query!(
            r#"
                INSERT INTO patients_medications (
                    patient_id, medication_id, reminder_cron_schedule, dose_limits
                )
                VALUES (1, 1, '', ?)
            "#,
            "NONSENSE VALUE"
        )
        .execute(&db)
        .await
        .unwrap();

        let err = Medication::get_dose_limits(&db, PatientId(1), MedicationId(1))
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                ServiceError::DatabaseError(sqlx::Error::ColumnDecode { .. })
            ),
            "expected DatabaseError(ColumnDecode), but got {err:?}"
        );
    }

    #[sqlx::test(migrations = false)]
    async fn per_patient_dose_limits_migration_copies_to_active_patients_with_fallback(
        db: SqlitePool,
    ) {
        const TARGET_VERSION: i64 = 20261117093000;

        let mut pre_migrator = sqlx::migrate!();
        pre_migrator
            .migrations
            .to_mut()
            .retain(|m| m.version < TARGET_VERSION);
        pre_migrator.run(&db).await.unwrap();

        // Seed patients: 1 Alice, 2 Bob, 3 Carol
        sqlx::raw_sql(
            r#"
                INSERT INTO patients (id, name) VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Carol');
                INSERT INTO medications (id, name, dose_limits) VALUES
                    (1, 'DosedMed', '4:1,24:4'),
                    (2, 'RemindedMed', '8:2'),
                    (3, 'UnusedMedWithLimits', '12:5'),
                    (4, 'NoLimitsMed', '');
                INSERT INTO doses (patient_id, medication_id, quantity, taken_at)
                    VALUES (1, 1, 1.0, '2025-01-01 08:00:00');
                INSERT INTO patients_medications (patient_id, medication_id, reminder_cron_schedule)
                    VALUES (2, 2, '0 0 8 * * *');
            "#,
        )
        .execute(&db)
        .await
        .unwrap();

        let mut migrator = sqlx::migrate!();
        migrator
            .migrations
            .to_mut()
            .retain(|m| m.version <= TARGET_VERSION);
        migrator.run(&db).await.unwrap();

        // DosedMed (1): only Alice (1) had a dose, so only Alice gets its limits
        assert_eq!(
            &*Medication::get_dose_limits(&db, PatientId(1), MedicationId(1))
                .await
                .unwrap(),
            [
                DoseLimit {
                    hours: 4,
                    amount: 1.0
                },
                DoseLimit {
                    hours: 24,
                    amount: 4.0
                }
            ]
        );
        assert!(
            Medication::get_dose_limits(&db, PatientId(2), MedicationId(1))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            Medication::get_dose_limits(&db, PatientId(3), MedicationId(1))
                .await
                .unwrap()
                .is_empty()
        );

        // RemindedMed (2): only Bob (2) had a patients_medications row, so only Bob gets its limits
        assert!(
            Medication::get_dose_limits(&db, PatientId(1), MedicationId(2))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            &*Medication::get_dose_limits(&db, PatientId(2), MedicationId(2))
                .await
                .unwrap(),
            [DoseLimit {
                hours: 8,
                amount: 2.0
            }]
        );

        // UnusedMedWithLimits (3): no doses or patients_medications rows -> fallback copies to all patients
        for pid in [PatientId(1), PatientId(2), PatientId(3)] {
            assert_eq!(
                &*Medication::get_dose_limits(&db, pid, MedicationId(3))
                    .await
                    .unwrap(),
                [DoseLimit {
                    hours: 12,
                    amount: 5.0
                }]
            );
        }

        // NoLimitsMed (4): no patient gets limits
        for pid in [PatientId(1), PatientId(2), PatientId(3)] {
            assert!(
                Medication::get_dose_limits(&db, pid, MedicationId(4))
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
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
