use crate::{
    errors::ServiceError, models, next_doses, reminder_scheduler::ReminderScheduler,
    storage::Storage,
};
use axum::{
    Json,
    extract::{Path, State},
};
use futures::stream::{self, StreamExt, TryStreamExt};
use shared::api::{
    medication::{DoseLimit, MedicationSummary},
    patient, requests, responses,
};

pub async fn get(
    Path(patient_id): Path<i64>,
    State(storage): State<Storage>,
) -> Result<Json<responses::PatientGetResponse>, ServiceError> {
    // Fetch patient details
    let patient = models::Patient::get(&storage.pool, patient_id).await?;

    // Fetch medications and their last dose time for this patient
    // This query joins medications and doses, grouping by medication
    let medications = sqlx::query!(
        r#"
        SELECT
            m.id AS "id!",
            m.name AS "name!",
            m.dose_limits AS dose_limits,
            m.inventory AS inventory,
            MAX(d.taken_at) AS last_taken_at
        FROM medications m
        LEFT JOIN doses d ON m.id = d.medication_id AND d.patient_id = $1
        GROUP BY m.id, m.name
        "#,
        patient_id
    )
    .fetch_all(&storage.pool)
    .await?;

    let mut medications = stream::iter(medications)
        .map(async |med| -> Result<MedicationSummary, ServiceError> {
            let storage = storage.clone();
            Ok(MedicationSummary {
                id: med.id,
                name: med.name,
                inventory: med.inventory,
                last_taken_at: med.last_taken_at.map(|ndt| ndt.and_utc()),
                next_doses: next_doses::get_next_doses(
                    &storage,
                    patient_id,
                    med.id,
                    &DoseLimit::vec_from_string(&med.dose_limits.unwrap_or_default()).unwrap(),
                )
                .await?,
            })
        })
        .buffer_unordered(5)
        .try_collect::<Vec<_>>()
        .await?;

    // Sort only after completing concurrent operations
    medications.sort_by_key(|med| std::cmp::Reverse(med.last_taken_at));

    let response = responses::PatientGetResponse {
        name: patient.name,
        telegram_group_id: patient.telegram_group_id,
        medications,
    };

    Ok(Json(response))
}

pub async fn delete(
    State(storage): State<Storage>,
    State(mut reminder_scheduler): State<ReminderScheduler>,
    Path(patient_id): Path<i64>,
) -> Result<(), ServiceError> {
    let mut tx = storage.pool.begin().await?;

    sqlx::query!(
        r#"
        DELETE FROM doses
        WHERE patient_id = ?
        "#,
        patient_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM reminders
        WHERE patient_id = ?
        "#,
        patient_id
    )
    .execute(&mut *tx)
    .await?;

    let result = sqlx::query!(
        r#"
        DELETE FROM patients
        WHERE id = ?
        "#,
        patient_id
    )
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        return Err(ServiceError::not_found("Patient not found"));
    }

    tx.commit().await?;

    reminder_scheduler.remove_patient(patient_id).await?;

    Ok(())
}

pub async fn update(
    State(storage): State<Storage>,
    Path(patient_id): Path<i64>,
    Json(payload): Json<requests::PatientCreateRequest>,
) -> Result<(), ServiceError> {
    let result = sqlx::query!(
        r#"
        UPDATE patients
        SET name = ?,
            telegram_group_id = ?
        WHERE id = ?
        "#,
        payload.name,
        payload.telegram_group_id,
        patient_id
    )
    .execute(&storage.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ServiceError::not_found("Patient not found"));
    }
    Ok(())
}

pub async fn create(
    State(storage): State<Storage>,
    Json(payload): Json<requests::PatientCreateRequest>,
) -> Result<Json<responses::PatientCreateResponse>, ServiceError> {
    let result = sqlx::query!(
        r#"
        INSERT INTO patients(name,telegram_group_id) VALUES
        (?, ?)
        "#,
        payload.name,
        payload.telegram_group_id,
    )
    .execute(&storage.pool)
    .await?;
    Ok(Json(responses::PatientCreateResponse {
        id: result.last_insert_rowid(),
    }))
}

pub async fn list(
    State(storage): State<Storage>,
) -> Result<Json<Vec<patient::Patient>>, ServiceError> {
    let patients = sqlx::query_as!(
        patient::Patient,
        r#"SELECT id as "id!", name FROM patients"#
    )
    .fetch_all(&storage.pool)
    .await?;

    Ok(Json(patients))
}

#[cfg(test)]
mod tests {
    use crate::{
        app_state::AppState,
        handlers::doses::record,
        messenger::{Messenger, nil_sender::NilSender},
    };

    use super::*;
    use axum::extract::Query;
    use chrono::Days;
    use pretty_assertions::assert_eq;
    use shared::{
        api::{
            dose::{self},
            patient::Patient,
            requests::CreateDoseQueryParams,
        },
        time,
    };
    use sqlx::SqlitePool;

    #[sqlx::test(fixtures("../fixtures/patients.sql"))]
    async fn list_patients_correct(db: SqlitePool) {
        let app_state = AppState::new(db, NilSender::new().into()).await.unwrap();

        let patients = list(State(app_state.storage.clone())).await.unwrap();
        assert_eq!(
            patients.0,
            vec![
                Patient {
                    id: 1,
                    name: "Alice".to_string(),
                },
                Patient {
                    id: 2,
                    name: "Bob".to_string(),
                },
                Patient {
                    id: 3,
                    name: "Carol".to_string(),
                },
            ]
        );
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    async fn test_get_order(db: SqlitePool) {
        let messenger: Messenger = NilSender::new().into();
        let app_state = AppState::new(db, messenger.clone()).await.unwrap();

        let want_id_last_taken_ordered = vec![
            (4, "2024-12-31T00:00:00+00:00"),
            (3, "2024-12-30T00:00:00+00:00"),
            (2, "2024-12-29T00:00:00+00:00"),
            (1, "2024-12-28T00:00:00+00:00"),
            (5, ""), // None is last
        ];

        for (medication_id, days_ago) in [(1, 5), (2, 4), (3, 3), (4, 2)] {
            record(
                Path((1, medication_id)),
                Query(CreateDoseQueryParams {
                    reminder_message_id: None,
                }),
                State(app_state.storage.clone()),
                State(messenger.clone()),
                Json(dose::CreateDose {
                    quantity: 1.0,
                    taken_at: time::now().checked_sub_days(Days::new(days_ago)).unwrap(),
                    noted_by_user: None,
                }),
            )
            .await
            .unwrap();
        }

        let result = get(Path(1), State(app_state.storage.clone()))
            .await
            .unwrap()
            .0
            .medications
            .into_iter()
            .map(|summary| {
                (
                    summary.id,
                    summary
                        .last_taken_at
                        .map_or("".to_string(), |t| t.to_rfc3339()),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            result,
            want_id_last_taken_ordered
                .into_iter()
                .map(|(id, time)| (id, time.to_string()))
                .collect::<Vec<_>>()
        );
    }
}
