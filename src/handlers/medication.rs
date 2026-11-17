// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::{
    api::{
        requests::{PatientMedicationCreateRequest, PatientMedicationUpdateRequest},
        responses::MedicationCreateResponse,
    },
    ids::{MedicationId, PatientId},
    models::Medication,
    natural_input,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    errors::ServiceError, models, reminder_scheduler::ReminderScheduler, storage::Storage,
};

use super::reminders;

pub const UTOIPA_TAG: &str = "medication";

#[utoipa::path(
    delete,
    path = "/api/medications/{id}",
    operation_id = "medication_delete",
    summary = "Delete a medication",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Medication deleted successfully"),
        (status = 404, description = "Medication not found"),
    ),
    params(
        ("id" = i32, Path, description = "Medication ID"),
    )
)]
pub async fn delete(
    State(storage): State<Storage>,
    State(reminder_scheduler): State<ReminderScheduler>,
    Path(medication_id): Path<MedicationId>,
) -> Result<(), ServiceError> {
    let mut tx = storage.pool.begin().await?;

    sqlx::query!(
        r#"
            DELETE FROM doses
            WHERE medication_id = ?
        "#,
        medication_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
            DELETE FROM patients_medications
            WHERE medication_id = ?
        "#,
        medication_id
    )
    .execute(&mut *tx)
    .await?;

    let result = sqlx::query!(
        r#"
            DELETE FROM medications
            WHERE id = ?
        "#,
        medication_id
    )
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ServiceError::not_found("Medication not found"));
    }

    tx.commit().await?;

    reminder_scheduler.remove_medication(medication_id).await;

    Ok(())
}

#[utoipa::path(
    put,
    path = "/api/patients/{patient_id}/medications/{medication_id}",
    summary = "Update a medication",
    operation_id = "medication_update",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Medication updated successfully"),
        (status = 404, description = "Medication not found"),
    ),
    request_body = PatientMedicationUpdateRequest,
    params(
        ("patient_id" = i32, Path, description = "Patient ID"),
        ("medication_id" = i32, Path, description = "Medication ID"),
    )
)]
pub async fn update(
    State(storage): State<Storage>,
    State(reminder_scheduler): State<ReminderScheduler>,
    Path((patient_id, medication_id)): Path<(PatientId, MedicationId)>,
    Json(payload): Json<PatientMedicationUpdateRequest>,
) -> Result<(), ServiceError> {
    let cleaned_aliases =
        natural_input::clean_aliases(&payload.medication.name, &payload.medication.aliases)
            .map_err(|err| ServiceError::bad_request(err.to_string()))?;
    let aliases = models::aliases_to_db(&cleaned_aliases);

    let mut tx = storage.pool.begin().await?;
    let result = sqlx::query!(
        r#"
            UPDATE medications
            SET
                name = ?,
                description = ?,
                inventory = ?,
                aliases = ?
            WHERE id = ?
        "#,
        payload.medication.name,
        payload.medication.description,
        payload.medication.inventory,
        aliases,
        medication_id
    )
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ServiceError::not_found("Medication not found"));
    }

    sqlx::query!(
        r#"
            INSERT INTO patients_medications (
                patient_id, medication_id, reminder_cron_schedule, dose_limits
            )
            VALUES (?, ?, '', ?)
            ON CONFLICT (patient_id, medication_id) DO UPDATE
            SET dose_limits = excluded.dose_limits
        "#,
        patient_id,
        medication_id,
        payload.dose_limits,
    )
    .execute(&mut *tx)
    .await?;

    if let Some(natural_input) = payload.natural_input {
        sqlx::query!(
            r#"
                INSERT INTO patients_medications (
                    patient_id, medication_id, reminder_cron_schedule, natural_input
                )
                VALUES (?, ?, '', ?)
                ON CONFLICT (patient_id, medication_id) DO UPDATE
                SET natural_input = excluded.natural_input
            "#,
            patient_id,
            medication_id,
            natural_input,
        )
        .execute(&mut *tx)
        .await?;
    }

    // We convert `payload.medication` from a PatientMedicationCreateRequest to a Medication so
    // that it has the same type as elements of neighbors.
    let proposed_updated_medication = Medication {
        id: medication_id,
        name: payload.medication.name,
        description: Default::default(), // Not used
        inventory: None,                 // Not used
        aliases: cleaned_aliases,
    };
    let neighbors = Medication::get_natural_input_neighbors(&mut *tx, medication_id).await?;
    let collisions = natural_input::find_collisions(&proposed_updated_medication, &neighbors);

    if !collisions.is_empty() {
        let collisions_descriptions: Vec<String> = collisions
            .iter()
            .map(|c| {
                format!(
                    "Label {:?} collides with preexisting label {:?} of {:?}",
                    c.added_label, c.existing_label, c.existing.name
                )
            })
            .collect();
        return Err(ServiceError::bad_request(
            collisions_descriptions.join(", "),
        ));
    }

    reminders::set_in_db(&mut *tx, patient_id, medication_id, &payload.reminders).await?;

    tx.commit().await?;

    reminder_scheduler
        .set_reminders(patient_id, medication_id, &payload.reminders.cron_schedules)
        .await?;

    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/medications",
    operation_id = "medication_create",
    summary = "Create a new medication",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Medication created successfully", body = MedicationCreateResponse),
    ),
    request_body = PatientMedicationCreateRequest,
)]
pub async fn create(
    State(storage): State<Storage>,
    Json(payload): Json<PatientMedicationCreateRequest>,
) -> Result<(StatusCode, Json<MedicationCreateResponse>), ServiceError> {
    let result = sqlx::query!(
        r#"INSERT INTO medications (name, description) VALUES (?, ?)"#,
        payload.name,
        payload.description,
    )
    .execute(&storage.pool)
    .await?
    .last_insert_rowid();
    Ok((
        StatusCode::CREATED,
        Json(MedicationCreateResponse {
            id: MedicationId(result),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use sqlx::SqlitePool;

    use super::*;
    use crate::{
        api::{
            medication::{DoseLimit, DoseLimits},
            patient::Reminders,
        },
        app_state::{AppState, Config},
        messenger::fake_sender::FakeSender,
    };

    // From src/fixtures/medications.sql and src/fixtures/patients.sql
    const ASPIRIN: MedicationId = MedicationId(1);
    const IBUPROFEN: MedicationId = MedicationId(2);
    const ALICE: PatientId = PatientId(1);
    const BOB: PatientId = PatientId(2);

    async fn setup(db: SqlitePool) -> AppState {
        let messenger = Arc::new(FakeSender::new()).into();
        AppState::new(db, messenger, Arc::new(Config::load().unwrap()))
            .await
            .unwrap()
    }

    fn request(
        name: &str,
        aliases: &[&str],
        natural_input: Option<bool>,
    ) -> PatientMedicationUpdateRequest {
        PatientMedicationUpdateRequest {
            medication: PatientMedicationCreateRequest {
                name: name.to_string(),
                description: None,
                inventory: None,
                aliases: aliases.iter().map(|a| a.to_string()).collect(),
            },
            dose_limits: DoseLimits::default(),
            reminders: Reminders {
                cron_schedules: vec![],
            },
            natural_input,
        }
    }

    async fn do_update(
        app: &AppState,
        patient_id: PatientId,
        medication_id: MedicationId,
        payload: PatientMedicationUpdateRequest,
    ) -> Result<(), ServiceError> {
        update(
            State(app.storage.clone()),
            State(app.reminder_scheduler.clone()),
            Path((patient_id, medication_id)),
            Json(payload),
        )
        .await
    }

    /// `None` if there is no `patients_medications` row at all.
    async fn get_natural_input_flag(
        db: &SqlitePool,
        patient_id: PatientId,
        medication_id: MedicationId,
    ) -> Option<bool> {
        sqlx::query_scalar!(
            "SELECT natural_input FROM patients_medications WHERE patient_id = ? AND medication_id = ?",
            patient_id,
            medication_id
        )
        .fetch_optional(db)
        .await
        .unwrap()
        .map(|flag| flag != 0)
    }

    async fn get_reminder_cron(
        db: &SqlitePool,
        patient_id: PatientId,
        medication_id: MedicationId,
    ) -> Option<String> {
        sqlx::query_scalar!(
            "SELECT reminder_cron_schedule FROM patients_medications WHERE patient_id = ? AND medication_id = ?",
            patient_id,
            medication_id
        )
        .fetch_optional(db)
        .await
        .unwrap()
    }

    async fn enable_natural_input(
        db: &SqlitePool,
        patient_id: PatientId,
        medication_id: MedicationId,
    ) {
        sqlx::query!(
            r#"
                INSERT INTO patients_medications (
                    patient_id, medication_id, reminder_cron_schedule, natural_input
                )
                VALUES (?, ?, '', 1)
                ON CONFLICT (patient_id, medication_id) DO UPDATE SET natural_input = 1
            "#,
            patient_id,
            medication_id
        )
        .execute(db)
        .await
        .unwrap();
    }

    async fn set_aliases(db: &SqlitePool, medication_id: MedicationId, aliases: &[&str]) {
        let aliases: Vec<String> = aliases.iter().map(|a| a.to_string()).collect();
        let aliases = models::aliases_to_db(&aliases);
        sqlx::query!(
            "UPDATE medications SET aliases = ? WHERE id = ?",
            aliases,
            medication_id
        )
        .execute(db)
        .await
        .unwrap();
    }

    fn assert_bad_request_message(result: Result<(), ServiceError>) -> String {
        match result {
            Err(ServiceError::BadRequest(message)) => message,
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    // cSpell: word אספירין
    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_writes_name_and_aliases(db: SqlitePool) {
        let app = setup(db.clone()).await;

        do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &["Asa", "אספירין"], None),
        )
        .await
        .unwrap();

        let medication = Medication::get(&db, ASPIRIN).await.unwrap();
        assert_eq!(medication.aliases, ["Asa", "אספירין"]);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_invalid_alias_is_rejected_and_changes_nothing(db: SqlitePool) {
        let app = setup(db.clone()).await;

        let result = do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Renamed", &["123"], Some(true)),
        )
        .await;

        assert_bad_request_message(result);
        let medication = Medication::get(&db, ASPIRIN).await.unwrap();
        assert_eq!(medication.name, "Aspirin");
        assert_eq!(get_natural_input_flag(&db, ALICE, ASPIRIN).await, None);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_missing_medication_is_not_found_and_writes_no_flag(db: SqlitePool) {
        let app = setup(db.clone()).await;

        let result = do_update(
            &app,
            ALICE,
            MedicationId(999),
            request("Nope", &[], Some(true)),
        )
        .await;

        assert!(
            matches!(result, Err(ServiceError::NotFound(_))),
            "expected NotFound, got {result:?}"
        );
        assert_eq!(
            get_natural_input_flag(&db, ALICE, MedicationId(999)).await,
            None
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_enabling_without_existing_row_creates_it(db: SqlitePool) {
        let app = setup(db.clone()).await;
        assert_eq!(get_natural_input_flag(&db, ALICE, ASPIRIN).await, None);

        do_update(&app, ALICE, ASPIRIN, request("Aspirin", &[], Some(true)))
            .await
            .unwrap();

        assert_eq!(
            get_natural_input_flag(&db, ALICE, ASPIRIN).await,
            Some(true)
        );
        assert_eq!(
            get_reminder_cron(&db, ALICE, ASPIRIN).await,
            Some(String::new())
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_without_natural_input_leaves_flag_unchanged(db: SqlitePool) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, ASPIRIN).await;

        do_update(&app, ALICE, ASPIRIN, request("Aspirin", &[], None))
            .await
            .unwrap();

        assert_eq!(
            get_natural_input_flag(&db, ALICE, ASPIRIN).await,
            Some(true)
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_without_natural_input_does_not_enable(db: SqlitePool) {
        let app = setup(db.clone()).await;

        do_update(&app, ALICE, ASPIRIN, request("Aspirin", &[], None))
            .await
            .unwrap();

        // `reminders::set` creates the row, but the flag must stay off.
        assert_eq!(
            get_natural_input_flag(&db, ALICE, ASPIRIN).await,
            Some(false)
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_can_disable_natural_input(db: SqlitePool) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, ASPIRIN).await;

        do_update(&app, ALICE, ASPIRIN, request("Aspirin", &[], Some(false)))
            .await
            .unwrap();

        assert_eq!(
            get_natural_input_flag(&db, ALICE, ASPIRIN).await,
            Some(false)
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_natural_input_is_per_patient(db: SqlitePool) {
        let app = setup(db.clone()).await;

        do_update(&app, ALICE, ASPIRIN, request("Aspirin", &[], Some(true)))
            .await
            .unwrap();

        assert_eq!(
            get_natural_input_flag(&db, ALICE, ASPIRIN).await,
            Some(true)
        );
        assert_eq!(get_natural_input_flag(&db, BOB, ASPIRIN).await, None);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_reminders_do_not_clobber_natural_input(db: SqlitePool) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, ASPIRIN).await;

        let mut payload = request("Aspirin", &[], None);
        payload.reminders.cron_schedules = vec!["0 0 8 * * *".to_string()];
        do_update(&app, ALICE, ASPIRIN, payload).await.unwrap();

        assert_eq!(
            get_natural_input_flag(&db, ALICE, ASPIRIN).await,
            Some(true)
        );
        assert_eq!(
            get_reminder_cron(&db, ALICE, ASPIRIN).await.as_deref(),
            Some("0 0 8 * * *")
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_enabling_into_collision_is_rejected_and_rolled_back(db: SqlitePool) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, IBUPROFEN).await;
        set_aliases(&db, IBUPROFEN, &["Pain"]).await;

        let result = do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &["pain"], Some(true)),
        )
        .await;

        let message = assert_bad_request_message(result);
        assert!(
            message.contains("pain") && message.contains("Ibuprofen"),
            "message should name the colliding label and medication: {message}"
        );
        assert_eq!(get_natural_input_flag(&db, ALICE, ASPIRIN).await, None);
        let aspirin = Medication::get(&db, ASPIRIN).await.unwrap();
        assert!(aspirin.aliases.is_empty(), "aliases were not rolled back");
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_enabling_into_collision_matches_reordered_tokens(db: SqlitePool) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, IBUPROFEN).await;
        set_aliases(&db, IBUPROFEN, &["Pain Killer"]).await;

        let result = do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &["killer PAIN"], Some(true)),
        )
        .await;

        assert_bad_request_message(result);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_renaming_enabled_medication_into_collision_is_rejected(db: SqlitePool) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, ASPIRIN).await;
        enable_natural_input(&db, ALICE, IBUPROFEN).await;

        let result = do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &["ibuprofen"], None),
        )
        .await;

        assert_bad_request_message(result);
        let aspirin = Medication::get(&db, ASPIRIN).await.unwrap();
        assert!(aspirin.aliases.is_empty(), "aliases were not rolled back");
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_rename_collides_with_medication_enabled_for_another_patient(db: SqlitePool) {
        // Aspirin is enabled for Alice and Bob, Ibuprofen only for Bob. Editing Aspirin through
        // Alice's page must still be checked against Bob's candidates.
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, ASPIRIN).await;
        enable_natural_input(&db, BOB, ASPIRIN).await;
        enable_natural_input(&db, BOB, IBUPROFEN).await;

        let result = do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &["Ibuprofen"], None),
        )
        .await;

        assert_bad_request_message(result);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_does_not_collide_with_itself(db: SqlitePool) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, ASPIRIN).await;
        set_aliases(&db, ASPIRIN, &["Asa"]).await;

        do_update(&app, ALICE, ASPIRIN, request("Aspirin", &["Asa"], None))
            .await
            .unwrap();
        do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &["Asa"], Some(true)),
        )
        .await
        .unwrap();
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_collision_with_medication_enabled_only_for_other_patient_is_allowed(
        db: SqlitePool,
    ) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, BOB, IBUPROFEN).await;

        do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &["Ibuprofen"], Some(true)),
        )
        .await
        .unwrap();

        assert_eq!(
            get_natural_input_flag(&db, ALICE, ASPIRIN).await,
            Some(true)
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_collision_with_disabled_medication_is_allowed(db: SqlitePool) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, IBUPROFEN).await;
        do_update(
            &app,
            ALICE,
            IBUPROFEN,
            request("Ibuprofen", &[], Some(false)),
        )
        .await
        .unwrap();

        do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &["Ibuprofen"], Some(true)),
        )
        .await
        .unwrap();
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_not_enabled_anywhere_is_not_checked(db: SqlitePool) {
        let app = setup(db.clone()).await;
        enable_natural_input(&db, ALICE, IBUPROFEN).await;

        do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &["Ibuprofen"], None),
        )
        .await
        .unwrap();

        let aspirin = Medication::get(&db, ASPIRIN).await.unwrap();
        assert_eq!(aspirin.aliases, ["Ibuprofen"]);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_rejects_separator_characters_in_aliases(db: SqlitePool) {
        let app = setup(db.clone()).await;

        for bad in ["a\nb", "Asa\n", "a\rb", "a, b"] {
            let result =
                do_update(&app, ALICE, ASPIRIN, request("Renamed", &[bad], Some(true))).await;

            assert_bad_request_message(result);
            let medication = Medication::get(&db, ASPIRIN).await.unwrap();
            assert_eq!(medication.name, "Aspirin");
            assert!(medication.aliases.is_empty());
            assert_eq!(get_natural_input_flag(&db, ALICE, ASPIRIN).await, None);
        }
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_normalizes_and_deduplicates_aliases_on_save(db: SqlitePool) {
        let app = setup(db.clone()).await;

        do_update(
            &app,
            ALICE,
            ASPIRIN,
            request(
                "Aspirin",
                &["  Asa  ", "", "   ", "asa", "Aspirin", "אספירין"],
                None,
            ),
        )
        .await
        .unwrap();

        let aspirin = Medication::get(&db, ASPIRIN).await.unwrap();
        assert_eq!(aspirin.aliases, ["Asa", "אספירין"]);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_rejects_more_than_twenty_aliases(db: SqlitePool) {
        let app = setup(db.clone()).await;

        let twenty: Vec<String> = (0..20)
            .map(|i| format!("alias-{}", (b'a' + i) as char))
            .collect();
        let twenty_refs: Vec<&str> = twenty.iter().map(String::as_str).collect();
        do_update(&app, ALICE, ASPIRIN, request("Aspirin", &twenty_refs, None))
            .await
            .unwrap();

        let aspirin = Medication::get(&db, ASPIRIN).await.unwrap();
        assert_eq!(aspirin.aliases, twenty);

        let twenty_one: Vec<String> = (0..21)
            .map(|i| format!("alias-{}", (b'a' + i) as char))
            .collect();
        let twenty_one_refs: Vec<&str> = twenty_one.iter().map(String::as_str).collect();
        let result = do_update(
            &app,
            ALICE,
            ASPIRIN,
            request("Aspirin", &twenty_one_refs, None),
        )
        .await;
        assert_bad_request_message(result);

        let aspirin = Medication::get(&db, ASPIRIN).await.unwrap();
        assert_eq!(aspirin.aliases, twenty);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_invalid_reminder_is_rejected_and_rolled_back(db: SqlitePool) {
        let app = setup(db.clone()).await;

        let mut payload = request("Renamed", &["Asa"], Some(true));
        payload.reminders.cron_schedules = vec!["not a cron schedule".to_string()];

        let result = do_update(&app, ALICE, ASPIRIN, payload).await;

        assert_bad_request_message(result);
        let aspirin = Medication::get(&db, ASPIRIN).await.unwrap();
        assert_eq!(aspirin.name, "Aspirin");
        assert!(aspirin.aliases.is_empty(), "aliases were not rolled back");
        assert_eq!(get_natural_input_flag(&db, ALICE, ASPIRIN).await, None);
        assert_eq!(get_reminder_cron(&db, ALICE, ASPIRIN).await, None);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_reminder_db_error_rolls_back_medication_changes(db: SqlitePool) {
        let app = setup(db.clone()).await;

        let result = do_update(
            &app,
            PatientId(999),
            ASPIRIN,
            request("Renamed", &["Asa"], None),
        )
        .await;

        assert!(
            matches!(result, Err(ServiceError::DatabaseError(_))),
            "expected DatabaseError, got {result:?}"
        );
        let aspirin = Medication::get(&db, ASPIRIN).await.unwrap();
        assert_eq!(aspirin.name, "Aspirin");
        assert!(aspirin.aliases.is_empty(), "aliases were not rolled back");
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn update_dose_limits_are_per_patient_and_preserve_other_settings(db: SqlitePool) {
        let app = setup(db.clone()).await;

        let mut alice_payload = request("Aspirin", &[], Some(true));
        alice_payload.reminders.cron_schedules = vec!["0 0 8 * * *".to_string()];
        alice_payload.dose_limits = DoseLimits(vec![
            DoseLimit {
                hours: 4,
                amount: 1.0,
            },
            DoseLimit {
                hours: 24,
                amount: 4.0,
            },
        ]);
        do_update(&app, ALICE, ASPIRIN, alice_payload)
            .await
            .unwrap();

        let mut bob_payload = request("Aspirin", &[], None);
        bob_payload.dose_limits = DoseLimits(vec![DoseLimit {
            hours: 6,
            amount: 2.0,
        }]);
        do_update(&app, BOB, ASPIRIN, bob_payload).await.unwrap();

        assert_eq!(
            &*Medication::get_dose_limits(&db, ALICE, ASPIRIN)
                .await
                .unwrap(),
            [
                DoseLimit {
                    hours: 4,
                    amount: 1.0,
                },
                DoseLimit {
                    hours: 24,
                    amount: 4.0,
                },
            ]
        );
        assert_eq!(
            &*Medication::get_dose_limits(&db, BOB, ASPIRIN)
                .await
                .unwrap(),
            [DoseLimit {
                hours: 6,
                amount: 2.0,
            }]
        );

        // Updating Alice's dose_limits again without natural_input/reminders keeps Alice's flags
        // and leaves Bob's dose_limits untouched.
        let mut alice_updated = request("Aspirin", &[], None);
        alice_updated.reminders.cron_schedules = vec!["0 0 8 * * *".to_string()];
        alice_updated.dose_limits = DoseLimits(vec![DoseLimit {
            hours: 12,
            amount: 3.0,
        }]);
        do_update(&app, ALICE, ASPIRIN, alice_updated)
            .await
            .unwrap();

        assert_eq!(
            &*Medication::get_dose_limits(&db, ALICE, ASPIRIN)
                .await
                .unwrap(),
            [DoseLimit {
                hours: 12,
                amount: 3.0,
            }]
        );
        assert_eq!(
            get_natural_input_flag(&db, ALICE, ASPIRIN).await,
            Some(true)
        );
        assert_eq!(
            get_reminder_cron(&db, ALICE, ASPIRIN).await.as_deref(),
            Some("0 0 8 * * *")
        );
        assert_eq!(
            &*Medication::get_dose_limits(&db, BOB, ASPIRIN)
                .await
                .unwrap(),
            [DoseLimit {
                hours: 6,
                amount: 2.0,
            }]
        );
    }
}
