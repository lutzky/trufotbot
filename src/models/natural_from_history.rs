// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::Serialize;
use sqlx::SqlitePool;

use super::{Medication, MedicationRow, Patient};
use crate::{errors::ServiceError, ids::MedicationId, natural_input};

pub const DEFAULT_NATURAL_FROM_HISTORY_DAYS: u32 = 90;

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct EnabledMedication {
    pub id: MedicationId,
    pub name: String,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct SkippedMedication {
    pub id: MedicationId,
    pub name: String,
    pub reason: String,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct NaturalFromHistoryResult {
    pub enabled: Vec<EnabledMedication>,
    pub skipped: Vec<SkippedMedication>,
    pub already_enabled: usize,
}

impl Patient {
    /// Enables natural input for medications this patient took (with `quantity > 0`)
    /// in the last `days` days, ordered most-recently-dosed first.
    ///
    /// Medications that would collide with already-enabled medications (including
    /// ones enabled earlier in the same scan) are skipped and reported rather than
    /// returning an error. Existing `reminder_cron_schedule` entries are preserved.
    pub async fn enable_natural_input_from_history(
        &self,
        db: &SqlitePool,
        days: u32,
    ) -> Result<NaturalFromHistoryResult, ServiceError> {
        let mut tx = db.begin().await?;

        let mut currently_enabled = self
            .get_natural_language_medications(&mut *tx, true)
            .await?;
        let already_enabled = currently_enabled.len();

        let days_str = format!("-{days} days");
        let recent_rows = sqlx::query_as!(
            MedicationRow,
            r#"
                SELECT
                    m.id AS "id!",
                    m.name,
                    m.inventory,
                    m.description,
                    m.aliases
                FROM medications m
                    JOIN doses d ON d.medication_id = m.id
                WHERE
                    d.patient_id = ?
                    AND d.quantity > 0
                    AND d.taken_at >= date('now', ?)
                GROUP BY m.id
                ORDER BY max(datetime(d.taken_at)) DESC, m.id ASC
            "#,
            self.id,
            days_str
        )
        .fetch_all(&mut *tx)
        .await?;

        let mut enabled = Vec::new();
        let mut skipped = Vec::new();

        for med in recent_rows.into_iter().map(Medication::from) {
            if currently_enabled.iter().any(|e| e.id == med.id) {
                continue;
            }

            let collisions = natural_input::find_collisions(&med, &currently_enabled);
            if !collisions.is_empty() {
                let reason = collisions
                    .iter()
                    .map(|c| {
                        format!(
                            "Label {:?} collides with preexisting label {:?} of {:?}",
                            c.added_label, c.existing_label, c.existing.name
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                skipped.push(SkippedMedication {
                    id: med.id,
                    name: med.name,
                    reason,
                });
                continue;
            }

            sqlx::query!(
                r#"
                    INSERT INTO patients_medications (
                        patient_id, medication_id, reminder_cron_schedule, natural_input
                    )
                    VALUES (?, ?, '', true)
                    ON CONFLICT (patient_id, medication_id) DO UPDATE
                    SET natural_input = true
                "#,
                self.id,
                med.id
            )
            .execute(&mut *tx)
            .await?;

            enabled.push(EnabledMedication {
                id: med.id,
                name: med.name.clone(),
            });
            currently_enabled.push(med);
        }

        tx.commit().await?;

        Ok(NaturalFromHistoryResult {
            enabled,
            skipped,
            already_enabled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{natural_med_names, set_flag};
    use super::*;
    use crate::ids::PatientId;

    async fn add_dose_days_ago(
        db: &SqlitePool,
        patient: PatientId,
        medication: MedicationId,
        quantity: f64,
        days_ago: u32,
    ) {
        let offset = format!("-{days_ago} days");
        sqlx::query!(
            r#"
                INSERT INTO doses (patient_id, medication_id, quantity, taken_at)
                VALUES (?, ?, ?, datetime('now', ?))
            "#,
            patient,
            medication,
            quantity,
            offset
        )
        .execute(db)
        .await
        .unwrap();
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn enable_from_history_cutoff_zero_quantity_reminders_and_idempotency(db: SqlitePool) {
        // 1 Aspirin: dosed 30 days ago, has an existing reminders row with natural_input = false
        sqlx::query!(
            r#"
                INSERT INTO patients_medications (
                    patient_id, medication_id, reminder_cron_schedule, natural_input
                )
                VALUES (?, ?, ?, false)
            "#,
            1,
            1,
            "0 0 8 * * *"
        )
        .execute(&db)
        .await
        .unwrap();
        add_dose_days_ago(&db, PatientId(1), MedicationId(1), 1.0, 30).await;

        // 2 Ibuprofen: dosed 200 days ago (outside default 90-day cutoff)
        add_dose_days_ago(&db, PatientId(1), MedicationId(2), 1.0, 200).await;

        // 3 Paracetamol: only a zero-quantity (skipped) dose 10 days ago
        add_dose_days_ago(&db, PatientId(1), MedicationId(3), 0.0, 10).await;

        // 4 Amoxicillin: already enabled with reminder, no recent doses — must stay enabled & keep reminder
        sqlx::query!(
            r#"
                INSERT INTO patients_medications (
                    patient_id, medication_id, reminder_cron_schedule, natural_input
                )
                VALUES (?, ?, ?, true)
            "#,
            1,
            4,
            "0 0 20 * * *"
        )
        .execute(&db)
        .await
        .unwrap();

        let alice = Patient::get(&db, PatientId(1)).await.unwrap();
        let res = alice
            .enable_natural_input_from_history(&db, DEFAULT_NATURAL_FROM_HISTORY_DAYS)
            .await
            .unwrap();

        assert_eq!(
            res,
            NaturalFromHistoryResult {
                enabled: vec![EnabledMedication {
                    id: MedicationId(1),
                    name: "Aspirin".to_string(),
                }],
                skipped: vec![],
                already_enabled: 1,
            }
        );

        assert_eq!(
            natural_med_names(&db, PatientId(1), true).await,
            ["Amoxicillin", "Aspirin"]
        );

        let aspirin_schedule: String = sqlx::query_scalar!(
            "SELECT reminder_cron_schedule FROM patients_medications WHERE patient_id = 1 AND medication_id = 1"
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(aspirin_schedule, "0 0 8 * * *");

        let amoxicillin_schedule: String = sqlx::query_scalar!(
            "SELECT reminder_cron_schedule FROM patients_medications WHERE patient_id = 1 AND medication_id = 4"
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(amoxicillin_schedule, "0 0 20 * * *");

        // Second call with the same cutoff is idempotent
        let second = alice
            .enable_natural_input_from_history(&db, DEFAULT_NATURAL_FROM_HISTORY_DAYS)
            .await
            .unwrap();
        assert_eq!(
            second,
            NaturalFromHistoryResult {
                enabled: vec![],
                skipped: vec![],
                already_enabled: 2,
            }
        );

        // Passing a larger `days` window reaches the 200-day-old dose
        let extended = alice
            .enable_natural_input_from_history(&db, 250)
            .await
            .unwrap();
        assert_eq!(
            extended,
            NaturalFromHistoryResult {
                enabled: vec![EnabledMedication {
                    id: MedicationId(2),
                    name: "Ibuprofen".to_string(),
                }],
                skipped: vec![],
                already_enabled: 2,
            }
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn enable_from_history_colliding_pair_most_recent_wins(db: SqlitePool) {
        // Give Ibuprofen (2) and Paracetamol (3) a colliding alias
        sqlx::query!("UPDATE medications SET aliases = 'Painkiller' WHERE id IN (2, 3)")
            .execute(&db)
            .await
            .unwrap();

        add_dose_days_ago(&db, PatientId(1), MedicationId(2), 1.0, 20).await; // Ibuprofen: older
        add_dose_days_ago(&db, PatientId(1), MedicationId(3), 1.0, 10).await; // Paracetamol: more recent -> wins

        let alice = Patient::get(&db, PatientId(1)).await.unwrap();
        let res = alice
            .enable_natural_input_from_history(&db, DEFAULT_NATURAL_FROM_HISTORY_DAYS)
            .await
            .unwrap();

        assert_eq!(
            res.enabled,
            [EnabledMedication {
                id: MedicationId(3),
                name: "Paracetamol".to_string(),
            }]
        );
        assert_eq!(res.already_enabled, 0);
        assert_eq!(res.skipped.len(), 1);
        assert_eq!(res.skipped[0].id, MedicationId(2));
        assert_eq!(res.skipped[0].name, "Ibuprofen");
        assert!(
            res.skipped[0].reason.contains("Painkiller")
                && res.skipped[0].reason.contains("Paracetamol"),
            "unexpected skip reason: {}",
            res.skipped[0].reason
        );
        assert_eq!(
            natural_med_names(&db, PatientId(1), true).await,
            ["Paracetamol"]
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn enable_from_history_skips_collision_with_already_enabled(db: SqlitePool) {
        sqlx::query!("UPDATE medications SET aliases = 'Painkiller' WHERE id IN (1, 2)")
            .execute(&db)
            .await
            .unwrap();

        set_flag(&db, PatientId(1), MedicationId(1), true).await; // Aspirin already enabled
        add_dose_days_ago(&db, PatientId(1), MedicationId(2), 1.0, 5).await; // Ibuprofen recently dosed, collides with Aspirin

        let alice = Patient::get(&db, PatientId(1)).await.unwrap();
        let res = alice
            .enable_natural_input_from_history(&db, DEFAULT_NATURAL_FROM_HISTORY_DAYS)
            .await
            .unwrap();

        assert!(res.enabled.is_empty());
        assert_eq!(res.already_enabled, 1);
        assert_eq!(res.skipped.len(), 1);
        assert_eq!(res.skipped[0].id, MedicationId(2));
        assert_eq!(res.skipped[0].name, "Ibuprofen");
        assert!(
            res.skipped[0].reason.contains("Painkiller")
                && res.skipped[0].reason.contains("Aspirin"),
            "unexpected skip reason: {}",
            res.skipped[0].reason
        );
        assert_eq!(
            natural_med_names(&db, PatientId(1), true).await,
            ["Aspirin"]
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn enable_from_history_isolates_patients(db: SqlitePool) {
        sqlx::query!("UPDATE medications SET aliases = 'Painkiller' WHERE id IN (1, 2)")
            .execute(&db)
            .await
            .unwrap();

        // Bob (2) has Aspirin enabled and a recent dose of Metformin
        set_flag(&db, PatientId(2), MedicationId(1), true).await;
        add_dose_days_ago(&db, PatientId(2), MedicationId(5), 1.0, 5).await;

        // Alice (1) has a recent dose of Ibuprofen (same alias as Bob's Aspirin)
        add_dose_days_ago(&db, PatientId(1), MedicationId(2), 1.0, 5).await;

        let alice = Patient::get(&db, PatientId(1)).await.unwrap();
        let res = alice
            .enable_natural_input_from_history(&db, DEFAULT_NATURAL_FROM_HISTORY_DAYS)
            .await
            .unwrap();

        assert_eq!(
            res.enabled,
            [EnabledMedication {
                id: MedicationId(2),
                name: "Ibuprofen".to_string(),
            }]
        );
        assert_eq!(res.skipped, []);
        assert_eq!(res.already_enabled, 0);

        // Alice only gets Ibuprofen; Bob's flags remain untouched (only Aspirin)
        assert_eq!(
            natural_med_names(&db, PatientId(1), true).await,
            ["Ibuprofen"]
        );
        assert_eq!(
            natural_med_names(&db, PatientId(2), true).await,
            ["Aspirin"]
        );
    }
}
