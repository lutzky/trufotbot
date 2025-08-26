use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use shared::{
    api::{
        dose::{self, CreateDose, Dose},
        requests::{CreateDoseQueryParams, PatientMedicationCreateRequest},
        responses,
    },
    time,
};
use teloxide::{types::ChatId, utils::markdown};

use crate::{
    errors::ServiceError,
    frontend_url,
    messenger::{MessageId, Messenger},
    models::{Medication, Patient},
    next_doses::get_next_doses,
    storage::Storage,
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
) -> Result<StatusCode, ServiceError> {
    let patient = Patient::get(&storage.pool, patient_id).await?;
    let medication = Medication::get(&storage.pool, medication_id).await?;

    let mut tx = storage.pool.begin().await?;

    let res = sqlx::query!(
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
    .await?;

    // https://sqlite.org/c3ref/last_insert_rowid.html indicates this should match our primary key
    let dose_id = res.last_insert_rowid();

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
        .await?;
    }

    tx.commit().await?;

    let dose = Dose {
        data: payload,
        id: dose_id,
    };

    let sent_message_id = notify(
        &messenger,
        if reminder_message_id.is_some() {
            NotificationType::ReminderDone
        } else {
            NotificationType::Normal
        },
        reminder_message_id,
        None,
        &patient,
        &medication,
        &dose,
    )
    .await?;

    if let Some(sent_message_id) = sent_message_id {
        let res = sqlx::query!(
            r#"
            UPDATE doses
            SET telegram_message_id = ?,
                telegram_group_id = ?
            WHERE id = ?
            "#,
            sent_message_id,
            patient.telegram_group_id,
            dose_id
        )
        .execute(&storage.pool)
        .await;
        if let Err(err) = res {
            log::error!("Error setting telegram_message_id for dose {dose_id}: {err}");
            // ...but continue operating.
        }
    }

    Ok(StatusCode::CREATED)
}

enum NotificationType {
    Normal,
    ReminderDone,
    Edited,
}

impl core::fmt::Display for NotificationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationType::Normal => write!(f, ""),
            NotificationType::ReminderDone => write!(f, "✅ "),
            NotificationType::Edited => write!(f, "✏️ "),
        }
    }
}

async fn notify(
    messenger: &Messenger,
    notification_type: NotificationType,
    edit_message_id: Option<MessageId>,
    override_chat_id: Option<ChatId>,
    patient: &Patient,
    medication: &Medication,
    dose: &Dose,
) -> Result<Option<MessageId>, ServiceError> {
    let base_msg = markdown::escape(&dose_message(&dose.data, patient, medication));

    let message = format!(
        "{notification_type}{base_msg}\n\n\\[{}\\]",
        edit_dose_link(patient, medication, dose.id)
    );

    let sent_message_id;

    if let Some(edit_message_id) = edit_message_id {
        messenger
            .edit(patient, override_chat_id, edit_message_id, message, vec![])
            .await?;
        sent_message_id = Some(edit_message_id);
    } else {
        sent_message_id = messenger.send(patient, message).await?.map(|id| id.id());
    }

    Ok(sent_message_id)
}

fn dose_message(payload: &CreateDose, patient: &Patient, medication: &Medication) -> String {
    let giver_name = match &payload.noted_by_user {
        None => "Someone",
        Some(name) => name,
    };

    let patient_name = &patient.name;
    let medication_name = &medication.name;

    fn normalize(s: &str) -> String {
        s.trim().to_lowercase()
    }

    let who_gave_whom = match (
        normalize(patient_name) == normalize(giver_name),
        payload.quantity == 0.0,
    ) {
        (true, false) => format!("{patient_name} took"),
        (true, true) => format!("{patient_name} decided to skip"),
        (false, false) => format!("{giver_name} gave {patient_name}"),
        (false, true) => format!("{giver_name} decided to skip giving {patient_name}"),
    };

    let medication_and_amount = format!("{medication_name} ({})", payload.quantity);
    let when = format!(
        "{} ({})",
        time::time_ago(&payload.taken_at),
        time::local_display(&payload.taken_at),
    );

    let who_gave_whom = markdown::escape(&who_gave_whom);

    format!("{who_gave_whom} {medication_and_amount} {when}")
}

fn edit_dose_link(patient: &Patient, medication: &Medication, dose_id: i64) -> String {
    let mut url = url::Url::parse(&frontend_url::get()).unwrap();

    url.path_segments_mut()
        .unwrap()
        .push("patients")
        .push(&patient.id.to_string())
        .push("medications")
        .push(&medication.id.to_string())
        .push("doses")
        .push(&dose_id.to_string());

    markdown::link(url.as_str(), "Edit")
}

pub async fn list(
    Path((patient_id, medication_id)): Path<(i64, i64)>,
    State(storage): State<Storage>,
) -> Result<Json<responses::PatientGetDosesResponse>, ServiceError> {
    let patient = Patient::get(&storage.pool, patient_id).await?;
    let medication = Medication::get(&storage.pool, medication_id).await?;

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
    .await?;

    let reminders = reminders::get(State(storage.clone()), Path((patient_id, medication_id)))
        .await
        .inspect_err(|e| {
            log::error!("Failed to fetch reminders: {e:?}");
        })?
        .0;

    let next_doses =
        get_next_doses(&storage, patient_id, medication_id, &medication.dose_limits).await?;

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
) -> Result<Json<responses::GetDoseResponse>, ServiceError> {
    let patient = Patient::get(&storage.pool, patient_id).await?;
    let medication = Medication::get(&storage.pool, medication_id).await?;

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
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => ServiceError::not_found("Dose not found"),
        _ => err.into(),
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
    State(messenger): State<Messenger>,
    State(storage): State<Storage>,
    Json(payload): Json<dose::CreateDose>,
) -> Result<(), ServiceError> {
    let medication = Medication::get(&storage.pool, medication_id).await?;

    let mut tx = storage.pool.begin().await?;

    if let Some(inventory) = medication.inventory {
        let old_quantity = sqlx::query!(
            r#"
            SELECT quantity
            FROM doses
            WHERE patient_id = ? AND medication_id = ? AND id = ?
            "#,
            patient_id,
            medication_id,
            dose_id
        )
        .map(|row| row.quantity)
        .fetch_one(&mut *tx)
        .await?;

        let new_inventory = inventory + old_quantity - payload.quantity;

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
        .await?;
    }

    let result = sqlx::query!(
        r#"
        UPDATE doses
        SET quantity = ?, taken_at = ?, noted_by_user = ?
        WHERE patient_id = ? AND medication_id = ? AND id = ?
        "#,
        payload.quantity,
        payload.taken_at,
        payload.noted_by_user,
        patient_id,
        medication_id,
        dose_id,
    )
    .execute(&mut *tx)
    .await?;

    match result.rows_affected() {
        n if n != 1 => {
            return Err(ServiceError::InternalError(anyhow::anyhow!(
                "Expected exactly one row to be updated, but {n} rows were affected"
            )));
        }
        _ => {}
    }

    tx.commit().await?;

    if let Some((group_id, message_id)) =
        get_dose_notification_details(dose_id, State(storage.clone())).await?
    {
        let dose = Dose {
            data: payload,
            id: dose_id,
        };

        let patient = Patient::get(&storage.pool, patient_id).await?;
        let result = notify(
            &messenger,
            NotificationType::Edited,
            Some(message_id),
            Some(group_id),
            &patient,
            &medication,
            &dose,
        )
        .await;
        if let Err(err) = result {
            log::error!("Failed to update message for dose {dose_id}: {err}");
            // ...but continue operating.
        }
    }

    Ok(())
}

fn convert_message_id_or_warn(message_id: i64) -> Option<MessageId> {
    message_id
        .try_into()
        .map_err(|e| {
            log::error!("Invalid message_id {message_id:?} doesn't fit in an i32: {e}");
        })
        .ok()
}

pub async fn get_dose_notification_details(
    dose_id: i64,
    State(storage): State<Storage>,
) -> Result<Option<(ChatId, MessageId)>, ServiceError> {
    let result = sqlx::query!(
        r#"
        SELECT telegram_group_id, telegram_message_id
        FROM doses
        WHERE id = ?
        "#,
        dose_id
    )
    .fetch_one(&storage.pool)
    .await;

    match result {
        Ok(row) => {
            let group_id: Option<ChatId> = row.telegram_group_id.map(ChatId);
            let message_id: Option<MessageId> =
                row.telegram_message_id.and_then(convert_message_id_or_warn);

            Ok(group_id.zip(message_id))
        }
        Err(sqlx::Error::RowNotFound) => Ok(None),
        Err(err) => Err(ServiceError::DatabaseError(err)),
    }
}

pub async fn delete(
    Path((patient_id, medication_id, dose_id)): Path<(i64, i64, i64)>,
    State(messenger): State<Messenger>,
    State(storage): State<Storage>,
) -> Result<(), ServiceError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM doses
        WHERE patient_id = ? AND medication_id = ? AND id = ?
        RETURNING telegram_group_id, telegram_message_id
        "#,
        patient_id,
        medication_id,
        dose_id,
    )
    .fetch_optional(&storage.pool)
    .await?;

    let Some(result) = result else {
        return Err(ServiceError::InternalError(anyhow::anyhow!(
            "No dose found"
        )));
    };

    match (result.telegram_group_id, result.telegram_message_id) {
        (None, None) => {}
        (Some(group_id), Some(message_id)) => {
            let patient = Patient {
                id: 0,
                telegram_group_id: result.telegram_group_id,
                name: String::new(),
            };
            if let Some(message_id) = convert_message_id_or_warn(message_id)
                && let Err(err) = messenger
                    .edit(
                        &patient,
                        Some(ChatId(group_id)),
                        message_id,
                        r"_This dose was deleted in trufotbot\._".to_string(),
                        vec![],
                    )
                    .await
            {
                log::warn!("Failed to update telegram message when deleting dose {dose_id}: {err}");
            }
        }
        (maybe_group_id, maybe_message_id) => {
            log::warn!(
                "When deleting dose {dose_id}, got group_id {maybe_group_id:?} and message_id \
                {maybe_message_id:?}. Expected either 0 or 2 of those to be None."
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        app_state::AppState,
        messenger::{
            fake_sender::{FakeSender, messages_from_slice},
            nil_sender::NilSender,
        },
    };

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
        Some("alice "),
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
        let patient = Patient {
            id: 0,
            telegram_group_id: None,
            name: patient_name.to_owned(),
        };
        let medication = Medication {
            id: 0,
            name: medication_name.to_owned(),
            description: None,
            dose_limits: vec![],
            inventory: None,
        };
        assert_eq!(expected, dose_message(&payload, &patient, &medication),);
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql"))]
    async fn record_dose_fails_with_nonexistent_medication(db: SqlitePool) {
        let app_state = AppState::new(db, NilSender::new().into()).await.unwrap();

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

        match result {
            Err(ServiceError::NotFound(msg)) if msg == "Medication not found" => {}
            _ => panic!("Unexpected result {result:?}"),
        }
    }

    #[sqlx::test(fixtures("../fixtures/patients.sql", "../fixtures/medications.sql"))]
    // TODO: Split this into separate tests for "record" and "update"
    async fn record_dose_succeeds(db: SqlitePool) {
        unsafe {
            time::use_fake_time();
        }
        let fake_telegram = Arc::new(FakeSender::new());
        let messenger = fake_telegram.clone().into();
        let app_state = AppState::new(db, messenger).await.unwrap();

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
            fake_telegram.messages.get_messages(-123).await.unwrap(),
            messages_from_slice(&[(
                r#"Alice took Aspirin \(2\) an hour ago \(2025\-01\-01 \(Wed\) 23:00\)

\[[Edit](http://0.0.0.0:8080/patients/1/medications/1/doses/1)\]"#,
                &[]
            )])
        );

        update(
            Path((1, 1, 1)),
            State(app_state.messenger.clone()),
            State(app_state.storage.clone()),
            Json(dose::CreateDose {
                quantity: 1.0,
                taken_at,
                noted_by_user: Some("Bob".to_string()),
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            fake_telegram.messages.get_messages(-123).await.unwrap(),
            messages_from_slice(&[(
                r#"✏️ Bob gave Alice Aspirin \(1\) an hour ago \(2025\-01\-01 \(Wed\) 23:00\)

\[[Edit](http://0.0.0.0:8080/patients/1/medications/1/doses/1)\]"#,
                &[]
            )])
        );

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
                    inventory: Some(5.0),
                },
                doses: vec![dose::Dose {
                    id: 1,
                    data: dose::CreateDose {
                        quantity: 1.0,
                        taken_at,
                        noted_by_user: Some("Bob".into()),
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
    }
}
