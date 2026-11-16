// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::Serialize;
use sqlx::SqlitePool;

use super::{Medication, MedicationRow};
use crate::{
    api::medication::DoseLimits,
    errors::ServiceError,
    ids::{PatientId, TelegramGroupId},
};

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Patient {
    pub id: PatientId,
    pub telegram_group_id: Option<TelegramGroupId>,
    pub name: String,
}

impl Patient {
    /// Fetches a patient by their ID from the database.
    pub async fn get(db: &SqlitePool, patient_id: PatientId) -> Result<Patient, ServiceError> {
        let res = sqlx::query_as!(
            Patient,
            r#"
                SELECT id, name, telegram_group_id AS "telegram_group_id: TelegramGroupId"
                FROM patients
                WHERE id = ?
            "#,
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
            r#"
                SELECT
                    id AS "id!", name, telegram_group_id AS "telegram_group_id: TelegramGroupId"
                FROM patients
                WHERE name = ?
            "#,
            patient_name
        )
        .fetch_optional(db)
        .await
    }

    /// Fetches all patients that have the given telegram_group_id configured
    pub async fn find_by_telegram_group_id(
        db: &SqlitePool,
        telegram_group_id: TelegramGroupId,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Patient,
            r#"
                SELECT
                    id AS "id!", name, telegram_group_id AS "telegram_group_id: TelegramGroupId"
                FROM patients
                WHERE telegram_group_id = ?
            "#,
            telegram_group_id
        )
        .fetch_all(db)
        .await
    }

    /// Medications for this patient filtered by whether `natural_input` is
    /// `enabled` (`false` includes medications with no `patients_medications` row).
    pub async fn get_natural_language_medications<'e, E>(
        &self,
        db: E,
        enabled: bool,
    ) -> Result<Vec<Medication>, ServiceError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        let result = sqlx::query_as!(
            MedicationRow,
            r#"
                SELECT
                    m.id AS "id!",
                    m.name,
                    m.inventory,
                    m.description,
                    m.dose_limits AS "dose_limits!: DoseLimits",
                    m.aliases
                FROM medications m
                    LEFT JOIN patients_medications pm
                        ON pm.medication_id = m.id AND pm.patient_id = ?
                WHERE COALESCE(pm.natural_input, false) = ?
            "#,
            self.id,
            enabled
        )
        .fetch_all(db)
        .await?;

        Ok(result.into_iter().map(Medication::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{natural_med_names, set_flag};
    use super::*;
    use crate::ids::MedicationId;

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn natural_language_medications_missing_row_means_disabled(db: SqlitePool) {
        assert_eq!(
            natural_med_names(&db, PatientId(1), true).await,
            Vec::<String>::new()
        );
        assert_eq!(
            natural_med_names(&db, PatientId(1), false).await,
            [
                "Amoxicillin",
                "Aspirin",
                "Ibuprofen",
                "Metformin",
                "Paracetamol",
            ]
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn natural_language_medications_respect_flag_and_patient(db: SqlitePool) {
        set_flag(&db, PatientId(1), MedicationId(1), true).await; // Alice: Aspirin enabled
        set_flag(&db, PatientId(1), MedicationId(2), false).await; // Alice: Ibuprofen disabled
        set_flag(&db, PatientId(2), MedicationId(3), true).await; // Bob: Paracetamol enabled
        set_flag(&db, PatientId(2), MedicationId(1), false).await; // Bob: Aspirin disabled

        assert_eq!(
            natural_med_names(&db, PatientId(1), true).await,
            ["Aspirin"]
        );
        assert_eq!(
            natural_med_names(&db, PatientId(1), false).await,
            ["Amoxicillin", "Ibuprofen", "Metformin", "Paracetamol"]
        );

        assert_eq!(
            natural_med_names(&db, PatientId(2), true).await,
            ["Paracetamol"]
        );
        assert_eq!(
            natural_med_names(&db, PatientId(2), false).await,
            ["Amoxicillin", "Aspirin", "Ibuprofen", "Metformin"]
        );
    }

    // cSpell: word אספירין Advil
    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn natural_language_medications_decode_aliases(db: SqlitePool) {
        sqlx::query!(
            "UPDATE medications SET aliases = ? WHERE id = ?",
            "Asa\nאספירין",
            1
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query!(
            "UPDATE medications SET aliases = ? WHERE id = ?",
            "Advil",
            2
        )
        .execute(&db)
        .await
        .unwrap();
        set_flag(&db, PatientId(1), MedicationId(1), true).await;

        let alice = Patient::get(&db, PatientId(1)).await.unwrap();
        let enabled = alice
            .get_natural_language_medications(&db, true)
            .await
            .unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "Aspirin");
        assert_eq!(enabled[0].aliases, ["Asa", "אספירין"]);

        let disabled = alice
            .get_natural_language_medications(&db, false)
            .await
            .unwrap();
        let ibuprofen = disabled.iter().find(|m| m.name == "Ibuprofen").unwrap();
        assert_eq!(ibuprofen.aliases, ["Advil"]);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn migration_defaults_aliases_empty_and_natural_input_false(db: SqlitePool) {
        let aspirin = Medication::get(&db, MedicationId(1)).await.unwrap();
        assert_eq!(aspirin.aliases, Vec::<String>::new());

        sqlx::query!(
            r#"
                INSERT INTO patients_medications (
                    patient_id, medication_id, reminder_cron_schedule
                )
                VALUES (?, ?, ?)
            "#,
            1,
            1,
            "0 0 8 * * *"
        )
        .execute(&db)
        .await
        .unwrap();

        sqlx::query!(
            r#"
                INSERT INTO doses (patient_id, medication_id, quantity, taken_at)
                VALUES (?, ?, ?, datetime('now'))
            "#,
            1,
            1,
            1.0
        )
        .execute(&db)
        .await
        .unwrap();

        assert_eq!(
            natural_med_names(&db, PatientId(1), true).await,
            Vec::<String>::new()
        );
    }
}
