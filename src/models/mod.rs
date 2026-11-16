// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

mod medication;
mod natural_from_history;
mod patient;

pub use medication::*;
pub use natural_from_history::*;
pub use patient::*;

#[cfg(test)]
mod test_helpers {
    use sqlx::SqlitePool;

    use super::Patient;
    use crate::ids::{MedicationId, PatientId};

    pub(super) async fn set_flag(
        db: &SqlitePool,
        patient: PatientId,
        medication: MedicationId,
        natural_input: bool,
    ) {
        sqlx::query!(
            r#"
                INSERT INTO patients_medications (
                    patient_id, medication_id, reminder_cron_schedule, natural_input
                )
                VALUES (?, ?, '', ?)
            "#,
            patient,
            medication,
            natural_input
        )
        .execute(db)
        .await
        .unwrap();
    }

    pub(super) async fn natural_med_names(
        db: &SqlitePool,
        patient: PatientId,
        enabled: bool,
    ) -> Vec<String> {
        let p = Patient::get(db, patient).await.unwrap();
        let mut names: Vec<String> = p
            .get_natural_language_medications(db, enabled)
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.name)
            .collect();
        names.sort();
        names
    }
}
