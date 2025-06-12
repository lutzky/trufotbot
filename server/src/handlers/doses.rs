use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use shared::{
    api::{
        dose::{self, CreateDose},
        requests::{CreateDoseQueryParams, PatientMedicationCreateRequest},
        responses,
    },
    time,
};
use teloxide::utils::markdown;

use crate::{
    messenger::Messenger, models::Medication, next_doses::get_next_doses, storage::Storage,
};

use super::reminders;

/// Record (create) a new dose
pub async fn record(
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    Query(CreateDoseQueryParams {
        reminder_message_id,
    }): Query<CreateDoseQueryParams>,
    State(storage): State<Storage>,
    State(messenger): State<Messenger>,
    Json(payload): Json<dose::CreateDose>,
) -> Result<StatusCode, (StatusCode, String)> {
    let internal_server_error = (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to record intake".to_string(),
    );
    let patient = storage.get_patient(patient_id).await?;
    let medication = Medication::get(&storage.pool, medication_id)
        .await
        .map_err(|e| {
            log::error!("Database error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to record intake".to_string(),
            )
        })?;
    let Some(medication) = medication else {
        return Err((StatusCode::NOT_FOUND, "Medication not found".to_string()));
    };

    let mut tx = storage.pool.begin().await.map_err(|e| {
        log::error!("Failed to create transaction: {e}");
        internal_server_error.clone()
    })?;

    sqlx::query!(
        r#"
        INSERT INTO doses (patient_id, medication_id, quantity, taken_at, noted_by_user)
        VALUES (?, ?, ?, ?, ?)
        "#,
        patient_id,
        medication_id,
        payload.quantity,
        payload.taken_at,
        payload.noted_by_user,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Database error creating dose: {e}");
        internal_server_error.clone()
    })?;

    if let Some(inventory) = medication.inventory {
        let new_inventory = inventory - payload.quantity;

        sqlx::query!(
            r#"
            UPDATE medications
            SET inventory = ?
            WHERE id = ?
            "#,
            new_inventory,
            medication_id
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            log::error!("Database error updating inventory: {e}");
            internal_server_error.clone()
        })?;
    }

    tx.commit().await.map_err(|e| {
        log::error!("Database error comitting create-dose transaction: {e}");
        internal_server_error.clone()
    })?;

    let base_msg = dose_message(&payload, &patient.name, &medication.name);

    if let Some(reminder_message_id) = reminder_message_id {
        messenger
            .edit(
                &patient,
                reminder_message_id,
                markdown::escape(&format!("✅ {base_msg}")),
            )
            .await?;
    } else {
        messenger
            .send(&patient, markdown::escape(&base_msg))
            .await?;
    }

    Ok(StatusCode::CREATED)
}

fn dose_message(payload: &CreateDose, patient_name: &str, medication_name: &str) -> String {
    let giver_name = match &payload.noted_by_user {
        None => "Someone",
        Some(name) => name,
    };

    let who_gave_whom = match (patient_name == giver_name, payload.quantity == 0.0) {
        (true, false) => format!("{patient_name} took"),
        (true, true) => format!("{patient_name} decided to skip"),
        (false, false) => format!("{giver_name} gave {patient_name}"),
        (false, true) => format!("{giver_name} decided to skip giving {patient_name}"),
    };

    let medication = format!("{medication_name} ({})", payload.quantity);
    let when = format!(
        "{} ({})",
        time::time_ago(&payload.taken_at),
        time::local_display(&payload.taken_at),
    );

    let who_gave_whom = markdown::escape(&who_gave_whom);

    format!("{who_gave_whom} {medication} {when}")
}

pub async fn list(
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    State(storage): State<Storage>,
) -> Result<Json<responses::PatientGetDosesResponse>, (StatusCode, String)> {
    let patient = storage.get_patient(patient_id).await?;
    let medication = storage.get_medication(medication_id).await?;

    let doses = sqlx::query!(
        r#"
        SELECT
            d.id,
            d.taken_at,
            d.quantity,
            d.noted_by_user,
            m.name AS medication_name
        FROM doses d
        JOIN medications m ON d.medication_id = m.id
        WHERE d.patient_id = ? AND d.medication_id = ?
        ORDER BY d.taken_at DESC
        "#,
        patient_id,
        medication_id
    )
    .map(|row| {
        let taken_at: chrono::NaiveDateTime = row.taken_at;
        let quantity: f64 = row.quantity;
        let noted_by_user: Option<String> = row.noted_by_user;

        dose::Dose {
            id: row.id,
            data: dose::CreateDose {
                quantity,
                taken_at: taken_at.and_utc(),
                noted_by_user,
            },
        }
    })
    .fetch_all(&storage.pool)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch doses".to_string(),
        )
    })?;

    let reminders = reminders::get(State(storage.clone()), Path((patient_id, medication_id)))
        .await
        .inspect_err(|e| {
            log::error!("Failed to fetch reminders: {e:?}");
        })?
        .0;

    let next_doses = get_next_doses(&storage, patient_id, medication_id, &medication.dose_limits)
        .await
        .map_err(|e| {
            log::error!("Failed to get next doses: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to calculate next dose".to_string(),
            )
        })?;

    Ok(Json(responses::PatientGetDosesResponse {
        patient_name: patient.name,
        medication: PatientMedicationCreateRequest {
            name: medication.name,
            description: medication.description,
            dose_limits: medication.dose_limits,
            inventory: medication.inventory,
        },
        doses,
        reminders,
        next_doses,
    }))
}

pub async fn get(
    Path((patient_id, medication_id, dose_id)): Path<(i64, i64, i64)>,
    State(storage): State<Storage>,
) -> Result<Json<responses::GetDoseResponse>, (StatusCode, String)> {
    let patient = storage.get_patient(patient_id).await?;
    let medication = storage.get_medication(medication_id).await?;

    let dose = sqlx::query!(
        r#"
        SELECT
            d.id,
            d.taken_at,
            d.quantity,
            d.noted_by_user,
            m.name AS medication_name
        FROM doses d
        JOIN medications m ON d.medication_id = m.id
        WHERE d.patient_id = ? AND d.medication_id = ? AND d.id = ?
        "#,
        patient_id,
        medication_id,
        dose_id,
    )
    .map(|row| {
        let taken_at: chrono::NaiveDateTime = row.taken_at;
        let quantity: f64 = row.quantity;
        let noted_by_user: Option<String> = row.noted_by_user;

        dose::Dose {
            id: row.id,
            data: dose::CreateDose {
                quantity,
                taken_at: taken_at.and_utc(),
                noted_by_user,
            },
        }
    })
    .fetch_one(&storage.pool)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch dose".to_string(),
        )
    })?;

    Ok(Json(responses::GetDoseResponse {
        patient_name: patient.name,
        medication_name: medication.name,
        inventory: medication.inventory,
        dose,
    }))
}

pub async fn update(
    Path((patient_id, medication_id, dose_id)): Path<(i64, i64, i64)>,
    State(storage): State<Storage>,
    Json(payload): Json<dose::CreateDose>,
) -> Result<(), (StatusCode, String)> {
    let taken_at_naive_utc = payload.taken_at.naive_utc();

    let result = sqlx::query!(
        r#"
        UPDATE doses
        SET quantity = ?, taken_at = ?, noted_by_user = ?
        WHERE patient_id = ? AND medication_id = ? AND id = ?
        "#,
        payload.quantity,
        taken_at_naive_utc,
        payload.noted_by_user,
        patient_id,
        medication_id,
        dose_id,
    )
    .execute(&storage.pool)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update dose".to_string(),
        )
    })?;

    match result.rows_affected() {
        n if n != 1 => {
            log::error!("Expected exactly one row to be updated, but {n} rows were affected");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update dose".to_string(),
            ));
        }
        _ => {}
    }

    Ok(())
}

pub async fn delete(
    Path((patient_id, medication_id, dose_id)): Path<(i64, i64, i64)>,
    State(storage): State<Storage>,
) -> Result<(), (StatusCode, String)> {
    let result = sqlx::query!(
        r#"
        DELETE FROM doses
        WHERE patient_id = ? AND medication_id = ? AND id = ?
        "#,
        patient_id,
        medication_id,
        dose_id,
    )
    .execute(&storage.pool)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to delete dose".to_string(),
        )
    })?;

    match result.rows_affected() {
        n if n != 1 => {
            log::error!("Expected exactly one row to be deleted, but {n} rows were affected");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update dose".to_string(),
            ));
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::app_state::AppState;

    use super::*;
    use chrono::{DateTime, TimeDelta, Utc};
    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};
    use shared::{
        api::{
            dose::{self, AvailableDose},
            patient::Reminders,
        },
        time::{now, use_fake_time},
    };
    use sqlx::SqlitePool;

    #[fixture]
    fn taken_at() -> DateTime<Utc> {
        unsafe {
            use_fake_time();
        }
        now() - TimeDelta::hours(1)
    }

    #[rstest]
    #[case(
        2.0,
        Some("Alice"),
        "Alice",
        "Aspirin",
        "Alice took Aspirin (2) an hour ago (2025-01-01 (Wed) 23:00)"
    )]
    #[case(
        0.0,
        Some("Alice"),
        "Alice",
        "Aspirin",
        "Alice decided to skip Aspirin (0) an hour ago (2025-01-01 (Wed) 23:00)"
    )]
    #[case(
        2.0,
        None,
        "Alice",
        "Aspirin",
        "Someone gave Alice Aspirin (2) an hour ago (2025-01-01 (Wed) 23:00)"
    )]
    #[case(
        0.0,
        None,
        "Alice",
        "Aspirin",
        "Someone decided to skip giving Alice Aspirin (0) an hour ago (2025-01-01 (Wed) 23:00)"
    )]
    #[case(
        2.0,
        Some("Alice"),
        "Bob",
        "Aspirin",
        "Alice gave Bob Aspirin (2) an hour ago (2025-01-01 (Wed) 23:00)"
    )]
    #[case(
        0.0,
        Some("Alice"),
        "Bob",
        "Aspirin",
        "Alice decided to skip giving Bob Aspirin (0) an hour ago (2025-01-01 (Wed) 23:00)"
    )]
    fn test_dose_message(
        #[case] quantity: f64,
        #[case] noted_by_user: Option<&str>,
        #[case] patient_name: &str,
        #[case] medication_name: &str,
        #[case] expected: &str,
        taken_at: DateTime<Utc>,
    ) {
        unsafe {
            time::use_fake_time();
        }
        let payload = CreateDose {
            quantity,
            taken_at,
            noted_by_user: noted_by_user.map(|s: &str| s.to_owned()),
        };
        assert_eq!(
            expected,
            dose_message(&payload, patient_name, medication_name),
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql"))]
    async fn record_dose_fails_with_nonexistent_medication(db: SqlitePool) {
        let app_state = AppState::new(db, None).await.unwrap();

        let result = record(
            Path((1, 999)),
            Query(Default::default()),
            State(app_state.storage.clone()),
            State(app_state.messenger.clone()),
            Json(dose::CreateDose {
                quantity: 2.0,
                taken_at: Utc::now(),
                noted_by_user: Some("Alice".to_string()),
            }),
        )
        .await;

        assert_eq!(
            result,
            Err((StatusCode::NOT_FOUND, "Medication not found".to_string()))
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn record_dose_succeeds(db: SqlitePool) {
        unsafe {
            time::use_fake_time();
        }
        let app_state = AppState::new(db, None).await.unwrap();

        let taken_at = now() - TimeDelta::hours(1);

        record(
            Path((1, 1)),
            Query(Default::default()),
            State(app_state.storage.clone()),
            State(app_state.messenger.clone()),
            Json(dose::CreateDose {
                quantity: 2.0,
                taken_at,
                noted_by_user: Some("Alice".to_string()),
            }),
        )
        .await
        .unwrap();

        let result = list(Path((1, 1)), State(app_state.storage.clone()))
            .await
            .unwrap()
            .0;

        assert_eq!(
            result,
            responses::PatientGetDosesResponse {
                patient_name: "Alice".into(),
                medication: PatientMedicationCreateRequest {
                    name: "Aspirin".into(),
                    description: Some("Pain reliever and anti-inflammatory".into()),
                    dose_limits: vec![],
                    inventory: Some(4.0), /* was 6.0 */
                },
                doses: vec![dose::Dose {
                    id: 1,
                    data: dose::CreateDose {
                        quantity: 2.0,
                        taken_at,
                        noted_by_user: Some("Alice".into()),
                    },
                }],
                reminders: Reminders {
                    cron_schedules: vec![]
                },
                next_doses: vec![AvailableDose {
                    time: now(),
                    quantity: None,
                }],
            }
        );

        assert_eq!(
            app_state
                .messenger
                .telegram_messages
                .get_messages(-123)
                .await
                .unwrap(),
            vec![(
                1,
                r#"Alice took Aspirin \(2\) an hour ago \(2025\-01\-01 \(Wed\) 23:00\)"#
                    .to_string()
            )]
        );
    }
}
