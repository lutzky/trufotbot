use crate::api::{
    dose::{AvailableDose, CreateDose},
    medication::DoseLimit,
};

use crate::{dose_limits, storage::Storage};

pub async fn get_next_doses(
    storage: &Storage,
    patient_id: i64,
    medication_id: i64,
    dose_limits: &[DoseLimit],
) -> anyhow::Result<Vec<AvailableDose>> {
    let max_age = dose_limits
        .iter()
        .max_by_key(|lim| lim.hours)
        .map(|lim| chrono::TimeDelta::hours(lim.hours.into()));

    let Some(max_age) = max_age else {
        return dose_limits::next_allowed(&[], dose_limits);
    };

    let earliest = crate::time::now().checked_sub_signed(max_age);

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
        anyhow::bail!("Failed to fetch doses for limit calculation: {doses:?}");
    };

    let doses: Vec<_> = doses
        .into_iter()
        .map(|dose| CreateDose {
            quantity: dose.quantity,
            taken_at: dose.taken_at.and_utc(),
            noted_by_user: None,
        })
        .collect();

    dose_limits::next_allowed(&doses, dose_limits)
}

#[cfg(test)]
mod tests {
    use crate::{
        api::requests::{
            CreateDoseQueryParams, PatientCreateRequest, PatientMedicationCreateRequest,
        },
        time::FAKE_TIME,
    };
    use axum::{
        Json,
        extract::{Path, Query, State},
    };
    use sqlx::SqlitePool;

    use crate::{app_state::AppState, handlers, messenger::nil_sender::NilSender};

    use super::*;

    struct TestFixture {
        app_state: AppState,
        patient_id: i64,
        medication_id: i64,
    }

    impl TestFixture {
        async fn new(db: SqlitePool, dose_limits: &str) -> TestFixture {
            let app_state = AppState::new(db.clone(), NilSender::new().into())
                .await
                .unwrap();

            let patient_id = handlers::patients::create(
                State(app_state.storage.clone()),
                Json(PatientCreateRequest {
                    name: "Jonathan Doe".into(),
                    telegram_group_id: None,
                }),
            )
            .await
            .unwrap()
            .id;

            let medication_id = handlers::medication::create(
                State(app_state.storage.clone()),
                Json(PatientMedicationCreateRequest {
                    name: "TestMed".into(),
                    description: None,
                    dose_limits: DoseLimit::vec_from_string(dose_limits).unwrap(),
                    inventory: None,
                }),
            )
            .await
            .unwrap()
            .1
            .id;

            TestFixture {
                app_state,
                patient_id,
                medication_id,
            }
        }

        async fn record_dose(&self, taken_at: &str, quantity: f64) {
            handlers::doses::record(
                Path((self.patient_id, self.medication_id)),
                Query(CreateDoseQueryParams {
                    reminder_message_id: None,
                    reminder_sent_time: None,
                }),
                State(self.app_state.storage.clone()),
                State(self.app_state.messenger.clone()),
                Json(CreateDose {
                    quantity,
                    taken_at: chrono::DateTime::parse_from_rfc3339(taken_at)
                        .unwrap()
                        .into(),
                    noted_by_user: None,
                }),
            )
            .await
            .unwrap();
        }
    }

    fn create_available_dose(quantity: f64, taken_at: &str) -> AvailableDose {
        AvailableDose {
            quantity: if quantity.is_nan() {
                None
            } else {
                Some(quantity)
            },
            time: chrono::DateTime::parse_from_rfc3339(taken_at)
                .unwrap()
                .into(),
        }
    }

    async fn assert_next_doses(
        db: SqlitePool,
        doses: &[(&str, f64)],
        dose_limits: &str,
        want: &[(&str, f64)],
    ) {
        let want: Vec<_> = want
            .iter()
            .map(|(taken_at, quantity)| create_available_dose(*quantity, taken_at))
            .collect();

        let fixture = TestFixture::new(db, dose_limits).await;

        FAKE_TIME
            .scope("2025-01-02T00:00:00Z", async {
                for (taken_at, quantity) in doses {
                    fixture.record_dose(taken_at, *quantity).await;
                }

                let got = get_next_doses(
                    &fixture.app_state.storage,
                    fixture.patient_id,
                    fixture.medication_id,
                    &DoseLimit::vec_from_string(dose_limits).unwrap(),
                )
                .await;

                pretty_assertions::assert_eq!(got.unwrap(), want);
            })
            .await;
    }

    #[sqlx::test]
    async fn relevant_dose(db: SqlitePool) {
        assert_next_doses(
            db,
            &[("2025-01-01T23:00:00Z", 2.0)],
            "4:2,24:8",
            &[("2025-01-02T03:00:00Z", 2.0)],
        )
        .await;
    }

    #[sqlx::test]
    async fn no_rules(db: SqlitePool) {
        assert_next_doses(db, &[], "", &[("2025-01-02T00:00:00Z", f64::NAN)]).await;
    }

    #[sqlx::test]
    async fn old_dose(db: SqlitePool) {
        assert_next_doses(
            db,
            &[("2024-12-28T17:00:00Z", 2.0)],
            "4:2,24:8",
            &[("2025-01-02T00:00:00Z", 2.0)],
        )
        .await;
    }

    #[sqlx::test]
    async fn full_test(db: SqlitePool) {
        assert_next_doses(
            db,
            &[
                ("2024-12-28T17:00:00Z", 2.0),
                ("2025-01-01T17:00:00Z", 2.0),
                ("2025-01-01T18:00:00Z", 2.0),
                ("2025-01-01T21:00:00Z", 2.0),
                ("2025-01-01T22:00:00Z", 1.0),
            ],
            "4:2,24:8",
            &[("2025-01-02T01:00:00Z", 1.0), ("2025-01-02T17:00:00Z", 2.0)],
        )
        .await;
    }

    #[sqlx::test]
    async fn no_doses(db: SqlitePool) {
        assert_next_doses(db, &[], "4:2,24:8", &[("2025-01-02T00:00:00Z", 2.0)]).await;
    }
}
