use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::dose::AvailableDose;

#[derive(Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct MedicationSummary {
    #[schema(format = Int32)]
    pub id: i64,
    pub name: String,
    pub last_taken_at: Option<DateTime<Utc>>,
    pub next_doses: Vec<AvailableDose>,
    pub inventory: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, ToSchema)]
pub struct DoseLimit {
    #[schema(examples(12))]
    pub hours: u16,

    #[schema(examples(2.5))]
    pub amount: f64,
}

impl DoseLimit {
    pub fn vec_from_string(s: &str) -> Result<Vec<DoseLimit>> {
        s.split(",")
            .filter(|s| !s.is_empty())
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

    pub fn string_from_vec(dose_limits: &[DoseLimit]) -> String {
        dose_limits
            .iter()
            .map(|DoseLimit { hours, amount }| format!("{hours}:{amount}"))
            .collect::<Vec<String>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::api::medication::DoseLimit;

    #[rstest(
        input_str,
        expected_dose_limits,
        case("", &[]),
        case("1:10.5", &[DoseLimit { hours: 1, amount: 10.5 }]),
        case("1:10.5,2:20", &[DoseLimit { hours: 1, amount: 10.5 }, DoseLimit { hours: 2, amount: 20.0 }]),
        case("3:15.123,4:25.0", &[DoseLimit { hours: 3, amount: 15.123 }, DoseLimit { hours: 4, amount: 25.0 }])
    )]
    fn test_dose_limits_from_string(input_str: &str, expected_dose_limits: &[DoseLimit]) {
        println!("Expecting {input_str:?} -> {expected_dose_limits:?}");
        let result = DoseLimit::vec_from_string(input_str);
        assert_eq!(result.unwrap(), expected_dose_limits);
    }

    #[rstest(
        want_string,
        dose_limits,
        case("", &[]),
        case("1:10.5", &[DoseLimit { hours: 1, amount: 10.5 }]),
        case("1:10.5,2:20", &[DoseLimit { hours: 1, amount: 10.5 }, DoseLimit { hours: 2, amount: 20.0 }]),
        case("3:15.123,4:25", &[DoseLimit { hours: 3, amount: 15.123 }, DoseLimit { hours: 4, amount: 25.0 }])
    )]
    fn test_dose_limits_to_string(dose_limits: &[DoseLimit], want_string: &str) {
        println!("Expecting {dose_limits:?} -> {want_string:?}");
        let result = DoseLimit::string_from_vec(dose_limits);
        assert_eq!(result, want_string);
    }
}
