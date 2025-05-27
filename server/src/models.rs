use anyhow::{Result, bail};

use chrono::NaiveDateTime;
use serde::Serialize;
use shared::api::medication::DoseLimit;
use sqlx::{FromRow, SqlitePool}; // Added SqlitePool

#[derive(FromRow, Serialize, Debug, PartialEq)]
pub struct Patient {
    pub id: i64,
    pub telegram_group_id: Option<i64>,
    pub name: String,
}

impl Patient {
    /// Fetches a patient by their ID from the database.
    pub async fn get(db: &SqlitePool, patient_id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Patient,
            r"SELECT id, name, telegram_group_id FROM patients WHERE id = ?",
            patient_id
        )
        .fetch_optional(db)
        .await
    }
}

#[derive(FromRow, Serialize, Debug)]
pub struct Medication {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub dose_limits: Vec<DoseLimit>,
}

impl Medication {
    pub async fn get(db: &SqlitePool, medication_id: i64) -> Result<Option<Self>> {
        let result = sqlx::query!(
            r"SELECT id, name, description, dose_limits FROM medications WHERE id = ?",
            medication_id
        )
        .fetch_optional(db)
        .await?;

        let res = result.map(|result| {
            Ok(Medication {
                id: result.id,
                name: result.name,
                description: result.description,
                dose_limits: Medication::dose_limits_from_string(&result.dose_limits)?,
            })
        });

        res.transpose()
    }

    fn dose_limits_from_string(s: &Option<String>) -> Result<Vec<DoseLimit>> {
        let Some(s) = s else {
            return Ok(vec![]);
        };

        if s.is_empty() {
            return Ok(vec![]);
        }

        s.split(",")
            .map(|part| {
                let Some((hours, amount)) = part.split_once(":") else {
                    bail!("Invalid dose-limit spec {part:?}");
                };
                Ok(DoseLimit {
                    hours: hours.parse()?,
                    amount: amount.parse()?,
                })
            })
            .collect::<Result<Vec<_>>>()
    }

    #[allow(dead_code)] // TODO
    fn dose_limits_to_string(dose_limits: &[DoseLimit]) -> Option<String> {
        if dose_limits.is_empty() {
            return None;
        }

        Some(
            dose_limits
                .iter()
                .map(|DoseLimit { hours, amount }| format!("{hours}:{amount}"))
                .collect::<Vec<String>>()
                .join(","),
        )
    }
}

#[derive(FromRow, Serialize)]
pub struct Reminder {
    pub patient_id: i64,
    pub medication_id: i64,
    pub hour: Option<u8>,
}

#[derive(FromRow, Serialize)]
pub struct Dose {
    pub id: i64,
    pub patient_id: i64,
    pub medication_id: i64,
    pub quantity: f64,
    pub taken_at: NaiveDateTime,
    pub noted_by_user: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::Medication;
    use rstest::rstest;
    use shared::api::medication::DoseLimit;

    #[rstest(
        input_str,
        expected_dose_limits,
        case(None, vec![]),
        case(Some("".to_string()), vec![]), // Empty string should also yield empty vec if split/parsing fails
        case(Some("1:10.5".to_string()), vec![DoseLimit { hours: 1, amount: 10.5 }]),
        case(Some("1:10.5,2:20".to_string()), vec![DoseLimit { hours: 1, amount: 10.5 }, DoseLimit { hours: 2, amount: 20.0 }]),
        case(Some("3:15.123,4:25.0".to_string()), vec![DoseLimit { hours: 3, amount: 15.123 }, DoseLimit { hours: 4, amount: 25.0 }])
    )]
    fn test_dose_limits_from_string(
        input_str: Option<String>,
        expected_dose_limits: Vec<DoseLimit>,
    ) {
        println!("Expecting {input_str:?} -> {expected_dose_limits:?}");
        let result = Medication::dose_limits_from_string(&input_str);
        assert_eq!(result.unwrap(), expected_dose_limits);
    }

    #[rstest(
        want_string,
        dose_limits,
        case(None, vec![]),
        case(Some("1:10.5".to_string()), vec![DoseLimit { hours: 1, amount: 10.5 }]),
        case(Some("1:10.5,2:20".to_string()), vec![DoseLimit { hours: 1, amount: 10.5 }, DoseLimit { hours: 2, amount: 20.0 }]),
        case(Some("3:15.123,4:25".to_string()), vec![DoseLimit { hours: 3, amount: 15.123 }, DoseLimit { hours: 4, amount: 25.0 }])
    )]
    fn test_dose_limits_to_string(dose_limits: Vec<DoseLimit>, want_string: Option<String>) {
        println!("Expecting {dose_limits:?} -> {want_string:?}");
        let result = Medication::dose_limits_to_string(&dose_limits);
        assert_eq!(result, want_string);
    }
}
