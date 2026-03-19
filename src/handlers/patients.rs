use crate::api::{
    medication::{DoseLimit, MedicationSummary},
    patient, requests, responses,
};
use crate::{
    errors::ServiceError, models, next_doses, reminder_scheduler::ReminderScheduler,
    storage::Storage,
};
use axum::{
    Json,
    extract::{Path, State},
};
use futures::stream::{self, StreamExt, TryStreamExt};

pub const UTOIPA_TAG: &str = "patients";

#[utoipa::path(
    get,
    path = "/api/patients/{id}",
    summary = "Get a patient",
    operation_id = "patients_get",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Patient found successfully", body=responses::PatientGetResponse),
        (status = 404, description = "Patient not found"),
    ),
    params(
        ("id" = i32, Path, description = "Patient ID"),
    )
)]
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
        LEFT JOIN doses d ON
            m.id = d.medication_id
            AND d.patient_id = $1
            AND d.quantity > 0
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
                    &DoseLimit::vec_from_string(&med.dose_limits.unwrap_or_default())
                        .map_err(ServiceError::InternalError)?,
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

#[utoipa::path(
    delete,
    path = "/api/patients/{id}",
    summary = "Delete a patient",
    tag = UTOIPA_TAG,
    operation_id = "patients_delete",
    responses(
        (status = 200, description = "Patient deleted successfully"),
        (status = 404, description = "Patient not found"),
    ),
    params(
        ("id" = i32, Path, description = "Patient ID"),
    )
)]
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

#[utoipa::path(
    put,
    path = "/api/patients/{id}",
    summary = "Update a patient",
    operation_id = "patients_update",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Patient updated successfully"),
        (status = 404, description = "Patient not found"),
    ),
    request_body = requests::PatientCreateRequest,
    params(
        ("id" = i32, Path, description = "Patient ID"),
    )
)]
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

#[utoipa::path(
    post,
    summary = "Create a patient",
    operation_id = "patients_create",
    path = "/api/patients",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, description = "Patient created successfully", body=responses::PatientCreateResponse),
    ),
    request_body = requests::PatientCreateRequest
)]
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

#[utoipa::path(
    get,
    path = "/api/patients",
    operation_id = "patients_list",
    summary = "List all patients",
    tag = UTOIPA_TAG,
    responses(
        (status = 200, body=Vec<patient::Patient>),
    ),
)]
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
        app_state::{AppState, Config},
        handlers::doses::record,
        messenger::{Messenger, nil_sender::NilSender},
        time::FAKE_TIME,
    };

    use super::*;
    use crate::{
        api::{
            dose::{self},
            patient::Patient,
            requests::CreateDoseQueryParams,
        },
        time,
    };
    use axum::extract::Query;
    use chrono::Days;
    use pretty_assertions::assert_eq;
    use sqlx::SqlitePool;

    #[sqlx::test(fixtures("../fixtures/patients.sql"))]
    async fn list_patients_correct(db: SqlitePool) {
        let app_state = AppState::new(db, NilSender::new().into(), Config::load().unwrap().into())
            .await
            .unwrap();

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
        let app_state = AppState::new(db, messenger.clone(), Config::load().unwrap().into())
            .await
            .unwrap();

        let want_id_last_taken_ordered = vec![
            (4, "2024-12-31T00:00:00+00:00"),
            (3, "2024-12-30T00:00:00+00:00"),
            (2, "2024-12-29T00:00:00+00:00"),
            (1, "2024-12-28T00:00:00+00:00"),
            (5, ""), // None is last
        ];

        let result = FAKE_TIME
            .scope("2025-01-02T00:00:00Z", async {
                for (medication_id, days_ago) in [(1, 5), (2, 4), (3, 3), (4, 2)] {
                    record(
                        Path((1, medication_id)),
                        Query(CreateDoseQueryParams {
                            reminder_message_id: None,
                            reminder_sent_time: None,
                        }),
                        State(app_state.storage.clone()),
                        State(messenger.clone()),
                        State(app_state.config.clone()),
                        Json(dose::CreateDose {
                            quantity: 1.0,
                            taken_at: time::now().checked_sub_days(Days::new(days_ago)).unwrap(),
                            noted_by_user: None,
                        }),
                    )
                    .await
                    .unwrap();
                }

                get(Path(1), State(app_state.storage.clone()))
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
                    .collect::<Vec<_>>()
            })
            .await;

        assert_eq!(
            result,
            want_id_last_taken_ordered
                .into_iter()
                .map(|(id, time)| (id, time.to_string()))
                .collect::<Vec<_>>()
        );
    }
}
