use crate::{
    dose_limits, messenger::Messenger, reminder_scheduler::ReminderScheduler, storage::Storage,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use futures::stream::{self, StreamExt, TryStreamExt};
use shared::api::{
    dose::CreateDose,
    medication::{self, DoseLimit},
    patient, requests, responses,
};
use teloxide::utils::markdown;

pub async fn get(
    Path(patient_id): Path<i64>,
    State(storage): State<Storage>,
) -> Result<Json<responses::PatientGetResponse>, (StatusCode, String)> {
    // Fetch patient details
    let patient = storage.get_patient(patient_id).await?;

    // Fetch medications and their last dose time for this patient
    // This query joins medications and doses, grouping by medication
    let medications = sqlx::query!(
        r#"
        SELECT
            m.id AS "id!",
            m.name AS "name!",
            m.dose_limits AS dose_limits,
            MAX(d.taken_at) AS last_taken_at
        FROM medications m
        LEFT JOIN doses d ON m.id = d.medication_id AND d.patient_id = $1
        GROUP BY m.id, m.name
        ORDER BY last_taken_at DESC NULLS LAST
        "#,
        patient_id
    )
    .fetch_all(&storage.pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch medication data".into(),
        )
    })?;

    // Can't use query_as! here because taken_at is interpreted as a
    // NaiveDateTime rather than DateTime<Utc>; see https://github.com/launchbadge/sqlx/issues/2288
    let medications: Vec<medication::MedicationSummary> = stream::iter(medications)
        .map(|med| {
            let storage = storage.clone();
            tokio::spawn(async move {
                medication::MedicationSummary {
                    id: med.id,
                    name: med.name,
                    last_taken_at: med.last_taken_at.map(|ndt| ndt.and_utc()),
                    next_doses: get_next_doses(&storage, patient_id, med.id, med.dose_limits).await,
                }
            })
        })
        .buffer_unordered(5)
        .try_collect()
        .await
        .map_err(|e| {
            log::error!("Failed to fetch next doses: {e:?}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch medication data".to_string(),
            )
        })?;

    let response = responses::PatientGetResponse {
        name: patient.name,
        telegram_group_id: patient.telegram_group_id,
        medications,
    };

    Ok(Json(response))
}

async fn get_next_doses(
    storage: &Storage,
    patient_id: i64,
    medication_id: i64,
    dose_limits: Option<String>,
) -> Vec<CreateDose> {
    // TODO: Add tests

    // TODO: Returning taken_at in the API here stutters a lot, and
    // noted_by_user showing up as explicit nulls is also not amazing.

    // TODO: In all of our "returning nulls", we make it very difficult to
    // confidently say "yes you can taken another dose now".
    let Ok(dose_limits) = DoseLimit::vec_from_string(&dose_limits) else {
        log::error!("Invalid dose_limits {dose_limits:?}");
        return vec![];
    };

    log::info!("Doing stuff, hey!");

    let max_age = dose_limits
        .iter()
        .max_by_key(|lim| lim.hours)
        .map(|lim| chrono::TimeDelta::hours(lim.hours.into()));

    let Some(max_age) = max_age else {
        // Presumably no limit
        return vec![];
    };

    let earliest = shared::time::now().checked_sub_signed(max_age);

    let doses = sqlx::query!(
        r#"
        SELECT
          quantity as "quantity!",
          taken_at as "taken_at!"
        FROM doses
        WHERE
          patient_id = ? AND
          medication_id = ? AND
          taken_at > ?
        ORDER BY taken_at ASC
        "#,
        patient_id,
        medication_id,
        earliest,
    )
    .fetch_all(&storage.pool)
    .await;

    let Ok(doses) = doses else {
        log::error!("Failed to fetch doses for limit calculation: {doses:?}");
        return vec![];
    };

    let doses: Vec<_> = doses
        .into_iter()
        .map(|dose| CreateDose {
            quantity: dose.quantity,
            taken_at: dose.taken_at.and_utc(),
            noted_by_user: None,
        })
        .collect();

    dose_limits::next_allowed(&doses, &dose_limits)
        // next_allowed might return None due to errors, but those would be
        // logged within.
        .unwrap_or_default()
}

pub async fn delete(
    State(storage): State<Storage>,
    State(mut reminder_scheduler): State<ReminderScheduler>,
    Path(patient_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let internal_server_error = (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to delete patient",
    );

    let mut tx = storage.pool.begin().await.map_err(|e| {
        log::error!("Failed to create transaction: {e}");
        internal_server_error
    })?;

    sqlx::query!(
        r#"
        DELETE FROM doses
        WHERE patient_id = ?
        "#,
        patient_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to delete patient's doses: {e}");
        internal_server_error
    })?;

    sqlx::query!(
        r#"
        DELETE FROM reminders
        WHERE patient_id = ?
        "#,
        patient_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to delete patient's reminders : {e}");
        internal_server_error
    })?;

    let result = sqlx::query!(
        r#"
        DELETE FROM patients
        WHERE id = ?
        "#,
        patient_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to delete patient: {e}");
        internal_server_error
    })?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Patient not found"));
    }

    tx.commit().await.map_err(|e| {
        log::error!("Failed to commit transaction: {e}");
        internal_server_error
    })?;

    reminder_scheduler
        .remove_patient(patient_id)
        .await
        .map_err(|e| {
            log::error!("Failed to remove patient from scheduler: {e}");
            internal_server_error
        })?;

    Ok(StatusCode::OK)
}

pub async fn update(
    State(storage): State<Storage>,
    Path(patient_id): Path<i64>,
    Json(payload): Json<requests::PatientCreateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
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
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update patient".to_string(),
        )
    })?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Patient not found".to_string()));
    }
    Ok(StatusCode::OK)
}

pub async fn create(
    State(storage): State<Storage>,
    Json(payload): Json<requests::PatientCreateRequest>,
) -> Result<(StatusCode, Json<responses::PatientCreateResponse>), (StatusCode, &'static str)> {
    let result = sqlx::query!(
        r#"
        INSERT INTO patients(name,telegram_group_id) VALUES
        (?, ?)
        "#,
        payload.name,
        payload.telegram_group_id,
    )
    .execute(&storage.pool)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create patient",
        )
    })?;
    Ok((
        StatusCode::OK,
        Json(responses::PatientCreateResponse {
            id: result.last_insert_rowid(),
        }),
    ))
}

pub async fn list(
    State(storage): State<Storage>,
) -> Result<Json<Vec<patient::Patient>>, (StatusCode, String)> {
    let patients = sqlx::query_as!(
        patient::Patient,
        r#"SELECT id as "id!", name FROM patients"#
    )
    .fetch_all(&storage.pool)
    .await
    .map_err(|e| {
        log::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch users".to_string(),
        )
    })?;

    Ok(Json(patients))
}

pub async fn ping(
    State(storage): State<Storage>,
    State(messenger): State<Messenger>,
    Path(patient_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let patient = storage.get_patient(patient_id).await?;

    log::debug!("Pinging patient {:?}", patient);

    messenger.send(&patient, markdown::escape("Ping!")).await?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use crate::app_state::AppState;

    use super::*;
    use pretty_assertions::assert_eq;
    use shared::api::patient::Patient;
    use sqlx::SqlitePool;

    #[sqlx::test(fixtures("../fixtures/patients.sql"))]
    async fn list_patients_correct(db: SqlitePool) {
        let app_state = AppState::new(db, None).await.unwrap();

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
}
