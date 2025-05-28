use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct MedicationSummary {
    pub id: i64,
    pub name: String,
    pub last_taken_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct DoseLimit {
    pub hours: u16,
    pub amount: f64,
}

use anyhow::{Result, bail};

impl DoseLimit {
    pub fn vec_from_string(s: &Option<String>) -> Result<Vec<DoseLimit>> {
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

    pub fn string_from_vec(dose_limits: &[DoseLimit]) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::api::medication::DoseLimit;

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
        let result = DoseLimit::vec_from_string(&input_str);
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
        let result = DoseLimit::string_from_vec(&dose_limits);
        assert_eq!(result, want_string);
    }
}
